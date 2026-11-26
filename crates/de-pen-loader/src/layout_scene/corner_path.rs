//! Rounded / smoothed outlines (#389) for the scene: rect-like nodes with
//! corner smoothing, polygons and stars with a corner radius, and paths
//! whose sharp vertices round. Built once here, in node-local doc px, so
//! painters, clips, hit-test and exports never rebuild them per frame.
//!
//! A rect with circular corners (smoothing `0`) gets no outline: the
//! rounded-rect primitives already draw it, byte for byte as before.

use std::sync::Arc;

use de_editor_core::corner_geometry::{
    flatten_outline, has_smoothing, outline_path_data, polygon_outline, rect_outline, round_corners,
};
use de_editor_core::render_backend::{Point2D, Rect};
use de_editor_core::vector_edit::{parse_path_data, subpaths_bounds, Subpath};
use jian_scene::layout_scene::{polygon_points, SceneCornerPath};

use crate::payload::NodePayload;

/// Curved segments are cut into this many pieces for hit-testing.
const HIT_STEPS: usize = 8;

/// The outline `node` paints instead of its plain shape, or `None` when
/// the plain shape is exact.
pub(super) fn scene_corner_path(node: &NodePayload) -> Option<Arc<SceneCornerPath>> {
    let (w, h) = (f64::from(node.w), f64::from(node.h));
    if !(w > 0.0 && h > 0.0) {
        return None;
    }
    let smoothing = f64::from(node.corner_smoothing);
    match node.kind.as_str() {
        "frame" | "rect" | "group" => {
            let radii = node_radii(node);
            if !has_smoothing(smoothing) || radii.iter().all(|r| *r <= 0.0) {
                return None;
            }
            let outline = rect_outline(w, h, radii, smoothing)?;
            let stroke = aligned_stroke_outline(node, w, h, radii, smoothing);
            Some(Arc::new(build(&[outline], stroke.as_ref())))
        }
        "polygon" => {
            let radius = f64::from(node.corner_radius);
            if radius <= 0.0 {
                return None;
            }
            let points: Vec<(f64, f64)> = polygon_points(
                Rect::xywh(0.0, 0.0, node.w, node.h),
                node.polygon_sides,
                node.arc_inner_radius,
            )
            .iter()
            .map(|p| (f64::from(p.x), f64::from(p.y)))
            .collect();
            let outline = polygon_outline(&points, radius, smoothing)?;
            Some(Arc::new(build(&[outline], None)))
        }
        "path" => {
            let subs = rounded_path_subpaths(node)?;
            Some(Arc::new(build(&subs, None)))
        }
        _ => None,
    }
}

/// A path node's subpaths in node-local doc px with its vertices rounded,
/// or `None` when it has no radius or no vertex rounds. The stroke-end
/// geometry follows the same rounded outline.
pub(super) fn rounded_path_subpaths(node: &NodePayload) -> Option<Vec<Subpath>> {
    let radius = f64::from(node.corner_radius);
    if node.kind != "path" || radius <= 0.0 {
        return None;
    }
    let subs = super::stroke_geometry::local_subpaths(node)?;
    let smoothing = f64::from(node.corner_smoothing);
    let rounded: Vec<Subpath> = subs
        .iter()
        .map(|sub| round_corners(sub, &vec![radius; sub.anchors.len()], smoothing))
        .collect();
    (rounded != subs).then_some(rounded)
}

/// Corner radii in top-left, top-right, bottom-right, bottom-left order.
fn node_radii(node: &NodePayload) -> [f64; 4] {
    node.corner_radii
        .unwrap_or([node.corner_radius; 4])
        .map(|r| {
            if r.is_finite() {
                f64::from(r).max(0.0)
            } else {
                0.0
            }
        })
}

/// The centre line of an inside / outside stroke: the outline of the box
/// shifted by half the stroke width, radii following it (the rounded-rect
/// stroke's own rule). `None` for a centred, sided or absent stroke.
fn aligned_stroke_outline(
    node: &NodePayload,
    w: f64,
    h: f64,
    radii: [f64; 4],
    smoothing: f64,
) -> Option<Subpath> {
    let stroke = node.stroke.as_ref()?;
    let width = f64::from(stroke.width);
    if !(width.is_finite() && width > 0.0) || stroke.align == 0 {
        return None;
    }
    if let Some(sides) = stroke.sides {
        if sides.iter().any(|s| (*s - sides[0]).abs() > f32::EPSILON) {
            return None;
        }
    }
    let half = width / 2.0;
    let (offset, radii) = if stroke.align < 0 {
        (half, radii.map(|r| (r - half).max(0.0)))
    } else {
        (-half, radii.map(|r| if r > 0.0 { r + half } else { 0.0 }))
    };
    let mut outline = rect_outline(w - 2.0 * offset, h - 2.0 * offset, radii, smoothing)?;
    for anchor in &mut outline.anchors {
        anchor.x += offset;
        anchor.y += offset;
    }
    Some(outline)
}

fn build(subs: &[Subpath], stroke: Option<&Subpath>) -> SceneCornerPath {
    let d = outline_path_data(subs);
    let (stroke_d, stroke_bounds) = match stroke {
        Some(sub) => {
            let d = outline_path_data(std::slice::from_ref(sub));
            let bounds = data_bounds(&d);
            (d, bounds)
        }
        None => (String::new(), Rect::ZERO),
    };
    // Hit-testing reads one ring; a compound path keeps the plain test.
    let hit_ring = match subs {
        [single] => {
            let mut ring: Vec<Point2D> = flatten_outline(single, HIT_STEPS)
                .into_iter()
                .map(|(x, y)| Point2D::new(x as f32, y as f32))
                .collect();
            if single.closed {
                if let Some(first) = ring.first().copied() {
                    ring.push(first);
                }
            }
            ring
        }
        _ => Vec::new(),
    };
    SceneCornerPath {
        bounds: data_bounds(&d),
        d,
        stroke_d,
        stroke_bounds,
        hit_ring,
    }
}

/// Tight bounds of written path data — parsed back so they match the
/// rounded coordinates the backends fit.
fn data_bounds(d: &str) -> Rect {
    let Some(subs) = parse_path_data(d) else {
        return Rect::ZERO;
    };
    let (x, y, w, h) = subpaths_bounds(&subs);
    Rect::xywh(x as f32, y as f32, w as f32, h as f32)
}

#[cfg(test)]
#[path = "corner_path_tests.rs"]
mod tests;
