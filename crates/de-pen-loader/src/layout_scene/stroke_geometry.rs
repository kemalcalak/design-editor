//! Stroke end geometry (#389) for the scene: lines and open paths whose
//! stroke asks for per-end caps or arrowheads get their centre line,
//! end shapes and line-arrow arms precomputed once here, in node-local
//! doc px, so painters and exports never parse or measure per frame.

use std::sync::Arc;

use de_editor_core::render_backend::{Point2D, Rect};
use de_editor_core::stroke_geometry::{stroke_end_geometry, CentreLine, StrokeEnds};
use de_editor_core::vector_edit::{parse_path_data, subpaths_bounds, Subpath};
use jian_ops_schema::node::{PenPathAnchor, PenPathHandle};
use jian_scene::layout_scene::{SceneLineDirection, SceneStrokeGeometry, SceneStrokeHead};

use crate::payload::NodePayload;

/// The precomputed stroke geometry of `node`, or `None` when its stroke
/// draws plain ends (backends handle `cap`) or it has no open stroke.
pub(super) fn scene_stroke_geometry(node: &NodePayload) -> Option<Arc<SceneStrokeGeometry>> {
    let stroke = node.stroke.as_ref()?;
    let ends = StrokeEnds::from_parts(stroke.stroke_start, stroke.stroke_end, stroke.cap.as_ref())?;
    // Rounded vertices (#389) shape the line the ends sit on.
    let subs = match super::corner_path::rounded_path_subpaths(node) {
        Some(subs) => subs,
        None => local_subpaths(node)?,
    };
    // The painters stroke a dashed centre line with the dash's own cap;
    // the same dash rule (`StrokeDash::new`) decides whether it is dashed.
    let dashed = de_editor_core::fills::render_stroke_style(
        None,
        None,
        stroke.dash_pattern.as_deref(),
        stroke.dash_offset,
    )
    .dash
    .is_some();
    let centre = if dashed {
        CentreLine::Dashed
    } else {
        CentreLine::Solid
    };
    let geometry = stroke_end_geometry(&subs, f64::from(stroke.width), ends, centre)?;
    Some(Arc::new(SceneStrokeGeometry {
        stroke_d: geometry.stroke_d,
        fill_d: geometry.fill_d,
        arms_d: geometry.arms_d,
        head_hits: geometry
            .heads
            .iter()
            .map(|head| SceneStrokeHead {
                center: Point2D::new(head.center.0 as f32, head.center.1 as f32),
                radius: head.radius as f32,
            })
            .collect(),
    }))
}

/// The node's outline in node-local doc px, drawn the way the painter
/// draws it: a line runs across its box the way it was drawn (its start
/// stays its start; flips ride on the node transform), a path is its `d`
/// fitted to the box like the backends' `fit_path_to_rect`, else its
/// anchors or points.
pub(super) fn local_subpaths(node: &NodePayload) -> Option<Vec<Subpath>> {
    let (w, h) = (f64::from(node.w), f64::from(node.h));
    match node.kind.as_str() {
        "line" => {
            let direction = SceneLineDirection {
                right_to_left: node.line_right_to_left,
                bottom_to_top: node.line_bottom_to_top,
            };
            let (from, to) = direction.endpoints(Rect::xywh(0.0, 0.0, node.w, node.h));
            Some(vec![Subpath::polyline(&[
                (f64::from(from.x), f64::from(from.y)),
                (f64::from(to.x), f64::from(to.y)),
            ])])
        }
        "path" => {
            if let Some(d) = node.svg_path.as_deref() {
                let mut subs = parse_path_data(d)?;
                fit_to_box(&mut subs, w, h);
                return Some(subs);
            }
            let (ox, oy) = (f64::from(node.x), f64::from(node.y));
            if node.path_anchors.len() >= 2 {
                let rel = |h: Option<[f32; 2]>, x: f32, y: f32| {
                    h.map(|[hx, hy]| PenPathHandle {
                        x: f64::from(hx - x),
                        y: f64::from(hy - y),
                    })
                };
                let anchors = node
                    .path_anchors
                    .iter()
                    .map(|a| PenPathAnchor {
                        x: f64::from(a.x) - ox,
                        y: f64::from(a.y) - oy,
                        handle_in: rel(a.handle_in, a.x, a.y),
                        handle_out: rel(a.handle_out, a.x, a.y),
                        point_type: None,
                    })
                    .collect();
                return Some(vec![Subpath {
                    anchors,
                    closed: node.path_closed,
                }]);
            }
            let points: Vec<(f64, f64)> = node
                .points
                .iter()
                .map(|[x, y]| (f64::from(*x) - ox, f64::from(*y) - oy))
                .collect();
            (points.len() >= 2).then(|| {
                let mut sub = Subpath::polyline(&points);
                sub.closed = node.path_closed;
                vec![sub]
            })
        }
        _ => None,
    }
}

/// Map `subs` from their tight bounds onto `(0, 0, w, h)`; a degenerate
/// axis keeps scale 1 (the backends' rule).
fn fit_to_box(subs: &mut [Subpath], w: f64, h: f64) {
    let (bx, by, bw, bh) = subpaths_bounds(subs);
    if !(bx.is_finite() && by.is_finite()) {
        return;
    }
    let sx = if bw.abs() > 0.01 { w / bw } else { 1.0 };
    let sy = if bh.abs() > 0.01 { h / bh } else { 1.0 };
    for anchor in subs.iter_mut().flat_map(|s| s.anchors.iter_mut()) {
        anchor.x = (anchor.x - bx) * sx;
        anchor.y = (anchor.y - by) * sy;
        for handle in [&mut anchor.handle_in, &mut anchor.handle_out]
            .into_iter()
            .flatten()
        {
            handle.x *= sx;
            handle.y *= sy;
        }
    }
}

#[cfg(test)]
#[path = "stroke_geometry_tests.rs"]
mod tests;
