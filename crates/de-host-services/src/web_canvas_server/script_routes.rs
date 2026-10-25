//! `POST /api/scripts/run` (#219): the web editor's script engine. The
//! browser bundle has no QuickJS, so the CanvasKit editor sends each stale
//! script node's job here and lands the answer itself. The run is the
//! desktop's (`de_mcp::script_node`: 1 s, 16 MiB, 2000 nodes, seeded
//! `Math.random`, nodes normalized), so both editors generate the same
//! children. Pure: it never touches the document or the state lock, and it
//! runs on the connection's own thread.

use de_editor_core::script_wire::{script_run_answer, ScriptRunRequest};
use serde_json::json;

/// The HTTP status and JSON body for a run request. A program that fails
/// is still a `200` answer (`ok: false` with the message the panel shows);
/// only a body that is not a job is a `400`.
pub(super) fn run_script(body: &str) -> (&'static str, String) {
    let request: ScriptRunRequest = match serde_json::from_str(body) {
        Ok(request) => request,
        Err(error) => {
            let body = json!({ "ok": false, "error": format!("invalid script job: {error}") });
            return ("400 Bad Request", body.to_string());
        }
    };
    let run = de_mcp::script_node::run_script_job(&request.into_job());
    let elapsed_ms = run.elapsed.as_millis() as u64;
    let body = match &run.result {
        Ok(nodes) => script_run_answer(Ok(nodes), elapsed_ms),
        Err(error) => script_run_answer(Err(&error.to_string()), elapsed_ms),
    };
    ("200 OK", body)
}

#[cfg(test)]
#[path = "script_routes_tests.rs"]
mod tests;
