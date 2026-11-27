//! Which agent tool calls ask first (#398), with `batch_design` scripts run
//! to their operations so their deletes are counted, not guessed
//! (`de_editor_core::agent_permission` holds the rules; core cannot run a
//! script).

use de_editor_core::agent_permission::{classify_with_expanded_script, DestructiveAction};

/// The destructive action of `tool(args_json)`, if any. A `batch_design`
/// script is expanded through the MCP script runner first; one that does
/// not expand stays a [`DestructiveAction::ScriptedBatch`] and asks.
pub fn classify_agent_tool(tool: &str, args_json: &str) -> Option<DestructiveAction> {
    classify_with_expanded_script(tool, args_json, |script| {
        de_mcp::script_runner::run_script_to_program_with_notes(script)
            .ok()
            .map(|program| program.program)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_script_is_counted_by_what_it_expands_to() {
        let script = |body: &str| serde_json::json!({ "script": body }).to_string();
        // An unparsable script cannot be counted, so it asks.
        assert_eq!(
            classify_agent_tool("batch_design", &script("this is not javascript (")),
            Some(DestructiveAction::ScriptedBatch)
        );
        assert_eq!(
            classify_agent_tool("delete_node", "{}"),
            Some(DestructiveAction::DeleteLayer)
        );
        assert_eq!(classify_agent_tool("update_node", "{}"), None);
    }
}
