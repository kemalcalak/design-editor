//! Geometry helpers shared by the agent-settings panel paint and hit-test paths.

use crate::widgets::agent_settings_i18n::t as t_settings;
use crate::widgets::agent_settings_panel::{
    CONTENT_BOTTOM_PAD, HEADER_HEIGHT, NAV_ITEM_GAP, NAV_ITEM_H, NAV_TITLE_H, NAV_W, PAD,
};
use crate::{Point2D, Rect};
use de_editor_core::agent_settings::AgentSettingsTab;
use de_editor_core::editor_ui_state::EditorUiState;

pub(super) fn tab_i18n_label(ui: &EditorUiState, tab: AgentSettingsTab) -> &'static str {
    match tab {
        AgentSettingsTab::Agents => t_settings(ui, "settings.tab.agents"),
        AgentSettingsTab::Mcp => t_settings(ui, "settings.tab.mcp"),
        AgentSettingsTab::Images => t_settings(ui, "settings.tab.images"),
        AgentSettingsTab::Fonts => t_settings(ui, "settings.tab.fonts"),
        AgentSettingsTab::System => t_settings(ui, "settings.tab.system"),
    }
}

pub(super) fn content_rect(panel: Rect) -> Rect {
    let x = panel.origin.x + NAV_W + PAD;
    Rect {
        origin: Point2D::new(x, panel.origin.y + HEADER_HEIGHT),
        size: Point2D::new(
            (panel.size.x - NAV_W - PAD * 2.0).max(0.0),
            panel.size.y - HEADER_HEIGHT - CONTENT_BOTTOM_PAD,
        ),
    }
}

/// The left navigation rail (its fill and right hairline).
pub(super) fn nav_rail_rect(panel: Rect) -> Rect {
    Rect {
        origin: panel.origin,
        size: Point2D::new(NAV_W, panel.size.y),
    }
}

/// Paint-only clip for the scrollable settings body. Full-width cards sit
/// exactly on the content edges, while their 1px outlines are centered on
/// those edges. Leave one horizontal pixel for the outer half of each stroke
/// (and its antialiasing) without relaxing the vertical scroll clip.
pub(super) fn content_paint_clip_rect(panel: Rect) -> Rect {
    let content = content_rect(panel);
    Rect {
        origin: Point2D::new(content.origin.x - 1.0, content.origin.y),
        size: Point2D::new(content.size.x + 2.0, content.size.y),
    }
}

/// One row of the left navigation rail, under the "Settings" title.
pub(super) fn nav_item_rect(panel: Rect, i: usize, _count: usize) -> Rect {
    Rect {
        origin: Point2D::new(
            panel.origin.x + 10.0,
            panel.origin.y + NAV_TITLE_H + i as f32 * (NAV_ITEM_H + NAV_ITEM_GAP),
        ),
        size: Point2D::new(NAV_W - 20.0, NAV_ITEM_H),
    }
}

pub(super) fn close_rect(panel: Rect) -> Rect {
    let s = 16.0;
    Rect {
        origin: Point2D::new(
            panel.origin.x + panel.size.x - 20.0 - s,
            panel.origin.y + 24.0,
        ),
        size: Point2D::new(s, s),
    }
}

/// Images / Fonts take the same intro every other tab does,
/// with one muted line.
pub(super) const SECONDARY_HEADING_HAS_DESC: bool = true;

/// Body rect handed to those tabs: the content viewport pushed down past
/// the intro the panel paints for them. Keeping the shift here means
/// their own geometry stays written against "the top of my body", so
/// adding the intro needed no edits inside them.
pub(super) fn hero_body_rect(content: Rect) -> Rect {
    hero_body_rect_for_ui(content)
}

pub(super) fn hero_body_rect_for_ui(content: Rect) -> Rect {
    let dy = crate::widgets::agent_settings_rows::tab_intro_height(SECONDARY_HEADING_HAS_DESC);
    // Desktop (Settings v2) sets these tabs' bodies on a card: the body
    // sits inside it, inset by the card padding.
    let inset_x = crate::widgets::agent_settings_rows::CARD_INSET_X;
    Rect {
        origin: Point2D::new(
            content.origin.x + inset_x,
            content.origin.y + dy + SECONDARY_CARD_PAD_Y,
        ),
        size: Point2D::new(
            (content.size.x - 2.0 * inset_x).max(0.0),
            (content.size.y - dy - SECONDARY_CARD_PAD_Y).max(0.0),
        ),
    }
}

/// Vertical padding inside the desktop card that holds a secondary tab's
/// body (top and bottom).
pub(super) const SECONDARY_CARD_PAD_Y: f32 = 12.0;

/// The desktop card behind a secondary tab's body of `body_h`.
pub(super) fn secondary_card_rect(content: Rect, body_h: f32) -> Rect {
    let top = content.origin.y
        + crate::widgets::agent_settings_rows::tab_intro_height(SECONDARY_HEADING_HAS_DESC);
    Rect {
        origin: Point2D::new(content.origin.x, top),
        size: Point2D::new(content.size.x, body_h + 2.0 * SECONDARY_CARD_PAD_Y),
    }
}
