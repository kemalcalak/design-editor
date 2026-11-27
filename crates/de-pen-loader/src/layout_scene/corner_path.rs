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
use de_editor_core::vector_edit::{subpaths_bounds, Subpath};
use jian_scene::layout_scene::{
    polygon_points, NodeKind, SceneCornerPath, SceneNode, SceneStrokeAlign,
};

use crate::payload::NodePayload;

/// Curved segments are cut into this many pieces for hit-testing.
const HIT_STEPS: usize = 8;

/// The outline `node` paints instead of its plain shape, or `None` when
/// the plain shape is exact.
pub(super) fn scene_corner_path(node: &NodePayload) -> Option<Arc<SceneCornerPath>> {
    if node.kind == "path" {
        if !(node.w > 0.0 && node.h > 0.0) {
            return None;
        }
        let subs = rounded_path_subpaths(node)?;
        return Some(Arc::new(build(&subs, None)));
    }
    let shape = match node.kind.as_str() {
        "frame" | "rect" | "group" => BoxShape::Rect,
        "polygon" => BoxShape::Polygon,
        _ => return None,
    };
    box_corner_path(&BoxCorners {
        shape,
        w: f64::from(node.w),
        h: f64::from(node.h),
        radius: node.corner_radius,
        radii: node.corner_radii,
        smoothing: node.corner_smoothing,
        polygon_sides: node.polygon_sides,
        inner_radius: node.arc_inner_radius,
        stroke: node.stroke.as_ref().map(|s| (s.width, s.align, s.sides)),
    })
}

/// The outline of a rect / frame / group or polygon scene node rebuilt
/// for the box it has now, from the corner fields the scene already
/// holds. A live resize calls it for the one node it moves, so it never
/// rebuilds the whole scene for a smoothed card or a rounded star. A path
/// is not handled here (its outline comes from its `d`): `None`.
pub fn rebuild_box_corner_path(node: &SceneNode) -> Option<Arc<SceneCornerPath>> {
    let shape = match node.kind {
        NodeKind::Frame | NodeKind::Rect | NodeKind::Group => BoxShape::Rect,
        NodeKind::Polygon => BoxShape::Polygon,
        _ => return None,
    };
    box_corner_path(&BoxCorners {
        shape,
        w: f64::from(node.bounds.size.x),
        h: f64::from(node.bounds.size.y),
        radius: node.corner_radius,
        radii: node.corner_radii,
        smoothing: node.corner_smoothing,
        polygon_sides: node.polygon_sides,
        inner_radius: node.arc_inner_radius,
        stroke: node.stroke.map(|s| {
            let align = match s.align {
                SceneStrokeAlign::Inside => -1,
                SceneStrokeAlign::Center => 0,
                SceneStrokeAlign::Outside => 1,
            };
            (s.width, align, s.sides)
        }),
    })
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum BoxShape {
    Rect,
    Polygon,
}

/// What a box-shaped outline is built from: the payload while the scene
/// is built, the scene node when a live resize moves its box.
struct BoxCorners {
    shape: BoxShape,
    w: f64,
    h: f64,
    radius: f32,
    radii: Option<[f32; 4]>,
    smoothing: f32,
    polygon_sides: u32,
    inner_radius: Option<f32>,
    /// Width, alignment (`< 0` inside, `> 0` outside) and per-side widths.
    stroke: Option<(f32, i8, Option<[f32; 4]>)>,
}

fn box_corner_path(c: &BoxCorners) -> Option<Arc<SceneCornerPath>> {
    let (w, h) = (c.w, c.h);
    if !(w > 0.0 && h > 0.0) {
        return None;
    }
    let smoothing = f64::from(c.smoothing);
    match c.shape {
        BoxShape::Rect => {
            let radii = corner_radii(c);
            if !has_smoothing(smoothing) || radii.iter().all(|r| *r <= 0.0) {
                return None;
            }
            let outline = rect_outline(w, h, radii, smoothing)?;
            let stroke = aligned_stroke_outline(c.stroke, w, h, radii, smoothing);
            Some(Arc::new(build(&[outline], stroke.as_ref())))
        }
        BoxShape::Polygon => {
            let radius = f64::from(c.radius);
            if radius <= 0.0 {
                return None;
            }
            let points: Vec<(f64, f64)> = polygon_points(
                Rect::xywh(0.0, 0.0, w as f32, h as f32),
                c.polygon_sides,
                c.inner_radius,
            )
            .iter()
            .map(|p| (f64::from(p.x), f64::from(p.y)))
            .collect();
            let outline = polygon_outline(&points, radius, smoothing)?;
            Some(Arc::new(build(&[outline], None)))
        }
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
fn corner_radii(c: &BoxCorners) -> [f64; 4] {
    c.radii.unwrap_or([c.radius; 4]).map(|r| {
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
    stroke: Option<(f32, i8, Option<[f32; 4]>)>,
    w: f64,
    h: f64,
    radii: [f64; 4],
    smoothing: f64,
) -> Option<Subpath> {
    let (width, align, sides) = stroke?;
    let width = f64::from(width);
    if !(width.is_finite() && width > 0.0) || align == 0 {
        return None;
    }
    if let Some(sides) = sides {
        if sides.iter().any(|s| (*s - sides[0]).abs() > f32::EPSILON) {
            return None;
        }
    }
    let half = width / 2.0;
    let (offset, radii) = if align < 0 {
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
        Some(sub) => (
            outline_path_data(std::slice::from_ref(sub)),
            outline_bounds(std::slice::from_ref(sub)),
        ),
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
        bounds: outline_bounds(subs),
        d,
        stroke_d,
        stroke_bounds,
        hit_ring,
    }
}

/// Tight bounds of the outline, from the subpaths themselves (curve
/// extrema included), the same box `d` spans.
fn outline_bounds(subs: &[Subpath]) -> Rect {
    let (x, y, w, h) = subpaths_bounds(subs);
    Rect::xywh(x as f32, y as f32, w as f32, h as f32)
}

#[cfg(test)]
#[path = "corner_path_tests.rs"]
mod tests;
