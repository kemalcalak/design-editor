//! Pixel grid (#399): one device hairline per document pixel, painted
//! over the nodes once the canvas is zoomed to
//! [`de_editor_core::pixel_grid::PIXEL_GRID_ZOOM`] or more.
//!
//! The lattice is anchored to the document (`pan + n * zoom`), so a
//! cached pan layer stays correct. With `limit` (a partial repaint's
//! cull rect) only the lines crossing it are emitted, the same discipline
//! as the dotted grid. No allocation: each line is one `stroke_line`.

use crate::widgets::PaintCx;
use crate::{Color, Point2D, Rect};
use de_editor_core::Viewport as DocViewport;

/// Neutral grey that reads over light and dark fills alike.
const LINE: Color = Color {
    r: 0.5,
    g: 0.5,
    b: 0.5,
    a: 0.28,
};

/// Paint the pixel lattice across `rect` (screen space). The caller
/// gates on visibility; this only guards a degenerate zoom.
pub(super) fn paint_pixel_grid(
    cx: &mut PaintCx<'_>,
    rect: Rect,
    viewport: &DocViewport,
    limit: Option<Rect>,
) {
    let step = viewport.zoom;
    if !step.is_finite() || step < 1.0 {
        return;
    }
    let area = match limit {
        Some(limit) => match intersect(rect, limit) {
            Some(area) => area,
            None => return,
        },
        None => rect,
    };
    let width = 1.0 / cx.backend.dpi_scale().max(1.0);
    let (x0, x1) = (area.origin.x, area.origin.x + area.size.x);
    let (y0, y1) = (area.origin.y, area.origin.y + area.size.y);
    let origin_x = rect.origin.x + viewport.pan_x;
    let origin_y = rect.origin.y + viewport.pan_y;
    let mut x = first_line(origin_x, x0, step);
    while x <= x1 {
        cx.backend
            .stroke_line(Point2D::new(x, y0), Point2D::new(x, y1), LINE, width);
        x += step;
    }
    let mut y = first_line(origin_y, y0, step);
    while y <= y1 {
        cx.backend
            .stroke_line(Point2D::new(x0, y), Point2D::new(x1, y), LINE, width);
        y += step;
    }
}

/// First lattice coordinate (`origin + n * step`) at or after `from`.
fn first_line(origin: f32, from: f32, step: f32) -> f32 {
    origin + ((from - origin) / step).ceil() * step
}

fn intersect(a: Rect, b: Rect) -> Option<Rect> {
    let x0 = a.origin.x.max(b.origin.x);
    let y0 = a.origin.y.max(b.origin.y);
    let x1 = (a.origin.x + a.size.x).min(b.origin.x + b.size.x);
    let y1 = (a.origin.y + a.size.y).min(b.origin.y + b.size.y);
    (x1 > x0 && y1 > y0).then(|| Rect::xywh(x0, y0, x1 - x0, y1 - y0))
}

#[cfg(test)]
#[path = "canvas_pixel_grid_tests.rs"]
mod tests;
