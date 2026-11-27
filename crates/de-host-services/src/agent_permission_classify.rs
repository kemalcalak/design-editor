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

/// An `execute` call's dry run: what it did, and whether it asks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecuteVerdict {
    /// The destructive calls it made; `None` when it could not be judged.
    pub effects: Option<de_mcp::script_execute::ExecuteEffects>,
    /// [`DestructiveAction::ScriptedBatch`] when it would delete, replace,
    /// export or overwrite the variables, or could not be judged (it threw,
    /// a write failed, it ran out of time); `None` when it only reads or
    /// makes plain writes.
    pub action: Option<DestructiveAction>,
}

/// Judge an `execute` call by a dry run of its script on `snapshot` on
/// `clock` (nothing applied, rendered or written). The real run must use
/// the same clock (`de_mcp::script_execute::pinned`) to do the same.
pub fn classify_execute(
    snapshot: de_editor_core::shared_state::SharedState,
    args_json: &str,
    clock: de_mcp::script_execute::ExecuteClock,
) -> ExecuteVerdict {
    let args: serde_json::Value = serde_json::from_str(args_json).unwrap_or_default();
    let Some(script) = args.get("script").and_then(serde_json::Value::as_str) else {
        // No script: the call fails without running anything.
        return ExecuteVerdict {
            effects: Some(Default::default()),
            action: None,
        };
    };
    let page_id = args
        .get("pageId")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    let effects = de_mcp::script_execute::execute_effects(snapshot, script, page_id, clock);
    let action = match &effects {
        Some(effects) if !effects.is_destructive() => None,
        _ => Some(DestructiveAction::ScriptedBatch),
    };
    ExecuteVerdict { effects, action }
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

    #[test]
    fn an_execute_script_is_judged_by_what_it_runs() {
        use de_editor_core::shared_state::SharedState;
        let doc = jian_ops_schema::load_str(
            r#"{"version":"1.0.0","children":[
              {"type":"frame","id":"hero","name":"Hero","x":0,"y":0,"width":100,"height":100}
            ]}"#,
        )
        .unwrap()
        .value;
        let state = de_editor_core::EditorState::from_document(doc);
        let run = |script: &str| {
            let args = serde_json::json!({ "script": script }).to_string();
            let clock = de_mcp::script_execute::ExecuteClock::now();
            classify_execute(SharedState::copied(&state), &args, clock).action
        };
        for asks in [
            "globalThis.Delete('hero')",
            "this.Delete('hero')",
            "globalThis['Del' + 'ete']('hero')",
            "x.Delete()",
        ] {
            assert_eq!(run(asks), Some(DestructiveAction::ScriptedBatch), "{asks}");
        }
        assert_eq!(run("Get('hero'); Print('ok')"), None);
        assert_eq!(run("Update('hero', {name: 'Top'})"), None);
    }
}
