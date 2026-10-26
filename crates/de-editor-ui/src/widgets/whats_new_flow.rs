//! Host-shared paint / press / hover flow of the "What's new" window
//! (#251). Both hosts call these from the top of their ladders and only
//! add their `mark_dirty` tail, so the window behaves the same on desktop
//! and in the browser.

use crate::widgets::whats_new_modal::{WhatsNewHit, WhatsNewModal};
use crate::widgets::{PaintCx, Widget};
use crate::{Color, Point2D, Rect};
use de_editor_core::EditorState;

/// Paint the scrim and the card. No-op while the window is closed.
pub fn paint(cx: &mut PaintCx<'_>, state: &EditorState, viewport_w: f32, viewport_h: f32) {
    let Some(modal) = WhatsNewModal::for_editor(state) else {
        return;
    };
    cx.backend.fill_rect(
        Rect::xywh(0.0, 0.0, viewport_w, viewport_h),
        Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.5,
        },
    );
    let rect = modal.rect(viewport_w, viewport_h);
    modal.paint(cx, rect);
}

/// `None` while the window is closed; otherwise the press is consumed and
/// the flag says whether state changed. Presses outside the card are
/// swallowed: only "Got it" or Escape closes the window.
pub fn press(
    state: &mut EditorState,
    point: Point2D,
    viewport_w: f32,
    viewport_h: f32,
) -> Option<bool> {
    let hit = {
        let modal = WhatsNewModal::for_editor(state)?;
        modal.hit_test(modal.rect(viewport_w, viewport_h), point)
    };
    let WhatsNewHit::Button(button) = hit else {
        return Some(false);
    };
    state.editor_ui.press_whats_new_button(button);
    // The pressed button may have moved or gone; the next move re-hovers.
    if let Some(whats_new) = state.editor_ui.whats_new.as_mut() {
        whats_new.hover = None;
    }
    Some(true)
}

/// Button hover. `None` while the window is closed; otherwise the move is
/// consumed and the flag says whether the hover changed.
pub fn hover(
    state: &mut EditorState,
    point: Point2D,
    viewport_w: f32,
    viewport_h: f32,
) -> Option<bool> {
    let hover = {
        let modal = WhatsNewModal::for_editor(state)?;
        match modal.hit_test(modal.rect(viewport_w, viewport_h), point) {
            WhatsNewHit::Button(button) => Some(button),
            _ => None,
        }
    };
    let whats_new = state.editor_ui.whats_new.as_mut()?;
    let changed = whats_new.hover != hover;
    whats_new.hover = hover;
    Some(changed)
}
