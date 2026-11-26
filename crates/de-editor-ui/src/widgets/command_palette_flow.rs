//! Command palette flow shared by the widget hosts (#395): press, wheel and
//! hover are `EditorState` transitions plus the panel's hit-tests, and
//! [`frame_nodes`] is the camera move after a find match is selected. A
//! host supplies the viewport and its scene, runs the returned
//! [`PaletteOutcome`] and repaints. The palette is modal: while it is open
//! it consumes every press; one on the scrim closes it.

use crate::layout_scene::LayoutScene;
use de_editor_core::command_palette::{PaletteField, PaletteOutcome};
use de_editor_core::{EditorState, NodeId};

use crate::widgets::command_palette_panel::{CommandPaletteHit, CommandPalettePanel};
use crate::{Point2D, Rect};

/// Route a press. `None` only when the palette is closed.
pub fn press(
    state: &mut EditorState,
    viewport_width: f32,
    viewport_height: f32,
    point: Point2D,
    now_ms: u64,
) -> Option<PaletteOutcome> {
    let hit = CommandPalettePanel::for_editor(state).map(|panel| {
        let rect = panel.rect(viewport_width, viewport_height);
        panel.hit_test(rect, point)
    })?;
    let focus = |state: &mut EditorState, field: PaletteField| {
        if let Some(palette) = state.editor_ui.command_palette.as_mut() {
            palette.field = field;
            palette.active_input_mut().touch(now_ms);
        }
    };
    Some(match hit {
        CommandPaletteHit::Query => {
            focus(state, PaletteField::Query);
            PaletteOutcome::Nothing
        }
        CommandPaletteHit::Replace => {
            focus(state, PaletteField::Replace);
            PaletteOutcome::Nothing
        }
        CommandPaletteHit::ReplaceAll => state.replace_in_command_palette(),
        CommandPaletteHit::Row(index) => state.activate_command_palette_row(index, false),
        CommandPaletteHit::Inside => PaletteOutcome::Nothing,
        CommandPaletteHit::Outside => {
            state.close_command_palette();
            PaletteOutcome::Nothing
        }
    })
}

/// Wheel over the rows. `None` when the palette is closed or the pointer
/// is not over the rows.
pub fn scroll(
    state: &mut EditorState,
    x: f32,
    y: f32,
    delta_y: f32,
    viewport_width: f32,
    viewport_height: f32,
) -> Option<bool> {
    let max = CommandPalettePanel::for_editor(state).and_then(|panel| {
        let rect = panel.rect(viewport_width, viewport_height);
        panel
            .rows_rect(rect)
            .contains(Point2D::new(x, y))
            .then(|| panel.max_rows_scroll())
    })?;
    let palette = state.editor_ui.command_palette.as_mut()?;
    let next = (palette.scroll.offset - delta_y).clamp(0.0, max);
    if next == palette.scroll.offset {
        return Some(false);
    }
    palette.scroll.offset = next;
    Some(true)
}

/// Update the hovered row / button. `true` when the tint changed.
pub fn hover(
    state: &mut EditorState,
    viewport_width: f32,
    viewport_height: f32,
    point: Point2D,
) -> bool {
    let next = CommandPalettePanel::for_editor(state).and_then(|panel| {
        let rect = panel.rect(viewport_width, viewport_height);
        panel.hover_at(rect, point)
    });
    let Some(palette) = state.editor_ui.command_palette.as_mut() else {
        return false;
    };
    if palette.hover == next {
        return false;
    }
    palette.hover = next;
    true
}

/// The card in viewport coordinates while the palette is open.
pub fn panel_rect(state: &EditorState, viewport_width: f32, viewport_height: f32) -> Option<Rect> {
    CommandPalettePanel::for_editor(state).map(|panel| panel.rect(viewport_width, viewport_height))
}

/// Frame `ids` (on the active page) in the uncovered canvas, never zooming
/// past 100 %. Camera-only. Returns whether the camera moved.
pub fn frame_nodes(
    state: &mut EditorState,
    scene: &LayoutScene,
    ids: &[NodeId],
    viewport_width: f32,
    viewport_height: f32,
) -> bool {
    let Some(page) = scene.active_page() else {
        return false;
    };
    let mut bounds: Option<(f32, f32, f32, f32)> = None;
    for id in ids {
        let Some(rect) = page.find(id.as_str()).map(|node| node.aggregate_bounds()) else {
            continue;
        };
        let (x0, y0) = (rect.origin.x, rect.origin.y);
        let (x1, y1) = (x0 + rect.size.x, y0 + rect.size.y);
        bounds = Some(match bounds {
            Some((a, b, c, d)) => (a.min(x0), b.min(y0), c.max(x1), d.max(y1)),
            None => (x0, y0, x1, y1),
        });
    }
    let Some((x0, y0, x1, y1)) = bounds else {
        return false;
    };
    let before = state.viewport;
    crate::widgets::floating_panels::fit_into_uncovered(
        state,
        Rect {
            origin: Point2D::new(x0, y0),
            size: Point2D::new(x1 - x0, y1 - y0),
        },
        viewport_width,
        viewport_height,
        48.0,
        1.0,
    );
    state.viewport != before
}
