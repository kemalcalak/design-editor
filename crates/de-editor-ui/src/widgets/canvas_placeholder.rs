//! In-progress frames (#16): a frame with `placeholder: true` is still being
//! built by an agent. The editor canvas paints it with the same periwinkle
//! panel and slow sweep the live generation scan uses for an empty section
//! (`canvas_generation_scan::paint_generation_scan`, Pencil's placeholder
//! look), under the frame's children, until the agent clears the flag.
//!
//! Only the editor canvas shows it: `canvas_viewport` paints the page inside
//! [`with_overlay`], so exports, screenshots and mask sources (which reach
//! the same node painter) keep the authored pixels. The sweep marks the frame
//! as animated at a calm cadence, so the canvas keeps repainting only while
//! such a frame is painted.

use std::cell::Cell;

use super::canvas_generation_scan::{paint_generation_scan, SKELETON_BLUE};
use crate::layout_scene::SceneNode;
use crate::widgets::PaintCx;
use crate::Rect;

/// Repaint cadence while a placeholder sweep is on screen (20 fps; the
/// band takes 2.2 s to cross the frame, so a faster rate buys nothing).
pub const SWEEP_FRAME_INTERVAL_MS: u64 = 50;

thread_local! {
    static OVERLAY: Cell<bool> = const { Cell::new(false) };
}

/// Run `paint` with the in-progress overlay enabled (the editor canvas).
pub(crate) fn with_overlay<R>(paint: impl FnOnce() -> R) -> R {
    // Restored on drop, so a panicking paint cannot leave later exports on
    // this thread painting the overlay.
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            OVERLAY.with(|overlay| overlay.set(self.0));
        }
    }
    let _restore = Restore(OVERLAY.with(|overlay| overlay.replace(true)));
    paint()
}

/// Paint the in-progress state over `node`'s box when it is a placeholder
/// frame on the editor canvas. Returns whether it painted.
pub(super) fn paint(
    cx: &mut PaintCx<'_>,
    node: &SceneNode,
    world_rect: Rect,
    zoom: f32,
    now_ms: u64,
) -> bool {
    if !node.placeholder || !OVERLAY.with(Cell::get) {
        return false;
    }
    paint_generation_scan(cx, node, world_rect, zoom, now_ms, SKELETON_BLUE);
    super::canvas_shader_clock::mark_animated(SWEEP_FRAME_INTERVAL_MS);
    true
}

#[cfg(test)]
#[path = "canvas_placeholder_tests.rs"]
mod tests;
