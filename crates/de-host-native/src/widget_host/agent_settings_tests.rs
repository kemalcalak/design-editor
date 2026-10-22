//! Agent-settings modal press tests for the native host — shared
//! geometry fixtures plus the module spine.
//!
//! The grouped test bodies live in the sibling `agent_settings_tests/`
//! directory so every file stays under the repo's 800-line cap.

use super::WidgetHostNative;
use de_editor_core::agent_settings::{
    AcpAgentField, AgentSettingsTab, BuiltinAgentField, ImageGenField, ImageGenProvider,
    ImageSearchField, ImageTestStatus, SettingsFocus,
};
use de_editor_core::{AgentSettingsButton, BuiltinAgentPresetKey, ButtonPressTarget};
use de_editor_ui::widgets::agent_settings_panel::{AgentSettingsHit, AgentSettingsPanel};
use de_editor_ui::Point2D;

/// The settings modal is a wide, tall workspace; these fixtures press
/// absolute rects deep in the Agents tab, so they run in a window big
/// enough to keep the whole body above the fold.
pub(super) const VIEWPORT_W: f32 = 1200.0;
pub(super) const VIEWPORT_H: f32 = 1000.0;

mod agents;
mod hover;
mod images;
mod mcp_system;

fn agent_settings_content_metrics(host: &WidgetHostNative) -> (f32, f32, f32) {
    let panel = AgentSettingsPanel::for_editor(host.editor_state());
    let rect = panel.rect(VIEWPORT_W, VIEWPORT_H);
    (
        de_editor_ui::widgets::agent_settings_panel::content_viewport(rect)
            .origin
            .x,
        de_editor_ui::widgets::agent_settings_panel::content_viewport(rect)
            .origin
            .y,
        de_editor_ui::widgets::agent_settings_panel::content_viewport(rect)
            .size
            .x,
    )
}

/// Body metrics for the Images tab, whose content starts below the hero
/// the panel paints on its behalf.
fn agent_settings_images_metrics(host: &WidgetHostNative) -> (f32, f32, f32) {
    let panel = AgentSettingsPanel::for_editor(host.editor_state());
    let rect = panel.rect(VIEWPORT_W, VIEWPORT_H);
    let body = de_editor_ui::widgets::agent_settings_panel::secondary_tab_body(rect);
    (body.origin.x, body.origin.y, body.size.x)
}

fn acp_header_y(content_y: f32) -> f32 {
    content_y + de_editor_ui::widgets::agent_settings_panel::AGENTS_HERO_HEIGHT + 120.0 + 28.0
}

fn acp_card_y(content_y: f32) -> f32 {
    acp_header_y(content_y) + 28.0 + 28.0
}

fn experimental_switch_y(host: &WidgetHostNative, x: f32) -> f32 {
    let panel = AgentSettingsPanel::for_editor(host.editor_state());
    let rect = panel.rect(VIEWPORT_W, VIEWPORT_H);
    let mut hits = (rect.origin.y.ceil() as i32..(rect.origin.y + rect.size.y).floor() as i32)
        .map(|y| y as f32 + 0.5)
        .filter(|y| {
            matches!(
                panel.hit_test(rect, Point2D::new(x, *y)),
                AgentSettingsHit::ToggleExperimental
            )
        });
    let first = hits.next().expect("experimental switch hit region");
    let last = hits.next_back().unwrap_or(first);
    (first + last) / 2.0
}

#[test]
fn escape_blurs_focus_before_hiding_settings_and_releases_text_owner() {
    let mut host = WidgetHostNative::new();
    let original_port = host.editor_state().editor_ui.agent_settings.mcp_server.port;
    {
        let ui = &mut host.editor_state_mut().editor_ui;
        ui.agent_settings_open = true;
        ui.agent_settings.focus = Some(SettingsFocus::McpPort);
        ui.settings_input.set_text("4321");
    }

    // Settings follows the host's one-layer-per-Escape contract: first the
    // field (discarding its draft), then the modal.
    assert!(host.apply_escape());
    assert!(host.editor_state().editor_ui.agent_settings_open);
    assert_eq!(
        host.editor_state().editor_ui.agent_settings.mcp_server.port,
        original_port
    );
    assert!(host.editor_state().editor_ui.agent_settings.focus.is_none());
    assert!(host.apply_escape());

    let ui = &host.editor_state().editor_ui;
    assert!(!ui.agent_settings_open);
    assert!(ui.agent_settings.focus.is_none());
    assert!(!host.input_active());
    assert!(!host.apply_text('9'));
    assert_eq!(
        host.editor_state().editor_ui.agent_settings.mcp_server.port,
        original_port
    );
}
