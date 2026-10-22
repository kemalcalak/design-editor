//! Focused Agent Settings input geometry.
//!
//! These are unscrolled document-space rects. Hosts use them to place the
//! caret in the active field without duplicating the form geometry owned by
//! the shared UI widgets.

use crate::widgets::agent_settings_builtin_layout::{
    card_height_for_ui, card_rect, draft_card_height_for_ui, field_input_rect_for_ui,
    sync_error_height, CARD_GAP, HEADER_HEIGHT, SUBTITLE_HEIGHT,
};
use crate::widgets::agent_settings_images;
use crate::widgets::agent_settings_panel_geometry::{agents_body_top, hero_body_rect_for_ui};
use crate::Rect;
use de_editor_core::agent_settings::{
    AgentSettings, AgentSettingsTab, BuiltinAgentField, SettingsFocus,
};

fn builtin_field_row(field: BuiltinAgentField) -> usize {
    match field {
        BuiltinAgentField::DisplayName => 0,
        BuiltinAgentField::ApiKey => 1,
        BuiltinAgentField::Model => 2,
        BuiltinAgentField::BaseUrl => 3,
    }
}

fn builtin_card_and_field(
    content: Rect,
    settings: &AgentSettings,
    focus: SettingsFocus,
) -> Option<(Rect, Option<usize>, BuiltinAgentField)> {
    let mut card_y =
        agents_body_top(content) + HEADER_HEIGHT + SUBTITLE_HEIGHT + sync_error_height(settings);
    match focus {
        SettingsFocus::BuiltinAgent { index, field } => {
            for previous in 0..index {
                card_y += card_height_for_ui(settings, previous) + CARD_GAP;
            }
            settings.builtin_agents.get(index)?;
            let card = card_rect(
                content.origin.x,
                card_y,
                content.size.x,
                card_height_for_ui(settings, index),
            );
            Some((card, Some(index), field))
        }
        SettingsFocus::BuiltinAgentDraft(field) => {
            for index in 0..settings.builtin_agents.len() {
                card_y += card_height_for_ui(settings, index) + CARD_GAP;
            }
            settings.builtin_agent_draft.as_ref()?;
            let card = card_rect(
                content.origin.x,
                card_y,
                content.size.x,
                draft_card_height_for_ui(settings),
            );
            Some((card, None, field))
        }
        _ => None,
    }
}

fn builtin_input_rect(
    content: Rect,
    settings: &AgentSettings,
    focus: SettingsFocus,
) -> Option<Rect> {
    let (card, index, field) = builtin_card_and_field(content, settings, focus)?;
    Some(field_input_rect_for_ui(
        settings,
        card,
        index,
        builtin_field_row(field),
    ))
}

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
        AgentSettingsTab::Agents => builtin_input_rect(content, settings, focus),
        AgentSettingsTab::Mcp if focus == SettingsFocus::McpPort => {
            Some(crate::widgets::agent_settings_mcp::port_field_rect(content))
        }
        AgentSettingsTab::Images => image_input_rect(content, settings, focus),
        AgentSettingsTab::Fonts | AgentSettingsTab::System => None,
        AgentSettingsTab::Mcp => None,
    }
}
