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

/// An `execute` call judged by a dry run of its script on `snapshot`
/// (nothing applied, rendered or written): `None` when it would only read or
/// make plain writes, [`DestructiveAction::ScriptedBatch`] when it would
/// delete, replace, export or overwrite the variables, or when it cannot be
/// judged (it throws, a write fails, it runs out of time).
pub fn classify_execute(
    snapshot: de_editor_core::shared_state::SharedState,
    args_json: &str,
) -> Option<DestructiveAction> {
    let args: serde_json::Value = serde_json::from_str(args_json).unwrap_or_default();
    let script = args.get("script").and_then(serde_json::Value::as_str)?;
    let page_id = args
        .get("pageId")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    match de_mcp::script_execute::execute_effects(snapshot, script, page_id) {
        Some(effects) if !effects.is_destructive() => None,
        _ => Some(DestructiveAction::ScriptedBatch),
    }
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
            classify_execute(SharedState::copied(&state), &args)
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
