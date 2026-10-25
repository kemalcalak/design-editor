//! Runs a script node's program (#19, feature `script`, native-only).
//!
//! pen.dev's script contract: the program reads `pencil.width`,
//! `pencil.height` and `pencil.input.<name>` and returns an array of nodes;
//! `Math.random()` is deterministic, so the same inputs give the same
//! output. Each run gets a fresh QuickJS runtime with a memory limit and a
//! wall-clock budget, so a runaway program stops instead of freezing the
//! editor. Returned nodes go through the `batch_design` node normalizer
//! (pen.dev field names, colour strings, sizing keywords) before they
//! become `PenNode`s; core assigns their ids when the result lands
//! (`EditorState::land_script_output`).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use de_editor_core::script_regen::{forest_size, ScriptJob, ScriptOutcome};
use de_editor_core::{EditorState, NodeId};
use jian_ops_schema::node::PenNode;
use jian_scene::layout_scene::LayoutScene;
use rquickjs::{Context, Runtime};
use serde_json::Value;

use crate::batch_design::{ensure_node_ids, normalize_node_shape};
use crate::script_runner::{describe_js_error, ScriptError, MAX_SCRIPT_BYTES};

#[path = "script_node_error.rs"]
mod error;

pub use error::ScriptNodeError;

/// Wall-clock budget of one run.
pub const TIME_LIMIT: Duration = Duration::from_secs(1);
/// QuickJS heap limit of one run.
pub const MEMORY_LIMIT_BYTES: usize = 16 * 1024 * 1024;
/// Most nodes (descendants included) one run may return.
pub const MAX_NODES: usize = 2000;

/// The sandbox prelude: a seeded `Math.random` (mulberry32), so a script's
/// output depends only on its inputs.
const PRELUDE: &str = r#"
Math.random = (function (seed) {
  return function () {
    seed = (seed + 0x6d2b79f5) | 0;
    let t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
})(0x9e3779b9);
"#;

/// A finished run, as the host hands it back to core.
#[derive(Debug)]
pub struct ScriptNodeRun {
    pub result: Result<Vec<PenNode>, ScriptNodeError>,
    pub elapsed: Duration,
}

impl ScriptNodeRun {
    /// The outcome core lands (`EditorState::land_script_output`).
    pub fn outcome(self) -> ScriptOutcome {
        match self.result {
            Ok(nodes) => ScriptOutcome::Generated(nodes),
            Err(error) => ScriptOutcome::Failed(error.to_string()),
        }
    }
}

/// Run `job` and time it.
pub fn run_script_job(job: &ScriptJob) -> ScriptNodeRun {
    let started = Instant::now();
    let result = run_script_node(job);
    ScriptNodeRun {
        result,
        elapsed: started.elapsed(),
    }
}

/// Run `job`'s program and parse what it returns.
pub fn run_script_node(job: &ScriptJob) -> Result<Vec<PenNode>, ScriptNodeError> {
    if job.source.len() > MAX_SCRIPT_BYTES {
        return Err(ScriptNodeError::SourceTooLarge {
            bytes: job.source.len(),
            max: MAX_SCRIPT_BYTES,
        });
    }
    let json = eval_to_json(job)?;
    nodes_from_json(&json)
}

/// A number JS can read back (`NaN` / infinities become 0).
fn js_number(value: f64) -> f64 {
    if value.is_finite() {
        value
    } else {
        0.0
    }
}

fn eval_to_json(job: &ScriptJob) -> Result<String, ScriptNodeError> {
    let rt = Runtime::new().map_err(|e| ScriptNodeError::Sandbox(e.to_string()))?;
    rt.set_memory_limit(MEMORY_LIMIT_BYTES);
    let deadline = Instant::now() + TIME_LIMIT;
    let timed_out = Arc::new(AtomicBool::new(false));
    let flag = timed_out.clone();
    rt.set_interrupt_handler(Some(Box::new(move || {
        let over = Instant::now() > deadline;
        if over {
            flag.store(true, Ordering::Relaxed);
        }
        over
    })));
    let ctx = Context::full(&rt).map_err(|e| ScriptNodeError::Sandbox(e.to_string()))?;
    let inputs = serde_json::to_string(&job.inputs).unwrap_or_else(|_| "{}".into());
    // JSON is a JS expression, so the inputs go in as a literal. The source
    // runs as a function body: pen.dev scripts `return` their nodes.
    let program = format!(
        "{PRELUDE}\nglobalThis.pencil = Object.freeze({{ width: {w}, height: {h}, \
         input: Object.freeze({inputs}) }});\nJSON.stringify((function () {{\n{source}\n}})())",
        w = js_number(job.width),
        h = js_number(job.height),
        source = job.source,
    );
    let outcome = ctx.with(|ctx| {
        ctx.eval::<Option<String>, _>(program)
            .map_err(|e| describe_js_error(&ctx, e))
    });
    if timed_out.load(Ordering::Relaxed) {
        return Err(ScriptNodeError::TimedOut {
            limit_ms: TIME_LIMIT.as_millis() as u64,
        });
    }
    match outcome {
        Ok(Some(json)) => Ok(json),
        Ok(None) => Err(ScriptNodeError::NotAnArray),
        Err(ScriptError::Threw(message)) => Err(ScriptNodeError::Threw(message)),
        Err(other) => Err(ScriptNodeError::Threw(other.to_string())),
    }
}

fn has_script_type(value: &Value) -> bool {
    value.get("type").and_then(Value::as_str) == Some("script")
        || value
            .get("children")
            .and_then(Value::as_array)
            .is_some_and(|children| children.iter().any(has_script_type))
}

/// The returned JSON as nodes. Ids are placeholders here.
pub fn nodes_from_json(json: &str) -> Result<Vec<PenNode>, ScriptNodeError> {
    let Ok(Value::Array(items)) = serde_json::from_str::<Value>(json) else {
        return Err(ScriptNodeError::NotAnArray);
    };
    let mut nodes = Vec::with_capacity(items.len());
    let mut next_id = 1usize;
    let mut count = 0usize;
    for (index, mut item) in items.into_iter().enumerate() {
        if !item.is_object() {
            return Err(ScriptNodeError::InvalidNode {
                index,
                detail: "not an object".into(),
            });
        }
        if has_script_type(&item) {
            return Err(ScriptNodeError::NestedScript { index });
        }
        normalize_node_shape(&mut item);
        ensure_node_ids(&mut item, &mut next_id);
        let node: PenNode =
            serde_json::from_value(item).map_err(|e| ScriptNodeError::InvalidNode {
                index,
                detail: e.to_string(),
            })?;
        count += forest_size(std::slice::from_ref(&node));
        nodes.push(node);
        if count > MAX_NODES {
            return Err(ScriptNodeError::TooManyNodes {
                count,
                max: MAX_NODES,
            });
        }
    }
    Ok(nodes)
}

/// Fill sizes for hosts that lay out on demand: a layout of the whole
/// document, built only when a script is fill-sized. Owned, so a host can
/// take jobs, release its document and land them later with the same
/// answers.
pub struct ScriptLayout {
    scene: Option<LayoutScene>,
}

impl ScriptLayout {
    pub fn of(state: &EditorState) -> Self {
        let scene = state
            .scripts_need_layout()
            .then(|| de_pen_loader::editor_state_to_layout_scene(state));
        Self { scene }
    }

    pub fn size(&self, id: &NodeId) -> Option<(f64, f64)> {
        de_pen_loader::scene_node_size(self.scene.as_ref()?, id.as_str())
    }
}

/// Run every stale script now and land the results. For hosts without a
/// frame loop (the file-backed MCP servers), after a write; the desktop
/// editor runs the same jobs on a worker thread instead. Returns whether
/// the document changed.
pub fn regenerate_stale_scripts(state: &mut EditorState) -> bool {
    let layout = ScriptLayout::of(state);
    let sizes = |id: &NodeId| layout.size(id);
    let mut changed = false;
    for job in state.take_script_jobs(&sizes) {
        let run = run_script_job(&job);
        let elapsed_ms = run.elapsed.as_millis() as u64;
        changed |=
            state.land_script_output(&job.node_id, &job.hash, run.outcome(), elapsed_ms, &sizes);
    }
    changed
}

#[cfg(test)]
#[path = "script_node_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "script_node_bench_tests.rs"]
mod bench_tests;
