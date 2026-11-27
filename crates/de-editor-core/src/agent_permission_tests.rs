use super::*;
use crate::find_layers::tests::multi_page_state;
use de_i18n::Locale;

fn asking() -> EditorState {
    let mut state = multi_page_state();
    state
        .editor_ui
        .agent_settings
        .confirm_destructive_agent_actions = true;
    state
}

#[test]
fn destructive_calls_are_recognised() {
    use DestructiveAction as A;
    assert_eq!(
        destructive_action("delete_node", "{}"),
        Some(A::DeleteLayer)
    );
    assert_eq!(destructive_action("remove_page", "{}"), Some(A::DeletePage));
    assert_eq!(destructive_action("delete_page", "{}"), Some(A::DeletePage));
    assert_eq!(
        destructive_action("delete_component", "{}"),
        Some(A::DeleteComponent)
    );
    assert_eq!(
        destructive_action("delete_variable", "{}"),
        Some(A::DeleteVariable)
    );
    assert_eq!(
        destructive_action("replace_node", "{}"),
        Some(A::ReplaceLayer)
    );
    assert_eq!(
        destructive_action("save_document", "{}"),
        Some(A::WriteFile)
    );
    assert_eq!(destructive_action("export_nodes", "{}"), Some(A::WriteFile));
    let ops = |ops: &str| serde_json::json!({ "operations": ops }).to_string();
    assert_eq!(
        destructive_action(
            "batch_design",
            &ops("U(\"a\",{\"x\":1})\nD(\"b\")\nx=D(\"c\")")
        ),
        Some(A::DeleteInBatch { count: 2 })
    );
    assert_eq!(
        destructive_action("batch_design", &ops("U(\"aD(\",{})")),
        None
    );
    // A replacement is destructive too.
    assert_eq!(
        destructive_action("batch_design", &ops("R(\"a\",{\"type\":\"text\"})\nD(\"b\")")),
        Some(A::DeleteInBatch { count: 2 })
    );
    // A script is unknown until expanded; inserts through nodes_json never ask.
    let script = serde_json::json!({ "script": "ops.D('a')" }).to_string();
    assert_eq!(
        destructive_action("batch_design", &script),
        Some(A::ScriptedBatch)
    );
    let nodes = serde_json::json!({ "nodes_json": "[{\"kind\":\"rect\"}]" }).to_string();
    assert_eq!(destructive_action("batch_design", &nodes), None);
    assert_eq!(destructive_action("batch_design", "{}"), None);
    for safe in ["insert_node", "update_node", "batch_get", "move_node"] {
        assert_eq!(destructive_action(safe, "{}"), None, "{safe}");
    }
}

#[test]
fn prompts_name_targets_the_way_the_user_sees_them() {
    let state = multi_page_state();
    let layer = PermissionPrompt::new(
        &state,
        "delete_node",
        DestructiveAction::DeleteLayer,
        r#"{"nodeId":"cta"}"#,
    );
    assert_eq!(layer.target, "Sign in button");
    assert_eq!(
        layer.sentence(Locale::EnUs),
        "Let the agent delete layer “Sign in button”?"
    );
    let page = PermissionPrompt::new(
        &state,
        "remove_page",
        DestructiveAction::DeletePage,
        r#"{"pageId":"p2"}"#,
    );
    assert_eq!(page.target, "Settings");
    let gone = PermissionPrompt::new(
        &state,
        "delete_node",
        DestructiveAction::DeleteLayer,
        r#"{"nodeId":"zzz"}"#,
    );
    assert_eq!(gone.target, "zzz");
    let batch = PermissionPrompt::new(
        &state,
        "batch_design",
        DestructiveAction::DeleteInBatch { count: 3 },
        "{}",
    );
    assert_eq!(
        batch.sentence(Locale::EnUs),
        "Let the agent delete layers in one batch (3)?"
    );
}

#[test]
fn gate_runs_everything_while_the_setting_is_off() {
    let mut state = multi_page_state();
    assert_eq!(
        state.gate_agent_tool(None, "delete_node", r#"{"nodeId":"cta"}"#),
        ToolGate::Run
    );
    assert!(state.chat.permission_prompt.is_none());
}

#[test]
fn gate_waits_for_the_answer_then_runs_or_declines() {
    let mut state = asking();
    let args = r#"{"nodeId":"cta"}"#;
    assert_eq!(
        state.gate_agent_tool(None, "update_node", "{}"),
        ToolGate::Run
    );
    assert_eq!(
        state.gate_agent_tool(None, "delete_node", args),
        ToolGate::Asked
    );
    assert!(state.chat.permission_prompt.is_some());
    // Still waiting on the next pump; another call queues behind it.
    assert_eq!(
        state.gate_agent_tool(None, "delete_node", args),
        ToolGate::Wait
    );
    assert_eq!(
        state.gate_agent_tool(None, "delete_node", r#"{"nodeId":"title"}"#),
        ToolGate::Wait
    );
    assert!(state.answer_agent_permission(true));
    assert!(!state.answer_agent_permission(false), "answered once");
    assert_eq!(
        state.gate_agent_tool(None, "delete_node", args),
        ToolGate::Run
    );
    assert!(state.chat.permission_prompt.is_none());

    assert_eq!(
        state.gate_agent_tool(None, "remove_page", "{}"),
        ToolGate::Asked
    );
    state.answer_agent_permission(false);
    assert_eq!(
        state.gate_agent_tool(None, "remove_page", "{}"),
        ToolGate::Declined
    );
    assert!(state.chat.permission_prompt.is_none());
}

#[test]
fn stop_new_chat_and_a_new_send_drop_a_waiting_prompt() {
    let mut state = asking();
    state.gate_agent_tool(None, "delete_node", "{}");
    state.chat.stop_streaming();
    assert!(state.chat.permission_prompt.is_none());
    state.gate_agent_tool(None, "delete_node", "{}");
    state.chat.new_chat();
    assert!(state.chat.permission_prompt.is_none());
    state.gate_agent_tool(None, "delete_node", "{}");
    state.chat.input.set_text("never mind");
    state.chat.begin_send();
    assert!(state.chat.permission_prompt.is_none());
}

#[test]
fn declined_result_tells_the_agent_not_to_retry() {
    let value: serde_json::Value = serde_json::from_str(DECLINED_RESULT).unwrap();
    assert_eq!(value["success"], false);
    assert!(value["error"].as_str().unwrap().contains("declined"));
}

#[test]
fn an_expanded_script_is_counted_like_operations() {
    let script = serde_json::json!({ "script": "anything" }).to_string();
    assert_eq!(
        classify_with_expanded_script("batch_design", &script, |_| Some(
            "x=I(\"root\",{})\nD(\"old\")".into()
        )),
        Some(DestructiveAction::DeleteInBatch { count: 1 })
    );
    assert_eq!(
        classify_with_expanded_script("batch_design", &script, |_| Some("I(\"root\",{})".into())),
        None,
        "a script that only inserts runs at once"
    );
    assert_eq!(
        classify_with_expanded_script("batch_design", &script, |_| None),
        Some(DestructiveAction::ScriptedBatch)
    );
    assert_eq!(
        classify_with_expanded_script("delete_node", "{}", |_| None),
        Some(DestructiveAction::DeleteLayer)
    );
}

#[test]
fn the_sentence_is_made_once_per_prompt() {
    let state = multi_page_state();
    let prompt = PermissionPrompt::new(
        &state,
        "delete_node",
        DestructiveAction::DeleteLayer,
        r#"{"nodeId":"cta"}"#,
    );
    let locale = state.editor_ui.effective_locale();
    assert!(matches!(
        prompt.sentence_in(locale),
        std::borrow::Cow::Borrowed(_)
    ));
    let other = if locale == Locale::Ja { Locale::EnUs } else { Locale::Ja };
    assert!(matches!(
        prompt.sentence_in(other),
        std::borrow::Cow::Owned(_)
    ));
}

#[test]
fn a_hidden_prompt_is_noticed_and_an_ending_turn_declines_it() {
    let mut state = asking();
    assert!(!state.permission_prompt_hidden(None));
    state.chat.new_tab();
    assert!(state.permission_prompt_hidden(Some(0)), "on another tab");
    state.gate_agent_tool(Some(0), "delete_node", "{}");
    state.decline_agent_permission(Some(0));
    assert_eq!(
        state.gate_agent_tool(Some(0), "delete_node", "{}"),
        ToolGate::Declined
    );
}
