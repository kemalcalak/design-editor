//! The install command's copy button (#304): pressed through the panel's
//! own geometry, at the full-size modal and at its narrowest width, it
//! puts the exact command on the desktop clipboard queue.

use super::super::WidgetHostNative;
use de_editor_core::agent_settings::{AgentsEntry, ProviderConnectPhase};
use de_editor_core::AgentProvider;
use de_editor_ui::widgets::agent_settings_panel::{AgentSettingsHit, AgentSettingsPanel};

const CLAUDE_INSTALL: &str = "npm install -g @anthropic-ai/claude-code";
const KIMI_INSTALL: &str = "curl -fsSL https://code.kimi.com/kimi-code/install.sh | bash";

/// Press `hit` in a `w`×`h` window, scrolling until it shows.
fn press_at(host: &mut WidgetHostNative, hit: AgentSettingsHit, w: f32, h: f32) -> bool {
    let mut scroll = 0.0_f32;
    loop {
        host.editor_state_mut()
            .editor_ui
            .agent_settings
            .scroll_y
            .offset = scroll;
        let (point, max) = {
            let panel = AgentSettingsPanel::for_editor(host.editor_state());
            let rect = panel.rect(w, h);
            (panel.point_for_hit(rect, hit), panel.max_scroll(rect))
        };
        if let Some(point) = point {
            return host.dispatch_agent_settings_press(point.x, point.y, w, h);
        }
        assert!(scroll < max, "no scroll offset shows {hit:?} at {w}×{h}");
        scroll = (scroll + 24.0).min(max);
    }
}

fn missing_claude() -> WidgetHostNative {
    let mut host = WidgetHostNative::new();
    host.set_now_ms(5_000);
    let state = host.editor_state_mut();
    state.editor_ui.agent_settings_open = true;
    let settings = &mut state.editor_ui.agent_settings;
    settings.select_agents_entry(AgentsEntry::Cli(AgentProvider::ClaudeCode));
    let connection = &mut settings.provider_connection[AgentProvider::ClaudeCode.index()];
    connection.phase = ProviderConnectPhase::Error;
    connection.not_installed = true;
    connection.install_command = Some(CLAUDE_INSTALL.into());
    host
}

#[test]
fn copying_a_missing_cli_agents_install_command_queues_the_exact_text() {
    // Full-size modal, then the 640 px window floor (the modal's minimum).
    for (w, h) in [(1200.0, 1000.0), (640.0, 400.0)] {
        let mut host = missing_claude();
        assert!(press_at(&mut host, AgentSettingsHit::CopyInstallCommand, w, h));
        let state = host.editor_state();
        assert_eq!(
            state.chat.pending_copy_text.as_deref(),
            Some(CLAUDE_INSTALL),
            "{w}×{h}"
        );
        assert_eq!(
            state.editor_ui.agent_settings.install_command_copied,
            Some((CLAUDE_INSTALL.to_string(), 5_000))
        );
        // The rest of the card still answers to its own controls.
        assert!(press_at(
            &mut host,
            AgentSettingsHit::RecheckProvider(AgentProvider::ClaudeCode),
            w,
            h
        ));
    }
}

#[test]
fn an_installed_agent_offers_no_copy_button() {
    let mut host = missing_claude();
    let connection = &mut host.editor_state_mut().editor_ui.agent_settings.provider_connection
        [AgentProvider::ClaudeCode.index()];
    connection.not_installed = false;
    connection.phase = ProviderConnectPhase::Idle;
    let panel = AgentSettingsPanel::for_editor(host.editor_state());
    let rect = panel.rect(1200.0, 1000.0);
    assert_eq!(
        panel.point_for_hit(rect, AgentSettingsHit::CopyInstallCommand),
        None
    );
}

#[test]
fn copying_a_missing_acp_presets_install_hint_queues_the_exact_text() {
    let mut host = WidgetHostNative::new();
    let state = host.editor_state_mut();
    state.editor_ui.agent_settings_open = true;
    let settings = &mut state.editor_ui.agent_settings;
    settings.acp_preset_installed.insert("kimi".into(), false);
    settings.begin_acp_agent_draft();
    settings.fill_acp_draft_from_preset(Some("kimi"));
    settings.select_agents_entry(AgentsEntry::AcpDraft);
    for (w, h) in [(1200.0, 1000.0), (640.0, 400.0)] {
        host.editor_state_mut().chat.pending_copy_text = None;
        assert!(press_at(&mut host, AgentSettingsHit::CopyInstallCommand, w, h));
        assert_eq!(
            host.editor_state().chat.pending_copy_text.as_deref(),
            Some(KIMI_INSTALL),
            "{w}×{h}"
        );
    }
}
