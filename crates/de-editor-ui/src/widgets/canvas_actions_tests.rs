use super::*;
use de_editor_core::{BuiltinAgentConfig, BuiltinAgentKind, BuiltinAgentPresetKey};

fn ui() -> EditorUiState {
    EditorUiState {
        locale: de_editor_core::Locale::EnUs,
        ..EditorUiState::default()
    }
}

fn ready_agent() -> BuiltinAgentConfig {
    BuiltinAgentConfig {
        id: "anthropic".into(),
        preset: BuiltinAgentPresetKey::Anthropic,
        display_name: "Anthropic".into(),
        kind: BuiltinAgentKind::Anthropic,
        api_key: "sk-test".into(),
        models: vec!["claude".into()],
        base_url: "https://api.anthropic.com".into(),
        enabled: true,
    }
}

#[test]
fn nothing_live_reads_as_the_set_up_label() {
    let actions = CanvasActions::for_editor_ui(&ui());
    assert!(!actions.status.live());
    assert_eq!(actions.status_text, "Agents & MCP");
}

#[test]
fn ready_builtin_agents_and_mcp_clis_both_count() {
    let mut ui = ui();
    ui.agent_settings.builtin_agents = vec![ready_agent(), ready_agent()];
    ui.agent_settings.mcp_cli_enabled[0] = true;
    let actions = CanvasActions::for_editor_ui(&ui);
    assert_eq!(actions.status, AgentStatus { agents: 2, mcp: 1 });
    assert_eq!(actions.status_text, "2 agents · 1 MCP");
    ui.agent_settings.builtin_agents.truncate(1);
    ui.agent_settings.mcp_cli_enabled[0] = false;
    assert_eq!(CanvasActions::for_editor_ui(&ui).status_text, "1 agent");
}

#[test]
fn the_chip_opens_settings_and_leads_the_group() {
    let actions = CanvasActions::for_editor_ui(&ui());
    let layout = actions.layout_right(1400.0, 14.0, false);
    let centre = |r: Rect| Point2D::new(r.origin.x + r.size.x / 2.0, r.origin.y + r.size.y / 2.0);
    assert_eq!(
        actions.hit_test(&layout, centre(layout.status)),
        Some(ChromeHit::OpenAgentSettings)
    );
    assert!(layout.status.origin.x + layout.status.size.x < layout.code.origin.x);
    assert_eq!(
        actions.hit_test(&layout, centre(layout.code)),
        Some(ChromeHit::ToggleCodeView)
    );
}

#[test]
fn compact_drops_the_text_but_keeps_the_button() {
    let actions = CanvasActions::for_editor_ui(&ui());
    assert!(actions.width(true) < actions.width(false));
    let layout = actions.layout_right(1400.0, 14.0, true);
    assert!(layout.status_compact);
    assert_eq!(layout.status.size.x, BUTTON);
    assert_eq!(layout.rect.origin.x + layout.rect.size.x, 1400.0);
}

#[test]
fn a_chip_press_opens_settings_through_the_shared_flow() {
    use crate::widgets::press_flow::{apply_shared_chrome_hit, ChromePress};
    let mut state = de_editor_core::EditorState::default();
    let press = apply_shared_chrome_hit(&mut state, ChromeHit::OpenAgentSettings, 1);
    assert_eq!(press, ChromePress::SettingsOpened);
    assert!(state.editor_ui.agent_settings_open);
}
