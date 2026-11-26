//! Scene node → skia geometry, in document space.
//!
//! [`node_geometry`] is the shape as authored: a rect with its corner
//! radii, an ellipse (or its arc / donut sector) as conics, a polygon or
//! star, a path's own curves, a line's centre line. [`node_outline`] is
//! the closed area a boolean combines: open geometry (a line, an open
//! path) contributes the outline of its stroke instead, and a group the
//! union of its children. Rotation and flips are applied about the same
//! pivot the canvas painter uses.

use crate::stroke::stroke_to_fill;
use de_editor_core::render_backend::{Point2D, Rect};
use jian_scene::layout_scene::{
    polygon_points, NodeKind, SceneNode, STAR_MAX_INNER_RADIUS, STAR_MIN_INNER_RADIUS,
};
use skia_safe::{Matrix, Path, PathBuilder, PathFillType, PathOp, RRect, Rect as SkRect, Vector};

/// A node's own geometry and whether it encloses an area.
pub struct Geometry {
    pub path: Path,
    pub closed: bool,
}

/// The node's geometry (doc space, transforms applied), or `None` for
/// kinds without vector geometry (text, images-as-icons, …) and hidden
/// nodes. Groups have none of their own; see [`node_outline`].
pub fn node_geometry(node: &SceneNode) -> Option<Geometry> {
    if node.hidden {
        return None;
    }
    let b = node.bounds;
    let (path, closed) = match &node.kind {
        NodeKind::Frame | NodeKind::Rect => (rect_path(node, b)?, true),
        NodeKind::Ellipse => (ellipse_path(node, b)?, true),
        NodeKind::Polygon => (polyline(&polygon_node_points(node), true)?, true),
        NodeKind::Path => path_geometry(node)?,
        NodeKind::Line => (line_path(node)?, false),
        NodeKind::Group | NodeKind::Text | NodeKind::Other(_) => return None,
    };
    Some(Geometry {
        path: apply_transform(path, node),
        closed,
    })
}

/// The closed area `node` covers for a boolean: its geometry when closed,
/// its stroke's outline when open, the union of its children for a
/// group. `None` when it covers nothing.
pub fn node_outline(node: &SceneNode) -> Option<Path> {
    if node.hidden {
        return None;
    }
    if node.kind == NodeKind::Group {
        let parts: Vec<Path> = node.children.iter().filter_map(node_outline).collect();
        return union_all(&parts);
    }
    let geometry = node_geometry(node)?;
    if geometry.closed {
        return Some(geometry.path);
    }
    let stroke = node.stroke.filter(|s| s.width > 0.0)?;
    stroke_to_fill(&geometry.path, stroke.width, &stroke.style)
}

/// Union of `paths`; `None` when empty or skia gives up.
pub fn union_all(paths: &[Path]) -> Option<Path> {
    let (first, rest) = paths.split_first()?;
    rest.iter()
        .try_fold(first.clone(), |acc, p| acc.op(p, PathOp::Union))
}

fn sk_rect(b: Rect) -> SkRect {
    SkRect::from_xywh(b.origin.x, b.origin.y, b.size.x, b.size.y)
}

fn has_area(b: Rect) -> bool {
    b.size.x > 0.0 && b.size.y > 0.0 && b.size.x.is_finite() && b.size.y.is_finite()
}

/// A rect / frame: corner radii (per corner when authored) clamped to
/// half the short side, as the painter rounds them.
fn rect_path(node: &SceneNode, b: Rect) -> Option<Path> {
    if !has_area(b) {
        return None;
    }
    let max = b.size.x.min(b.size.y) / 2.0;
    let radii = node
        .corner_radii
        .unwrap_or([node.corner_radius; 4])
        .map(|r| r.clamp(0.0, max));
    let mut builder = PathBuilder::new();
    if radii.iter().all(|r| *r <= 0.0) {
        builder.add_rect(sk_rect(b), None, None);
    } else {
        let v = |r: f32| Vector::new(r, r);
        let rr = RRect::new_rect_radii(
            sk_rect(b),
            &[v(radii[0]), v(radii[1]), v(radii[2]), v(radii[3])],
        );
        builder.add_rrect(rr, None, None);
    }
    Some(builder.detach())
}

/// An ellipse: the full oval, or the arc / pie / donut sector the node
/// authors (`0°` = +X, clockwise), all as conics.
fn ellipse_path(node: &SceneNode, b: Rect) -> Option<Path> {
    if !has_area(b) {
        return None;
    }
    let inner = node.arc_inner_radius.unwrap_or(0.0).clamp(0.0, 1.0);
    let start = node.arc_start_angle.unwrap_or(0.0);
    let sweep = node.arc_sweep_angle.unwrap_or(360.0).clamp(-360.0, 360.0);
    let full = sweep.abs() >= 359.9;
    let oval = sk_rect(b);
    let inner_oval = || {
        let (w, h) = (b.size.x * inner, b.size.y * inner);
        SkRect::from_xywh(
            b.origin.x + (b.size.x - w) / 2.0,
            b.origin.y + (b.size.y - h) / 2.0,
            w,
            h,
        )
    };
    let mut builder = PathBuilder::new();
    if full {
        builder.add_oval(oval, None, None);
        if inner > 0.001 {
            builder.add_oval(inner_oval(), skia_safe::PathDirection::CCW, None);
        }
        let mut path = builder.detach();
        path.set_fill_type(PathFillType::EvenOdd);
        return Some(path);
    }
    if sweep.abs() < 0.01 {
        return None;
    }
    if inner > 0.001 {
        builder.arc_to(oval, start, sweep, true);
        builder.arc_to(inner_oval(), start + sweep, -sweep, false);
    } else {
        let centre = (b.origin.x + b.size.x / 2.0, b.origin.y + b.size.y / 2.0);
        builder.move_to(centre);
        builder.arc_to(oval, start, sweep, false);
    }
    builder.close();
    Some(builder.detach())
}

fn polygon_node_points(node: &SceneNode) -> Vec<Point2D> {
    let star = node
        .arc_inner_radius
        .filter(|r| r.is_finite())
        .map(|r| r.clamp(STAR_MIN_INNER_RADIUS, STAR_MAX_INNER_RADIUS));
    polygon_points(node.bounds, node.polygon_sides, star)
}

fn polyline(points: &[Point2D], close: bool) -> Option<Path> {
    let (first, rest) = points.split_first()?;
    if rest.is_empty() {
        return None;
    }
    let mut builder = PathBuilder::new();
    builder.move_to((first.x, first.y));
    for p in rest {
        builder.line_to((p.x, p.y));
    }
    if close {
        builder.close();
    }
    Some(builder.detach())
}

/// A path: its compound `d` fitted into the box (as the painter fits it),
/// else its anchors with their handles, else its flat points.
fn path_geometry(node: &SceneNode) -> Option<(Path, bool)> {
    if let Some(d) = node.svg_path.as_deref() {
        let parsed = skia_safe::utils::parse_path::from_svg(d)?;
        if parsed.is_empty() {
            return None;
        }
        let mut path = fit_path_to_rect(&parsed, node.bounds);
        let contours = d.bytes().filter(|c| *c == b'Z' || *c == b'z').count();
        if node.even_odd_fill || contours >= 2 {
            path.set_fill_type(PathFillType::EvenOdd);
        }
        // A `d` without any close is an open stroke path.
        return Some((path, contours > 0));
    }
    let anchors = &node.path_anchors;
    if anchors.len() >= 2 {
        let mut builder = PathBuilder::new();
        builder.move_to((anchors[0].pos.x, anchors[0].pos.y));
        let seg = |b: &mut PathBuilder,
                   a: &jian_scene::layout_scene::SceneAnchor,
                   n: &jian_scene::layout_scene::SceneAnchor| {
            match (a.handle_out, n.handle_in) {
                (None, None) => {
                    b.line_to((n.pos.x, n.pos.y));
                }
                (o, i) => {
                    let c1 = o.unwrap_or(a.pos);
                    let c2 = i.unwrap_or(n.pos);
                    b.cubic_to((c1.x, c1.y), (c2.x, c2.y), (n.pos.x, n.pos.y));
                }
            }
        };
        for pair in anchors.windows(2) {
            seg(&mut builder, &pair[0], &pair[1]);
        }
        if node.path_closed {
            seg(&mut builder, &anchors[anchors.len() - 1], &anchors[0]);
            builder.close();
        }
        return Some((builder.detach(), node.path_closed));
    }
    Some((polyline(&node.points, node.path_closed)?, node.path_closed))
}

/// A line: its two points, else the box diagonal.
fn line_path(node: &SceneNode) -> Option<Path> {
    if node.points.len() >= 2 {
        return polyline(&node.points, false);
    }
    // Start to end as drawn: an up-right line is not its box's
    // top-left → bottom-right diagonal (#389).
    let (from, to) = node.line_endpoints();
    polyline(&[from, to], false)
}

/// Scale + translate `path` so its tight bounds fill `rect`: the canvas
/// painter's fit for a node's `d`, rule for rule (de-host-native
/// `backend/skia/path.rs`), so outlines land on the painted pixels. A
/// negative side, or a zero side over a path with extent on that axis,
/// only translates.
pub fn fit_path_to_rect(path: &Path, rect: Rect) -> Path {
    let bounds = path.compute_tight_bounds();
    let (native_w, native_h) = (bounds.width(), bounds.height());
    let degenerate = |side: f32, native: f32| side < 0.0 || (side == 0.0 && native.abs() > 0.01);
    if !bounds.is_finite()
        || !rect.size.x.is_finite()
        || !rect.size.y.is_finite()
        || degenerate(rect.size.x, native_w)
        || degenerate(rect.size.y, native_h)
    {
        let mut m = Matrix::new_identity();
        m.set_translate((rect.origin.x, rect.origin.y));
        return path.with_transform(&m);
    }
    let scale = |side: f32, native: f32| {
        if native.abs() > 0.01 {
            side / native
        } else {
            1.0
        }
    };
    let (sx, sy) = (scale(rect.size.x, native_w), scale(rect.size.y, native_h));
    let mut m = Matrix::new_identity();
    m.set_scale_translate(
        (sx, sy),
        (
            rect.origin.x - bounds.left() * sx,
            rect.origin.y - bounds.top() * sy,
        ),
    );
    path.with_transform(&m)
}

/// Rotation (clockwise radians) and flips about the node's aggregate
/// bounds centre — the canvas painter's pivot.
fn apply_transform(path: Path, node: &SceneNode) -> Path {
    let rotated = node.rotation.abs() > f32::EPSILON;
    if !rotated && !node.flip_x && !node.flip_y {
        return path;
    }
    let b = node.aggregate_bounds();
    let pivot = (b.origin.x + b.size.x / 2.0, b.origin.y + b.size.y / 2.0);
    let mut m = Matrix::new_identity();
    // The painter concatenates the flip, then the rotation: a point is
    // rotated first and mirrored after.
    m.pre_translate(pivot);
    m.pre_scale(
        (
            if node.flip_x { -1.0 } else { 1.0 },
            if node.flip_y { -1.0 } else { 1.0 },
        ),
        None,
    );
    if rotated {
        m.pre_rotate(node.rotation.to_degrees(), None);
    }
    m.pre_translate((-pivot.0, -pivot.1));
    path.with_transform(&m)
}

#[cfg(test)]
#[path = "outline_tests.rs"]
mod tests;
