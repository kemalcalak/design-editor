//! ACP agent detail presses on the native host (#26): fields, remove and
//! the connect lifecycle, pressed through `press_hit`.

use super::agent_settings_tests::press_hit;
use super::WidgetHostNative;
use de_editor_core::agent_settings::{AcpAgentField, SettingsFocus};
use de_editor_core::{AgentSettingsButton, ButtonPressTarget};
use de_editor_ui::widgets::agent_settings_panel::AgentSettingsHit;

#[test]
fn acp_agent_command_field_accepts_text_and_commits() {
    let mut host = WidgetHostNative::new();
    host.editor_state_mut()
        .editor_ui
        .agent_settings
        .add_acp_agent();
    assert!(press_hit(&mut host, AgentSettingsHit::SelectAcpEntry(0)));
    assert!(press_hit(
        &mut host,
        AgentSettingsHit::FocusAcpAgent {
            index: 0,
            field: AcpAgentField::Command,
        }
    ));
    for c in "op-agent".chars() {
        assert!(host.apply_text(c));
    }
    assert!(host.apply_send());

    let settings = &host.editor_state().editor_ui.agent_settings;
    assert_eq!(settings.acp_agents[0].command, "op-agent");
    assert!(settings.focus.is_none());
    assert!(host
        .editor_state()
        .editor_ui
        .settings_input
        .text()
        .is_empty());
}

#[test]
fn acp_agent_args_field_commits_comma_separated_args() {
    let mut host = WidgetHostNative::new();
    host.editor_state_mut()
        .editor_ui
        .agent_settings
        .add_acp_agent();
    host.editor_state_mut().editor_ui.agent_settings.focus = Some(SettingsFocus::AcpAgent {
        index: 0,
        field: AcpAgentField::Args,
    });
    host.editor_state_mut()
        .editor_ui
        .settings_input
        .set_text(" --stdio, --workspace /tmp, , --verbose ");

    assert!(host.apply_send());

    let settings = &host.editor_state().editor_ui.agent_settings;
    assert_eq!(
        settings.acp_agents[0].args,
        vec!["--stdio", "--workspace /tmp", "--verbose"]
    );
    assert!(settings.focus.is_none());
    assert!(host
        .editor_state()
        .editor_ui
        .settings_input
        .text()
        .is_empty());
}

#[test]
fn acp_agent_remove_press_deletes_agent_and_clears_focus() {
    let mut host = WidgetHostNative::new();
    host.editor_state_mut()
        .editor_ui
        .agent_settings
        .add_acp_agent();
    assert!(press_hit(&mut host, AgentSettingsHit::SelectAcpEntry(0)));
    assert!(press_hit(&mut host, AgentSettingsHit::RemoveAcpAgent(0)));

    let settings = &host.editor_state().editor_ui.agent_settings;
    assert!(settings.acp_agents.is_empty());
    assert!(settings.focus.is_none());
    assert!(host
        .editor_state()
        .editor_ui
        .settings_input
        .text()
        .is_empty());
}

#[test]
fn acp_agent_connect_press_starts_real_probe_request() {
    let mut host = WidgetHostNative::new();
    host.editor_state_mut()
        .editor_ui
        .agent_settings
        .add_acp_agent();
    host.editor_state_mut().editor_ui.agent_settings.acp_agents[0].command = "op-agent".into();

    assert!(press_hit(&mut host, AgentSettingsHit::SelectAcpEntry(0)));
    assert!(press_hit(
        &mut host,
        AgentSettingsHit::ToggleAcpConnected(0)
    ));
    assert_eq!(
        host.editor_state().editor_ui.pressed_button,
        Some(ButtonPressTarget::AgentSettings(
            AgentSettingsButton::AcpConnection(0)
        ))
    );
    assert!(
        !host.editor_state().editor_ui.agent_settings.acp_agents[0].connected,
        "configured local ACP agent must wait for a real probe before becoming connected"
    );
    assert_eq!(
        host.editor_state()
            .editor_ui
            .agent_settings
            .pending_acp_agent_connect
            .as_ref()
            .map(|request| request.id.as_str()),
        Some("acp-1")
    );
    assert!(host.apply_release_with_viewport(
        super::agent_settings_tests::VIEWPORT_W,
        super::agent_settings_tests::VIEWPORT_H
    ));
    assert_eq!(host.editor_state().editor_ui.pressed_button, None);

    host.editor_state_mut()
        .editor_ui
        .agent_settings
        .apply_acp_agent_connect_outcome(
            "acp-1",
            de_editor_core::AcpAgentConnectOutcome {
                connected: true,
                info: Some("Test Agent".into()),
                ..de_editor_core::AcpAgentConnectOutcome::default()
            },
        );
    assert!(press_hit(
        &mut host,
        AgentSettingsHit::ToggleAcpConnected(0)
    ));
    assert!(!host.editor_state().editor_ui.agent_settings.acp_agents[0].connected);
    assert!(host.apply_release_with_viewport(
        super::agent_settings_tests::VIEWPORT_W,
        super::agent_settings_tests::VIEWPORT_H
    ));
    assert_eq!(host.editor_state().editor_ui.pressed_button, None);
}
