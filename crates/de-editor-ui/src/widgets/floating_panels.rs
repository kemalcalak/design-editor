//! Editor v3 (#86): the panels float as cards over a full-bleed canvas.
//!
//! The canvas fills the window and never resizes when a panel opens, so
//! artboards stay where they are. The left panel (Agent / Layers /
//! Components / Images) is a card inset from the window's left edge, the
//! properties panel a card on the right that shows only with a selection,
//! and the panel toggle sits above the tool rail. Opening the left card
//! slides it in from the left while the toggle and the rail slide right by
//! the card's width — every rect here reads the one [`panel_reveal`]
//! progress, so paint and hit-test can never disagree mid-motion.

use de_editor_core::EditorState;

use crate::theme::Theme;
use crate::widgets::canvas_actions::CANVAS_ACTIONS_HEIGHT;
use crate::widgets::file_pill::FILE_PILL_HEIGHT;
use crate::widgets::host_canvas_geometry::{workspace_docked, CHROME_INSET_TOP, STATUS_INSET};
use crate::widgets::{STATUS_BAR_HEIGHT, TOOLBAR_WIDTH};
use crate::{Point2D, Rect, RenderBackend};

/// Gap between a floating card and the window edge it sits against.
pub const CARD_INSET: f32 = 12.0;
/// Corner radius of the floating cards.
pub const CARD_RADIUS: f32 = 12.0;
/// Gap between the open left card and the toggle / tool rail beside it.
pub const CARD_GAP: f32 = 8.0;
/// Gap under the chrome row before a card starts, and between the panel
/// toggle and the tool rail.
const ROW_GAP: f32 = 12.0;
/// The left card starts under the file pill.
pub const LEFT_CARD_TOP: f32 = CHROME_INSET_TOP + FILE_PILL_HEIGHT + ROW_GAP;
/// The properties card starts under the action group.
pub const RIGHT_CARD_TOP: f32 = CHROME_INSET_TOP + CANVAS_ACTIONS_HEIGHT + ROW_GAP;
/// The toggle is a square the tool rail's width.
pub const PANEL_TOGGLE_SIZE: f32 = TOOLBAR_WIDTH;
/// Gap between the toggle and the tool rail under it.
pub const TOGGLE_RAIL_GAP: f32 = 10.0;

/// How far the left card is open, `0.0` (shut) ..= `1.0` (open). Settled
/// state follows `sidebar_open`; while the panel motion runs it is the
/// eased progress the hosts' frame bookkeeping stored.
pub fn panel_reveal(state: &EditorState) -> f32 {
    state.editor_ui.panel_reveal()
}

/// Whether any part of the left card is on screen: it is open or still
/// sliding shut, and nothing hides it (the workspace dock owns the left
/// column; Preview hides the editing cards over its full-bleed stage).
/// The one answer both hosts' paint and input read.
pub fn left_card_on_screen(state: &EditorState) -> bool {
    let ui = &state.editor_ui;
    !workspace_docked(state) && !ui.preview.mode && (ui.sidebar_open || ui.panel_motion_active())
}

/// Whether the properties card is on screen: something to inspect, and
/// not hidden by the workspace dock or Preview.
pub fn right_card_on_screen(state: &EditorState) -> bool {
    !workspace_docked(state) && !state.editor_ui.preview.mode && state.right_rail_visible()
}

/// Open / close the left panel with its motion — the toggle, ⌘\ and
/// File ▸ Show / Hide panel all land here. Refused (returns `false`)
/// while the workspace dock owns the column or Preview hides the panels.
pub fn toggle_panel(state: &mut EditorState, now_ms: u64) -> bool {
    if workspace_docked(state) || state.editor_ui.preview.mode {
        return false;
    }
    state.editor_ui.toggle_panel_animated(now_ms);
    true
}

/// How far the left card is slid out of view, in px: its own width plus
/// the inset when shut, `0` when open.
fn left_card_slide(state: &EditorState) -> f32 {
    (state.editor_ui.layer_panel_width + CARD_INSET) * (1.0 - panel_reveal(state))
}

/// The left card. Callers gate on [`left_card_on_screen`] (or
/// `sidebar_open` for input) — this returns the rect it WOULD occupy at
/// the current reveal.
pub fn left_card_rect(state: &EditorState, viewport_h: f32) -> Rect {
    Rect::xywh(
        CARD_INSET - left_card_slide(state),
        LEFT_CARD_TOP,
        state.editor_ui.layer_panel_width,
        (viewport_h - LEFT_CARD_TOP - CARD_INSET).max(0.0),
    )
}

/// Left edge of the tool column (toggle + rail): beside the card when it
/// is open, at the window inset when it is shut.
pub fn tool_column_x(state: &EditorState) -> f32 {
    let open = state.editor_ui.layer_panel_width + CARD_GAP;
    CARD_INSET + open * panel_reveal(state)
}

/// The panel toggle above the tool rail.
pub fn panel_toggle_rect(state: &EditorState) -> Rect {
    Rect::xywh(
        tool_column_x(state),
        LEFT_CARD_TOP,
        PANEL_TOGGLE_SIZE,
        PANEL_TOGGLE_SIZE,
    )
}

/// Top of the tool rail: under the toggle.
pub fn tool_rail_top() -> f32 {
    LEFT_CARD_TOP + PANEL_TOGGLE_SIZE + TOGGLE_RAIL_GAP
}

/// The properties card. It runs from under the action group down to just
/// above the zoom pill, so the zoom stays reachable with a selection.
/// Callers gate on the panel resolving for the selection.
pub fn right_card_rect(state: &EditorState, viewport_w: f32, viewport_h: f32) -> Rect {
    let width = state.editor_ui.property_panel_width;
    let floor = viewport_h - STATUS_INSET - STATUS_BAR_HEIGHT - ROW_GAP;
    Rect::xywh(
        viewport_w - width - CARD_INSET,
        RIGHT_CARD_TOP,
        width,
        (floor - RIGHT_CARD_TOP).max(0.0),
    )
}

/// Whether `point` lands on a floating card (or the toggle) — what the
/// canvas must never receive a press, hover or wheel through.
pub fn over_floating_card(
    state: &EditorState,
    viewport_w: f32,
    viewport_h: f32,
    point: Point2D,
) -> bool {
    if workspace_docked(state) {
        return false;
    }
    if left_card_rect(state, viewport_h).contains(point) && left_card_on_screen(state) {
        return true;
    }
    if !state.editor_ui.preview.mode && panel_toggle_rect(state).contains(point) {
        return true;
    }
    // Rect first: the visibility test walks the selection.
    right_card_rect(state, viewport_w, viewport_h).contains(point) && right_card_on_screen(state)
}

/// The part of the canvas no settled panel covers: right of the tool
/// column (and the open left card), left of the properties card. "Fit"
/// frames content here so it is not born under a card; nothing else uses
/// it — opening or closing a panel never moves the camera.
pub fn uncovered_canvas_rect(state: &EditorState, viewport_w: f32, viewport_h: f32) -> Rect {
    let (cx0, cy0, cw, ch) =
        crate::widgets::host_canvas_geometry::canvas_region(state, viewport_w, viewport_h);
    if workspace_docked(state) || state.editor_ui.preview.mode {
        return Rect::xywh(cx0, cy0, cw, ch);
    }
    let shown = |open: bool| if open { 1.0 } else { 0.0 };
    let left_card =
        (state.editor_ui.layer_panel_width + CARD_GAP) * shown(state.editor_ui.sidebar_open);
    let left = cx0 + CARD_INSET + left_card + TOOLBAR_WIDTH;
    let right = if state.right_rail_visible() {
        right_card_rect(state, viewport_w, viewport_h).origin.x
    } else {
        cx0 + cw
    };
    if right - left < cw * 0.25 {
        // Too narrow to be worth framing into; use the whole canvas.
        return Rect::xywh(cx0, cy0, cw, ch);
    }
    Rect::xywh(left, cy0, right - left, ch)
}

/// Fit `content` (document space) into [`uncovered_canvas_rect`].
pub fn fit_into_uncovered(
    state: &mut EditorState,
    content: Rect,
    viewport_w: f32,
    viewport_h: f32,
    padding: f32,
    max_zoom: f32,
) {
    let (cx0, cy0, _, _) =
        crate::widgets::host_canvas_geometry::canvas_region(state, viewport_w, viewport_h);
    let area = uncovered_canvas_rect(state, viewport_w, viewport_h);
    let viewport = &mut state.viewport;
    viewport.fit_to_with_max_zoom(content, area.size.x, area.size.y, padding, max_zoom);
    // `fit_to` centres in a canvas at the region's origin; shift that
    // centre onto the uncovered area's.
    viewport.pan_x += area.origin.x - cx0;
    viewport.pan_y += area.origin.y - cy0;
}

/// Paint a floating card: its ground, `body` clipped to the rounded
/// corners, then the hairline on top (so content that fills its rect
/// square never covers the border).
pub fn paint_card(
    backend: &mut dyn RenderBackend,
    theme: &Theme,
    rect: Rect,
    body: impl FnOnce(&mut dyn RenderBackend),
) {
    backend.fill_round_rect(rect, CARD_RADIUS, theme.card);
    backend.save();
    backend.clip_round_rect(rect, CARD_RADIUS);
    body(&mut *backend);
    backend.restore();
    backend.stroke_round_rect(rect, CARD_RADIUS, theme.border, 1.0);
}

#[cfg(test)]
#[path = "floating_panels_tests.rs"]
mod tests;
