//! Canvas drop-target preview and the hand-segmented dashed-rect helper
//! used by the viewport overlays.
//!
//! Split out of `canvas_viewport.rs` to keep that spine under the
//! repository's 800-line cap.

use crate::theme::Theme;
use crate::widgets::PaintCx;
use crate::{Color, Point2D, Rect};
use de_editor_core::Viewport as DocViewport;
pub(super) fn paint_drop_indicator(
    cx: &mut PaintCx<'_>,
    canvas_rect: Rect,
    viewport: &DocViewport,
    theme: &Theme,
    indicator: &de_editor_core::editor_ui_state::CanvasDropIndicator,
    paint_ghost: bool,
) {
    let to_screen_rect = |r: de_editor_core::editor_ui_state::CanvasOverlayRect| {
        Rect::xywh(
            canvas_rect.origin.x + viewport.pan_x + r.x as f32 * viewport.zoom,
            canvas_rect.origin.y + viewport.pan_y + r.y as f32 * viewport.zoom,
            r.w as f32 * viewport.zoom,
            r.h as f32 * viewport.zoom,
        )
    };
    let primary = theme.primary;
    if let Some(target) = indicator.target {
        let rect = to_screen_rect(target);
        let fill = Color { a: 0.08, ..primary };
        let stroke = Color { a: 0.45, ..primary };
        cx.backend.fill_rect(rect, fill);
        cx.backend.stroke_rect(rect, stroke, 1.0);
    }
    if paint_ghost {
        let ghost = to_screen_rect(indicator.ghost);
        let ghost_fill = Color { a: 0.10, ..primary };
        let ghost_stroke = Color { a: 0.85, ..primary };
        cx.backend.fill_rect(ghost, ghost_fill);
        paint_dashed_rect(cx, ghost, ghost_stroke, 1.25);
    }
    if let Some(line) = indicator.insertion {
        let from = Point2D::new(
            canvas_rect.origin.x + viewport.pan_x + line.x1 as f32 * viewport.zoom,
            canvas_rect.origin.y + viewport.pan_y + line.y1 as f32 * viewport.zoom,
        );
        let to = Point2D::new(
            canvas_rect.origin.x + viewport.pan_x + line.x2 as f32 * viewport.zoom,
            canvas_rect.origin.y + viewport.pan_y + line.y2 as f32 * viewport.zoom,
        );
        cx.backend.stroke_line(from, to, primary, 2.0);
    }
}

/// Stroke a dashed rectangle as 4 dashed edges (4 px on / 4 px off,
/// screen-space) — the `RenderBackend` trait has no path-effect
/// surface, so the dash is segmented by hand; backend-agnostic.
pub(crate) fn paint_dashed_rect(cx: &mut PaintCx<'_>, rect: Rect, color: Color, width: f32) {
    let tl = rect.origin;
    let tr = Point2D::new(rect.origin.x + rect.size.x, rect.origin.y);
    let br = Point2D::new(rect.origin.x + rect.size.x, rect.origin.y + rect.size.y);
    let bl = Point2D::new(rect.origin.x, rect.origin.y + rect.size.y);
    paint_dashed_line(cx, tl, tr, color, width);
    paint_dashed_line(cx, tr, br, color, width);
    paint_dashed_line(cx, br, bl, color, width);
    paint_dashed_line(cx, bl, tl, color, width);
}

/// Stroke one dashed segment (4 px on / 4 px off, screen space).
pub(crate) fn paint_dashed_line(
    cx: &mut PaintCx<'_>,
    from: Point2D,
    to: Point2D,
    color: Color,
    width: f32,
) {
    const ON: f32 = 4.0;
    const OFF: f32 = 4.0;
    let dx = to.x - from.x;
    let dy = to.y - from.y;
    let len = (dx * dx + dy * dy).sqrt();
    if len <= f32::EPSILON {
        return;
    }
    let (ux, uy) = (dx / len, dy / len);
    let mut t = 0.0;
    while t < len {
        let end = (t + ON).min(len);
        cx.backend.stroke_line(
            Point2D::new(from.x + ux * t, from.y + uy * t),
            Point2D::new(from.x + ux * end, from.y + uy * end),
            color,
            width,
        );
        t = end + OFF;
    }
}
