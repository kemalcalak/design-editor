//! `debug_memory_report` (#456): what the editor process holds, for a
//! field report or an agent checking a large document.
//!
//! Three parts, each read without touching the UI thread: the process
//! footprint (`de_editor_core::process_memory`, read now), the undo
//! history's estimate (walked here, on the MCP connection thread, from the
//! request snapshot's history; seconds on #412's document), and the host's
//! cache lines as the desktop last published them
//! (`de_editor_core::memory_reading`, refreshed on a timer, never from
//! paint). A server without a canvas (`--mcp` on a file) publishes no cache
//! lines; the result says so. Gated behind `DESIGN_EDITOR_DEBUG_TOOLS=1`
//! like the other debug tools.

use std::collections::BTreeMap;

use de_editor_core::history_snapshot::SharedDoc;
use de_editor_core::memory_reading;
use de_editor_core::memory_report;
use de_editor_core::process_memory;
use de_editor_core::EditorState;
use serde_json::json;

use super::debug_tools::debug_tools_enabled;
use super::{McpTool, ToolErrorCode, ToolOutcome};

pub struct DebugMemoryReport {
    /// The request snapshot's undo / redo documents (`Arc` clones).
    history: Vec<(SharedDoc, usize)>,
}

pub fn debug_memory_report_snapshot(state: &EditorState) -> DebugMemoryReport {
    DebugMemoryReport {
        history: memory_report::history_docs(state),
    }
}

impl McpTool for DebugMemoryReport {
    fn name(&self) -> &str {
        "debug_memory_report"
    }

    fn call(&self, _args: &BTreeMap<String, String>) -> ToolOutcome {
        if !debug_tools_enabled() {
            return ToolOutcome::Err(
                ToolErrorCode::ToolFailed,
                "debug tools are disabled — set DESIGN_EDITOR_DEBUG_TOOLS=1 to enable".into(),
            );
        }
        let process = process_memory::current();
        let history = memory_report::history_usage_of(&self.history);
        let reading = memory_reading::latest();
        let host = reading.measured.then(|| reading.caches.to_json());
        let mut out = json!({
            "process": {
                "residentBytes": process.resident,
                "footprintBytes": process.footprint,
            },
            "undoHistory": {
                "bytes": history.bytes,
                "entries": history.entries,
                "exact": history.exact,
            },
            "hostCaches": host,
        });
        if !reading.measured {
            out["note"] = json!(
                "no host cache lines: this server has no canvas (or the editor has not published yet)"
            );
        }
        ToolOutcome::OkJson(out.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_process_history_and_host_lines() {
        let _gate = super::super::debug_tools::debug_gate_override::Guard::forcing(true);
        let mut state = EditorState::new();
        state.commit_history();
        let ToolOutcome::OkJson(json) =
            debug_memory_report_snapshot(&state).call(&BTreeMap::new())
        else {
            panic!("expected a JSON report");
        };
        let value: serde_json::Value = serde_json::from_str(&json).expect("json");
        assert_eq!(value["undoHistory"]["entries"], 1, "{json}");
        assert!(value["process"].is_object(), "{json}");
        assert!(value.get("hostCaches").is_some(), "{json}");
    }

    #[test]
    fn disabled_without_the_debug_flag() {
        let _gate = super::super::debug_tools::debug_gate_override::Guard::forcing(false);
        let outcome = debug_memory_report_snapshot(&EditorState::new()).call(&BTreeMap::new());
        assert!(matches!(outcome, ToolOutcome::Err(ToolErrorCode::ToolFailed, _)));
    }
}
