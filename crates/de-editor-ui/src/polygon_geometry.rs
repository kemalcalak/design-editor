//! Polygon and star vertices for every scene consumer (#87).
//!
//! The canvas painter, the SVG / HTML exporters and the boolean ops all
//! turn a `NodeKind::Polygon` scene node into vertices through
//! [`polygon_node_points`], so a star paints, exports and combines the
//! same way everywhere. A star is a polygon whose scene node carries an
//! inner radius in `arc_inner_radius` (the loader writes it from
//! `PolygonNode::inner_radius`).

use crate::layout_scene::{regular_polygon_points, NodeKind, SceneNode};
use crate::{Point2D, Rect};
use de_editor_core::star_shape::{STAR_MAX_INNER_RADIUS, STAR_MIN_INNER_RADIUS};
use std::f32::consts::{FRAC_PI_2, PI};

/// The star inner radius of a polygon scene node, or `None` for a
/// regular polygon (and for every other kind).
pub fn polygon_star_inner_ratio(node: &SceneNode) -> Option<f32> {
    if node.kind != NodeKind::Polygon {
        return None;
    }
    node.arc_inner_radius
        .filter(|r| r.is_finite())
        .map(|r| r.clamp(STAR_MIN_INNER_RADIUS as f32, STAR_MAX_INNER_RADIUS as f32))
}

/// Vertices of a polygon scene node fitted into `rect`: the regular
/// polygon, or the star when the node carries an inner radius.
pub fn polygon_node_points(node: &SceneNode, rect: Rect) -> Vec<Point2D> {
    match polygon_star_inner_ratio(node) {
        Some(inner) => star_points(rect, node.polygon_sides, inner),
        None => regular_polygon_points(rect, node.polygon_sides),
    }
}

/// `2 × points` star vertices, outer point first at the top, fitted to
/// `rect`: the star is built on a unit circle and its bounding box is
/// stretched onto `rect`, so the points touch every edge (Figma's star
/// geometry, which keeps Figma imports in place).
pub fn star_points(rect: Rect, points: u32, inner_ratio: f32) -> Vec<Point2D> {
    let n = points.clamp(3, 100) as usize;
    let inner = inner_ratio.clamp(STAR_MIN_INNER_RADIUS as f32, STAR_MAX_INNER_RADIUS as f32);
    let step = PI / n as f32;
    let unit: Vec<(f32, f32)> = (0..n * 2)
        .map(|i| {
            let radius = if i % 2 == 0 { 1.0 } else { inner };
            let angle = -FRAC_PI_2 + i as f32 * step;
            (radius * angle.cos(), radius * angle.sin())
        })
        .collect();
    let (mut min_x, mut max_x, mut min_y, mut max_y) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
    for &(x, y) in &unit {
        min_x = min_x.min(x);
        max_x = max_x.max(x);
        min_y = min_y.min(y);
        max_y = max_y.max(y);
    }
    // n >= 3 always spans both axes, but keep a zero-extent guard so a
    // degenerate input can never divide by zero.
    let span_x = (max_x - min_x).max(f32::EPSILON);
    let span_y = (max_y - min_y).max(f32::EPSILON);
    unit.into_iter()
        .map(|(x, y)| {
            Point2D::new(
                rect.origin.x + (x - min_x) / span_x * rect.size.x,
                rect.origin.y + (y - min_y) / span_y * rect.size.y,
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(w: f32, h: f32) -> Rect {
        Rect {
            origin: Point2D::new(10.0, 20.0),
            size: Point2D::new(w, h),
        }
    }

    fn bounds(points: &[Point2D]) -> (f32, f32, f32, f32) {
        points.iter().fold(
            (f32::MAX, f32::MIN, f32::MAX, f32::MIN),
            |(a, b, c, d), p| (a.min(p.x), b.max(p.x), c.min(p.y), d.max(p.y)),
        )
    }

    #[test]
    fn a_star_has_two_vertices_per_point_and_fills_its_box() {
        let pts = star_points(rect(100.0, 80.0), 5, 0.382);
        assert_eq!(pts.len(), 10);
        let (min_x, max_x, min_y, max_y) = bounds(&pts);
        assert!((min_x - 10.0).abs() < 1e-3 && (max_x - 110.0).abs() < 1e-3);
        assert!((min_y - 20.0).abs() < 1e-3 && (max_y - 100.0).abs() < 1e-3);
        // The first point is the top tip, horizontally centred.
        assert!((pts[0].x - 60.0).abs() < 1e-3 && (pts[0].y - 20.0).abs() < 1e-3);
    }

    #[test]
    fn valleys_sit_at_the_inner_radius() {
        let r = rect(200.0, 200.0);
        let pts = star_points(r, 4, 0.5);
        // A 4-point star is symmetric, so the box centre is the star centre
        // and the outer radius is half the box.
        let (cx, cy) = (110.0, 120.0);
        let dist = |p: &Point2D| ((p.x - cx).powi(2) + (p.y - cy).powi(2)).sqrt();
        assert!((dist(&pts[0]) - 100.0).abs() < 1e-2);
        assert!((dist(&pts[1]) - 50.0).abs() < 1e-2);
    }

    #[test]
    fn point_count_and_ratio_are_clamped() {
        assert_eq!(star_points(rect(10.0, 10.0), 1, 0.5).len(), 6);
        assert_eq!(star_points(rect(10.0, 10.0), 500, 0.5).len(), 200);
        assert!(star_points(rect(10.0, 10.0), 5, -3.0)
            .iter()
            .all(|p| p.x.is_finite() && p.y.is_finite()));
    }

    #[test]
    fn polygon_nodes_pick_star_or_regular_geometry() {
        let mut node = SceneNode::leaf("p", NodeKind::Polygon);
        node.polygon_sides = 6;
        assert_eq!(polygon_star_inner_ratio(&node), None);
        assert_eq!(polygon_node_points(&node, rect(10.0, 10.0)).len(), 6);
        node.arc_inner_radius = Some(0.4);
        assert_eq!(polygon_star_inner_ratio(&node), Some(0.4));
        assert_eq!(polygon_node_points(&node, rect(10.0, 10.0)).len(), 12);
        node.kind = NodeKind::Ellipse;
        assert_eq!(polygon_star_inner_ratio(&node), None);
    }
}
