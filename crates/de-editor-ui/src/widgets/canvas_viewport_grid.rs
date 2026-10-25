//! Dotted canvas grid painter for [`super::canvas_viewport::CanvasViewport`].
//!
//! Kept separate from the viewport widget so the hot grid path can
//! evolve without pushing `canvas_viewport.rs` over the 800-line cap.

use crate::theme::Theme;
use crate::widgets::PaintCx;
use crate::{Color, Point2D, Rect};
use de_editor_core::Viewport as DocViewport;

const GRID_SPACING: f32 = 32.0;

#[derive(Clone, Copy)]
struct GridMetrics {
    step: f32,
    origin_x: f32,
    origin_y: f32,
    dot_radius: f32,
    color: Color,
}

/// Paint a dotted grid across the canvas widget rect. The grid is
/// drawn in canvas-local coordinates and offset by `viewport.pan`
/// so it scrolls with the document content. Dots get sparser as
/// zoom decreases (skipping every other dot at low zoom) so they
/// stay visually airy.
///
/// With `limit` (a partial repaint's cull rect: a pan strip, a restore
/// tile, an animated region, #208) only the dots near it are emitted. The
/// lattice stays anchored to `rect`, so they land exactly where a full
/// paint puts them; the rest would be clipped away anyway.
pub(super) fn paint_grid_within(
    cx: &mut PaintCx<'_>,
    rect: Rect,
    viewport: &DocViewport,
    theme: &Theme,
    limit: Option<Rect>,
) {
    let metrics = grid_metrics(rect, viewport, theme);
    let step = metrics.step;
    let first_x = metrics.origin_x - step;
    let first_y = metrics.origin_y - step;
    let end_x = rect.origin.x + rect.size.x + step;
    let end_y = rect.origin.y + rect.size.y + step;
    // First lattice index at or before `from` (never before the start).
    let skip = |first: f32, from: f32| ((from - first) / step).floor().max(0.0);
    let (x0, y0, x1, y1) = match limit {
        Some(limit) => (
            first_x + skip(first_x, limit.origin.x - step) * step,
            first_y + skip(first_y, limit.origin.y - step) * step,
            end_x.min(limit.origin.x + limit.size.x + step),
            end_y.min(limit.origin.y + limit.size.y + step),
        ),
        None => (first_x, first_y, end_x, end_y),
    };
    let mut centers = Vec::with_capacity(match limit {
        Some(_) => 0,
        None => grid_dot_count_from_metrics(rect, metrics),
    });
    let mut y = y0;
    while y < y1 {
        let mut x = x0;
        while x < x1 {
            centers.push(Point2D::new(x, y));
            x += step;
        }
        y += step;
    }
    if !centers.is_empty() {
        cx.backend
            .fill_dots(&centers, metrics.dot_radius, metrics.color);
    }
}

#[cfg(test)]
pub(crate) fn grid_dot_count(rect: Rect, viewport: &DocViewport) -> usize {
    grid_dot_count_from_metrics(rect, grid_metrics(rect, viewport, &Theme::light()))
}

fn grid_dot_count_from_metrics(rect: Rect, metrics: GridMetrics) -> usize {
    let cols = axis_dot_count(
        metrics.origin_x - metrics.step,
        rect.origin.x + rect.size.x + metrics.step,
        metrics.step,
    );
    let rows = axis_dot_count(
        metrics.origin_y - metrics.step,
        rect.origin.y + rect.size.y + metrics.step,
        metrics.step,
    );
    cols.saturating_mul(rows)
}

fn axis_dot_count(first: f32, limit: f32, step: f32) -> usize {
    if step <= 0.0 || limit <= first {
        return 0;
    }
    ((limit - first) / step).ceil() as usize
}

fn grid_metrics(rect: Rect, viewport: &DocViewport, theme: &Theme) -> GridMetrics {
    let zoom = viewport.zoom.max(0.0001);
    let mut step = GRID_SPACING * zoom;
    while step < 8.0 {
        step *= 2.0;
    }
    let dot_size = (1.5 * zoom.sqrt()).clamp(1.0, 2.5);
    GridMetrics {
        step,
        origin_x: rect.origin.x + viewport.pan_x.rem_euclid(step),
        origin_y: rect.origin.y + viewport.pan_y.rem_euclid(step),
        dot_radius: dot_size / 2.0,
        color: Color {
            r: theme.muted_foreground.r,
            g: theme.muted_foreground.g,
            b: theme.muted_foreground.b,
            a: 0.18,
        },
    }
}
