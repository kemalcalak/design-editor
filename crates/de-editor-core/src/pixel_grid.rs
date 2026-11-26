//! Pixel grid (#399): when it shows and how geometry snaps to it.
//!
//! The grid paints one hairline per document pixel once the canvas is
//! zoomed to [`PIXEL_GRID_ZOOM`] or more (View ▸ Pixel grid, Shift+').
//! View ▸ Snap to pixel grid (Cmd+Shift+') rounds node moves, handle
//! resizes and shape drawing to whole pixels at the same zoom, the gate
//! vector edit mode already used for points (#387).

use crate::walkers::find_node;
use crate::{EditorState, PenNodeExt};

/// Zoom (4 = 400 %) from which the pixel grid shows and snapping to it
/// applies.
pub const PIXEL_GRID_ZOOM: f32 = 4.0;

/// Whether the pixel grid paints at `zoom` with View ▸ Pixel grid set to
/// `hidden`.
pub fn pixel_grid_visible(zoom: f32, hidden: bool) -> bool {
    !hidden && zoom >= PIXEL_GRID_ZOOM
}

/// Round a rectangle's edges to whole pixels, keeping at least a 1 px
/// size on each axis. `(x, y, w, h)` in and out.
pub fn snap_rect_edges(x: f64, y: f64, w: f64, h: f64) -> (f64, f64, f64, f64) {
    let x0 = x.round();
    let y0 = y.round();
    let x1 = (x + w).round().max(x0 + 1.0);
    let y1 = (y + h).round().max(y0 + 1.0);
    (x0, y0, x1 - x0, y1 - y0)
}

/// Round only the edges a resize handle moves (`[left, top, right,
/// bottom]`), keeping at least a 1 px size. `(x, y, w, h)` in and out.
pub fn snap_moving_edges(
    (x, y, w, h): (f64, f64, f64, f64),
    [left, top, right, bottom]: [bool; 4],
) -> (f64, f64, f64, f64) {
    let (mut x0, mut y0, mut x1, mut y1) = (x, y, x + w, y + h);
    if left {
        x0 = x0.round().min(x1 - 1.0);
    }
    if right {
        x1 = x1.round().max(x0 + 1.0);
    }
    if top {
        y0 = y0.round().min(y1 - 1.0);
    }
    if bottom {
        y1 = y1.round().max(y0 + 1.0);
    }
    (x0, y0, x1 - x0, y1 - y0)
}

/// Offset that moves `v` onto the nearest whole pixel.
pub fn snap_offset(v: f64) -> f64 {
    v.round() - v
}

/// One axis of a snapped drag. `pos` is where the drag put the node and
/// `residual` the rounding the previous step added, so `pos - residual`
/// is where the cursor alone would have it. Rounds that unsnapped
/// position (not `pos`, or slow travel would never leave the pixel) and
/// returns `(correction to apply, new residual)`.
pub fn snap_drag_axis(pos: f64, residual: f64) -> (f64, f64) {
    let unsnapped = pos - residual;
    let target = unsnapped.round();
    (target - pos, target - unsnapped)
}

impl EditorState {
    /// Snap to pixel grid is on and the canvas is zoomed in far enough
    /// for it to apply.
    pub fn pixel_snap_active(&self) -> bool {
        !self.ui.pixel_grid_snap_off && self.viewport.zoom >= PIXEL_GRID_ZOOM
    }

    /// The selection anchor's authored (parent-relative) position, when it
    /// has one. A flow child in auto layout has none and never snaps.
    pub fn anchor_authored_position(&self) -> Option<(f64, f64)> {
        let anchor = &self.selection.anchor;
        if !anchor.is_real() {
            return None;
        }
        let base = find_node(self.active_children(), anchor)?.base();
        Some((base.x?, base.y?))
    }
}

#[cfg(test)]
#[path = "pixel_grid_tests.rs"]
mod tests;
