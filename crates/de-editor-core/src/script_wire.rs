//! The script-run contract between an editor without a script engine (the
//! CanvasKit web editor) and the daemon that runs the job for it (#219):
//! `POST /api/scripts/run`. Both sides use these helpers, so the request
//! and answer shapes are spelled once.
//!
//! Request: `{source, width, height, inputs}`. Answer: `{ok: true, nodes,
//! elapsedMs}`, or `{ok: false, error, elapsedMs}` for a program that
//! failed, which is still an answer, not an HTTP error.

use std::collections::BTreeMap;

use jian_ops_schema::node::PenNode;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::script_regen::{ScriptJob, ScriptOutcome};

/// A request body: the job without its bookkeeping (node id, hash), which
/// stays with the editor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

/// The request body for `job`.
pub fn script_run_request(job: &ScriptJob) -> String {
    let request = ScriptRunRequest {
        source: job.source.clone(),
        width: job.width,
        height: job.height,
        inputs: job.inputs.clone(),
    };
    serde_json::to_string(&request).unwrap_or_default()
}

/// An answer body.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ScriptRunAnswer {
    ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    nodes: Option<Vec<PenNode>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    #[serde(default)]
    elapsed_ms: u64,
}

/// The answer body for a finished run. `Unreachable` is the editor's own
/// verdict, never the daemon's; it is sent as a failure if it ever gets
/// here.
pub fn script_run_answer(outcome: ScriptOutcome, elapsed_ms: u64) -> String {
    let (ok, nodes, error) = match outcome {
        ScriptOutcome::Generated(nodes) => (true, Some(nodes), None),
        ScriptOutcome::Failed(message) => (false, None, Some(message)),
        ScriptOutcome::Unreachable => (false, None, Some("script engine unreachable".into())),
    };
    let answer = ScriptRunAnswer {
        ok,
        nodes,
        error,
        elapsed_ms,
    };
    serde_json::to_string(&answer).unwrap_or_default()
}

/// The daemon's answer as an outcome and its run time. No answer (status
/// 0), or a 404 / 405 that is not a script answer (a static page, a daemon
/// without the route), is `Unreachable`: nothing judged the program. Any
/// other answer that is not `{ok: true, nodes}` fails with the daemon's
/// message, or a generic one.
pub fn parse_script_run_answer(status: u16, text: &str) -> (ScriptOutcome, u64) {
    let answer = serde_json::from_str::<ScriptRunAnswer>(text).ok();
    let Some(answer) = answer else {
        if matches!(status, 0 | 404 | 405) {
            return (ScriptOutcome::Unreachable, 0);
        }
        let generic = format!("script engine answered {status} without a result");
        return (ScriptOutcome::Failed(generic), 0);
    };
    match answer {
        ScriptRunAnswer {
            ok: true,
            nodes: Some(nodes),
            elapsed_ms,
            ..
        } => (ScriptOutcome::Generated(nodes), elapsed_ms),
        ScriptRunAnswer {
            error, elapsed_ms, ..
        } => {
            let message = error
                .unwrap_or_else(|| format!("script engine answered {status} without a result"));
            (ScriptOutcome::Failed(message), elapsed_ms)
        }
    }
}

#[cfg(test)]
#[path = "script_wire_tests.rs"]
mod tests;
