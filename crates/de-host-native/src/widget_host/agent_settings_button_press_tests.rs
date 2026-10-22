//! Press feedback for the Agents tab's footer buttons (#26): a press sets
//! the shared `pressed_button` target and the release clears it.

use super::agent_settings_tests::{press_hit, VIEWPORT_H, VIEWPORT_W};
use super::WidgetHostNative;
use de_editor_core::{AgentSettingsButton, BuiltinAgentPresetKey, ButtonPressTarget};
use de_editor_ui::widgets::agent_settings_panel::AgentSettingsHit;

fn assert_press_feedback(
    host: &mut WidgetHostNative,
    hit: AgentSettingsHit,
    button: AgentSettingsButton,
) {
    assert!(press_hit(host, hit));
    assert_eq!(
        host.editor_state().editor_ui.pressed_button,
        Some(ButtonPressTarget::AgentSettings(button))
    );
    assert!(host.apply_release_with_viewport(VIEWPORT_W, VIEWPORT_H));
    assert_eq!(host.editor_state().editor_ui.pressed_button, None);
}

#[test]
fn builtin_remove_press_sets_and_release_clears_its_button() {
    let mut host = WidgetHostNative::new();
    host.editor_state_mut()
        .editor_ui
        .agent_settings
        .add_builtin_agent_with_defaults("MiniMax", "sk-test", "MiniMax-M2.7");
    assert_press_feedback(
        &mut host,
        AgentSettingsHit::RemoveBuiltinAgent(0),
        AgentSettingsButton::BuiltinRemove(0),
    );
    assert!(host
        .editor_state()
        .editor_ui
        .agent_settings
        .builtin_agents
        .is_empty());
}

#[test]
fn builtin_draft_save_and_cancel_press_feedback() {
    let mut host = WidgetHostNative::new();
    assert!(press_hit(
        &mut host,
        AgentSettingsHit::SelectApiPreset(BuiltinAgentPresetKey::OpenAi)
    ));
    assert_press_feedback(
        &mut host,
        AgentSettingsHit::CancelBuiltinAgentDraft,
        AgentSettingsButton::BuiltinCancelDraft,
    );
    assert!(press_hit(
        &mut host,
        AgentSettingsHit::SelectApiPreset(BuiltinAgentPresetKey::OpenAi)
    ));
    assert_press_feedback(
        &mut host,
        AgentSettingsHit::SaveBuiltinAgentDraft,
        AgentSettingsButton::BuiltinSaveDraft,
    );
}

#[test]
fn acp_footer_buttons_press_feedback() {
    let mut host = WidgetHostNative::new();
    assert!(press_hit(&mut host, AgentSettingsHit::AddAcpAgent));
    assert_press_feedback(
        &mut host,
        AgentSettingsHit::CancelAcpAgentDraft,
        AgentSettingsButton::AcpCancelDraft,
    );
    assert!(press_hit(&mut host, AgentSettingsHit::AddAcpAgent));
    assert_press_feedback(
        &mut host,
        AgentSettingsHit::SaveAcpAgentDraft,
        AgentSettingsButton::AcpSaveDraft,
    );

    host.editor_state_mut()
        .editor_ui
        .agent_settings
        .add_acp_agent();
    assert!(press_hit(&mut host, AgentSettingsHit::SelectAcpEntry(0)));
    assert_press_feedback(
        &mut host,
        AgentSettingsHit::RemoveAcpAgent(0),
        AgentSettingsButton::AcpRemove(0),
    );
}
