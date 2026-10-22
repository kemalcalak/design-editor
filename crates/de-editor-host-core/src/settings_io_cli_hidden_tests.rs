//! Hidden CLI model persistence (#26 D6) — sibling of `settings_io_tests.rs`
//! (that file sits at the 800-line cap).

use super::*;

#[test]
fn hidden_cli_models_round_trip_by_provider_name_and_change_fingerprint() {
    let mut src = EditorState::new();
    let before = fingerprint(&src);
    src.editor_ui
        .agent_settings
        .toggle_cli_model_hidden(de_editor_core::AgentProvider::CodexCli, "gpt-5.5");
    assert_ne!(before, fingerprint(&src));

    let json = serde_json::to_string(&to_payload(&src)).unwrap();
    assert!(
        json.contains("\"cli_hidden_models\":{\"Codex CLI\":[\"gpt-5.5\"]}"),
        "hidden models are keyed by the provider's stable name: {json}"
    );
    let payload: SettingsPayload = serde_json::from_str(&json).unwrap();
    let mut dst = EditorState::new();
    apply_payload(&mut dst, payload);
    assert!(dst
        .editor_ui
        .agent_settings
        .cli_model_hidden(de_editor_core::AgentProvider::CodexCli, "gpt-5.5"));
}

#[test]
fn hidden_cli_models_from_an_unknown_provider_are_dropped() {
    let json = r#"{"version":1,"cli_hidden_models":{"Future Agent":["m"],"Claude Code":["opus"]}}"#;
    let payload: SettingsPayload = serde_json::from_str(json).unwrap();
    let mut dst = EditorState::new();
    apply_payload(&mut dst, payload);
    let hidden = &dst.editor_ui.agent_settings.cli_hidden_models;
    assert_eq!(hidden.iter().map(|set| set.len()).sum::<usize>(), 1);
    assert!(dst
        .editor_ui
        .agent_settings
        .cli_model_hidden(de_editor_core::AgentProvider::ClaudeCode, "opus"));
}
