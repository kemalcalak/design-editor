//! `POST /api/scripts/run` (#219): the web editor's script engine. The
//! browser bundle has no QuickJS, so the CanvasKit editor sends each stale
//! script node's job here and lands the answer itself. The run is the
//! desktop's (`de_mcp::script_node`: 1 s, 16 MiB, 2000 nodes, seeded
//! `Math.random`, nodes normalized), so both editors generate the same
//! children. Pure: it never touches the document or the state lock, and it
//! runs on the connection's own thread.

use de_editor_core::script_wire::{script_run_answer, ScriptRunRequest};

use super::WebReply;
use crate::mcp_serve::rest_error_body;

/// The reply to a run request. A program that fails is still a `200`
/// answer (`ok: false` with the message the panel shows); only a body that
/// is not a job is a `400`.
pub(super) fn run_script(body: &str) -> WebReply {
    let request: ScriptRunRequest = match serde_json::from_str(body) {
        Ok(request) => request,
        Err(error) => {
            return WebReply {
                status: "400 Bad Request",
                body: rest_error_body(&format!("invalid script job: {error}")),
            };
        }
    };
    let run = de_mcp::script_node::run_script_job(&request.into_job());
    let elapsed_ms = run.elapsed.as_millis() as u64;
    WebReply {
        status: "200 OK",
        body: script_run_answer(run.outcome(), elapsed_ms),
    }
}

#[cfg(test)]
#[path = "script_routes_tests.rs"]
mod tests;
