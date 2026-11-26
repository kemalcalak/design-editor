//! Rename-layers preview flow shared by the widget hosts (#383): every
//! host-visible transition (press, wheel, hover) is `EditorState` mutation
//! plus the panel's hit-tests, so a host only supplies the viewport and its
//! `mark_dirty()` tail. The dialog is modal: while it is open it consumes
//! every press, including one on the scrim.

use de_editor_core::EditorState;

use crate::widgets::rename_layers_panel::{RenameLayersHit, RenameLayersPanel};
use crate::{Point2D, Rect};

/// Route a press. `false` only when the preview is closed.
pub fn press(
    state: &mut EditorState,
    viewport_width: f32,
    viewport_height: f32,
    point: Point2D,
    now_ms: u64,
) -> bool {
    let Some(hit) = RenameLayersPanel::for_editor(state).map(|panel| {
        let rect = panel.rect(viewport_width, viewport_height);
        panel.hit_test(rect, point)
    }) else {
        return false;
    };
    match hit {
        RenameLayersHit::Toggle(index) => {
            state.toggle_rename_layers_row(index);
        }
        RenameLayersHit::Name(index) => {
            state.start_rename_layers_edit(index, now_ms);
        }
        // Nothing ticked: the button is drawn disabled and does nothing.
        RenameLayersHit::Apply => {
            let any_ticked = state
                .editor_ui
                .rename_layers
                .as_ref()
                .is_some_and(|dialog| dialog.accepted_count() > 0);
            if any_ticked {
                state.apply_rename_layers();
            }
        }
        RenameLayersHit::Cancel => state.close_rename_layers(),
        // A press anywhere else ends an inline edit, keeping what was typed.
        RenameLayersHit::Inside | RenameLayersHit::Outside => {
            state.commit_rename_layers_edit();
        }
    }
    true
}

/// Wheel over the rows. `None` when the preview is closed or the pointer is
/// not over the rows.
pub fn scroll(
    state: &mut EditorState,
    x: f32,
    y: f32,
    delta_y: f32,
    viewport_width: f32,
    viewport_height: f32,
) -> Option<bool> {
    let max = RenameLayersPanel::for_editor(state).and_then(|panel| {
        let rect = panel.rect(viewport_width, viewport_height);
        panel
            .rows_rect(rect)
            .contains(Point2D::new(x, y))
            .then(|| panel.max_rows_scroll())
    })?;
    let dialog = state.editor_ui.rename_layers.as_mut()?;
    let next = (dialog.scroll.offset - delta_y).clamp(0.0, max);
    if next == dialog.scroll.offset {
        return Some(false);
    }
    dialog.scroll.offset = next;
    Some(true)
}

/// Update the hovered control. `true` when the tint changed.
pub fn hover(
    state: &mut EditorState,
    viewport_width: f32,
    viewport_height: f32,
    point: Point2D,
) -> bool {
    let next = RenameLayersPanel::for_editor(state).and_then(|panel| {
        let rect = panel.rect(viewport_width, viewport_height);
        panel.hover_at(rect, point)
    });
    let Some(dialog) = state.editor_ui.rename_layers.as_mut() else {
        return false;
    };
    if dialog.hover == next {
        return false;
    }
    dialog.hover = next;
    true
}

/// The card in viewport coordinates while the preview is open.
pub fn panel_rect(state: &EditorState, viewport_width: f32, viewport_height: f32) -> Option<Rect> {
    RenameLayersPanel::for_editor(state).map(|panel| panel.rect(viewport_width, viewport_height))
}
