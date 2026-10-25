//! The script-run contract between an editor without a script engine (the
//! CanvasKit web editor) and the daemon that runs the job for it (#219):
//! `POST /api/scripts/run`. Both sides use these helpers, so the request
//! and answer shapes are spelled once.
//!
//! Request: the job without its bookkeeping, which stays with the editor:
//! `{source, width, height, inputs}`. Answer: `{ok: true, nodes,
//! elapsedMs}`, or `{ok: false, error, elapsedMs}` for a program that
//! failed, which is still an answer, not an HTTP error.

use std::collections::BTreeMap;

use jian_ops_schema::node::PenNode;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::script_regen::{ScriptJob, ScriptOutcome};

/// The request body for `job`.
pub fn script_run_request(job: &ScriptJob) -> String {
    json!({
        "source": job.source,
        "width": job.width,
        "height": job.height,
        "inputs": job.inputs,
    })
    .to_string()
}

/// A request body as the daemon reads it.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ScriptRunRequest {
    pub source: String,
    pub width: f64,
    pub height: f64,
    #[serde(default)]
    pub inputs: BTreeMap<String, Value>,
}

impl ScriptRunRequest {
    /// The job this request asks for, with placeholder bookkeeping: the
    /// daemon only runs it.
    pub fn into_job(self) -> ScriptJob {
        ScriptJob {
            node_id: crate::NodeId::new("script-run"),
            source: self.source,
            width: self.width,
            height: self.height,
            inputs: self.inputs,
            hash: String::new(),
        }
    }
}

/// The answer body for a run that produced `nodes` or failed with a
/// message.
pub fn script_run_answer(result: Result<&[PenNode], &str>, elapsed_ms: u64) -> String {
    match result {
        Ok(nodes) => json!({ "ok": true, "nodes": nodes, "elapsedMs": elapsed_ms }),
        Err(error) => json!({ "ok": false, "error": error, "elapsedMs": elapsed_ms }),
    }
    .to_string()
}

/// What the editor shows when no daemon answers (a static page, a
/// stopped daemon).
pub const SCRIPT_ENGINE_UNREACHABLE: &str =
    "script engine unreachable: run the editor through its daemon";

/// The daemon's answer as an outcome and its run time. HTTP status 0 is a
/// network failure; any answer that is not `{ok: true, nodes}` fails with
/// the daemon's message, or a generic one.
pub fn parse_script_run_answer(status: u16, text: &str) -> (ScriptOutcome, u64) {
    if status == 0 {
        return (
            ScriptOutcome::Failed(SCRIPT_ENGINE_UNREACHABLE.to_string()),
            0,
        );
    }
    let generic = || format!("script engine answered {status} without a result");
    let Ok(answer) = serde_json::from_str::<Value>(text) else {
        return (ScriptOutcome::Failed(generic()), 0);
    };
    let elapsed_ms = answer["elapsedMs"].as_u64().unwrap_or(0);
    if answer["ok"] == true {
        if let Ok(nodes) = serde_json::from_value::<Vec<PenNode>>(answer["nodes"].clone()) {
            return (ScriptOutcome::Generated(nodes), elapsed_ms);
        }
    }
    let message = answer["error"]
        .as_str()
        .map(str::to_string)
        .unwrap_or_else(generic);
    (ScriptOutcome::Failed(message), elapsed_ms)
}

#[cfg(test)]
#[path = "script_wire_tests.rs"]
mod tests;
