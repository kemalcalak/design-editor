//! Vector edit mode overlay (#387): dims the rest of the canvas, then
//! paints the edited path's outline, its points (selected ones filled),
//! the handles in play, the marquee / lasso, snap guides and the pen's
//! rubber band.

use super::canvas_overlay_transform::OverlayTransform;
use super::canvas_path_overlay::{
    dashed_line, dot, stroke_polyline, to_screen_point, with_node_overlay_transform, SELECTION_BLUE,
};
use super::vector_edit_view::VectorEditView;
use super::PaintCx;
use crate::layout_scene::{LayoutScene, SceneNode};
use crate::{Color, Point2D, Rect};
use de_editor_core::vector_edit::{cubic_point, segment_controls, segment_count};
use de_editor_core::{EditorState, Tool, Viewport};

const WHITE: Color = Color {
    r: 1.0,
    g: 1.0,
    b: 1.0,
    a: 1.0,
};
const HANDLE_LINE: Color = Color {
    r: 0x88 as f32 / 255.0,
    g: 0x88 as f32 / 255.0,
    b: 0x88 as f32 / 255.0,
    a: 1.0,
};
/// Smart-guide magenta, the node-drag guides' colour.
const GUIDE: Color = Color {
    r: 0.93,
    g: 0.25,
    b: 0.6,
    a: 1.0,
};
/// How much of the canvas background veils everything but the path.
const DIM_ALPHA: f32 = 0.6;

/// Everything the overlay needs, captured from the editor per paint.
#[derive(Debug, Clone, PartialEq)]
pub struct VectorEditPaint {
    pub view: VectorEditView,
    pub marquee: Vec<Point2D>,
    pub guides: Vec<(Point2D, Point2D)>,
    pub pen_cursor: Option<Point2D>,
    pub pen_anchor: Option<Point2D>,
}

impl VectorEditPaint {
    pub fn build(state: &EditorState, scene: &LayoutScene) -> Option<Self> {
        let view = VectorEditView::build(state, scene)?;
        let session = state.vector_edit()?;
        let p = |(x, y): (f64, f64)| Point2D::new(x as f32, y as f32);
        let pen_on = matches!(state.tool, Tool::Pen) && session.pen_continue.is_some();
        Some(Self {
            marquee: session.marquee.iter().copied().map(p).collect(),
            guides: session
                .guides
                .iter()
                .map(|g| (p(g.from), p(g.to)))
                .collect(),
            pen_cursor: session.pen_cursor.filter(|_| pen_on).map(p),
            pen_anchor: state
                .vector_pen_anchor()
                .filter(|_| pen_on)
                .map(|a| view.to_scene(a)),
            view,
        })
    }
}

/// Veil the canvas and repaint the edited node on top, so only it reads
/// at full strength. Runs right after the scene walk.
#[allow(clippy::too_many_arguments)]
pub(super) fn paint_dim_and_node(
    cx: &mut PaintCx<'_>,
    page_node: Option<&SceneNode>,
    canvas_rect: Rect,
    background: Color,
    viewport_origin: Point2D,
    zoom: f32,
    cull: Rect,
) {
    cx.backend.fill_rect(
        canvas_rect,
        Color {
            a: DIM_ALPHA,
            ..background
        },
    );
    if let Some(node) = page_node {
        let _ = super::canvas_viewport_paint::paint_node_with_options(
            cx,
            node,
            viewport_origin,
            zoom,
            None,
            cull,
            None,
            None,
            None,
            None,
        );
    }
}

/// The mode's handles and chrome. `node` is the edited scene node;
/// `transforms` its traversal-captured transform chain.
pub(super) fn paint_vector_edit_overlay(
    cx: &mut PaintCx<'_>,
    paint: &VectorEditPaint,
    node: Option<&SceneNode>,
    transforms: &[OverlayTransform],
    canvas_rect: Rect,
    viewport: &Viewport,
) {
    let view = &paint.view;
    let ts = |p: (f64, f64)| to_screen_point(view.to_scene(p), canvas_rect, viewport);
    let body = |cx: &mut PaintCx<'_>| {
        for sub in &view.subs {
            let mut pts = Vec::new();
            for seg in 0..segment_count(sub) {
                let Some(c) = segment_controls(sub, seg) else {
                    continue;
                };
                if pts.is_empty() {
                    pts.push(ts(c[0]));
                }
                let straight = c[1] == c[0] && c[2] == c[3];
                let steps = if straight { 1 } else { 24 };
                for i in 1..=steps {
                    pts.push(ts(cubic_point(c, i as f64 / steps as f64)));
                }
            }
            stroke_polyline(cx, &pts, SELECTION_BLUE, 1.5);
        }
        for (p, side) in view.visible_handles() {
            if let (Some(a), Some(h)) = (view.point(p), view.handle(p, side)) {
                cx.backend.stroke_line(ts(a), ts(h), HANDLE_LINE, 1.0);
                dot(cx, ts(h), 3.0, WHITE, SELECTION_BLUE, 1.0);
            }
        }
        for (si, sub) in view.subs.iter().enumerate() {
            for (i, a) in sub.anchors.iter().enumerate() {
                let p = de_editor_core::vector_edit::PointRef::new(si, i);
                let (fill, line) = if view.selected.contains(&p) {
                    (SELECTION_BLUE, WHITE)
                } else {
                    (WHITE, SELECTION_BLUE)
                };
                dot(cx, ts((a.x, a.y)), 4.0, fill, line, 1.5);
            }
        }
    };
    match node {
        Some(node) => {
            with_node_overlay_transform(cx, node, canvas_rect, viewport, transforms, body)
        }
        None => body(cx),
    }
    if let (Some(from), Some(to)) = (paint.pen_anchor, paint.pen_cursor) {
        dashed_line(
            cx,
            to_screen_point(from, canvas_rect, viewport),
            to_screen_point(to, canvas_rect, viewport),
            Color {
                a: 0.5,
                ..SELECTION_BLUE
            },
            1.0,
        );
    }
    for (from, to) in &paint.guides {
        cx.backend.stroke_line(
            to_screen_point(*from, canvas_rect, viewport),
            to_screen_point(*to, canvas_rect, viewport),
            GUIDE,
            1.0,
        );
    }
    if paint.marquee.len() >= 3 {
        let pts: Vec<Point2D> = paint
            .marquee
            .iter()
            .map(|p| to_screen_point(*p, canvas_rect, viewport))
            .collect();
        cx.backend.fill_polygon(
            &pts,
            Color {
                a: 0.08,
                ..SELECTION_BLUE
            },
        );
        let mut closed = pts.clone();
        closed.push(pts[0]);
        stroke_polyline(cx, &closed, SELECTION_BLUE, 1.0);
    }
}
