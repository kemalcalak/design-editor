//! Focused Agent Settings input geometry.
//!
//! These are unscrolled document-space rects. Hosts use them to place the
//! caret in the active field without duplicating the form geometry owned by
//! the shared UI widgets.

use crate::widgets::agent_settings_images;
use crate::widgets::agent_settings_panel_geometry::hero_body_rect_for_ui;
use crate::Rect;
use de_editor_core::agent_settings::{AgentSettings, AgentSettingsTab, SettingsFocus};

fn image_input_rect(content: Rect, settings: &AgentSettings, focus: SettingsFocus) -> Option<Rect> {
    let body = hero_body_rect_for_ui(content);
    agent_settings_images::focused_input_rect_for_ui(body, settings, focus)
}

pub(super) fn focused_input_rect(
    content: Rect,
    settings: &AgentSettings,
    active_tab: AgentSettingsTab,
) -> Option<Rect> {
    let focus = settings.focus?;
    match active_tab {
        // The Agents tab resolves through `AgentsTab::focused_input_rect`.
        AgentSettingsTab::Agents => None,
        AgentSettingsTab::Mcp if focus == SettingsFocus::McpPort => {
            Some(crate::widgets::agent_settings_mcp::port_field_rect(content))
        }
        AgentSettingsTab::Images => image_input_rect(content, settings, focus),
        AgentSettingsTab::Fonts | AgentSettingsTab::System => None,
        AgentSettingsTab::Mcp => None,
    }
}
