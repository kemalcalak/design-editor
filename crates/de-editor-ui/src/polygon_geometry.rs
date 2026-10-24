//! Polygon and star vertices for the editor's scene consumers (#87).
//!
//! The canvas painter, the SVG / HTML exporters and the boolean ops turn
//! a `NodeKind::Polygon` scene node into vertices through
//! [`polygon_node_points`]; jian-scene's hit-test calls the same
//! `polygon_points` geometry, so a star paints, exports, combines and
//! clicks along one outline. A star is a polygon whose scene node carries
//! an inner radius in `arc_inner_radius` (the loader writes it from
//! `PolygonNode::inner_radius`); on every other kind that field is the
//! ellipse donut hole, so read the star radius only through
//! [`polygon_star_inner_radius`].

use crate::layout_scene::{
    polygon_points, NodeKind, SceneNode, STAR_MAX_INNER_RADIUS, STAR_MIN_INNER_RADIUS,
};
use crate::{Point2D, Rect};

/// The star inner radius of a polygon scene node, or `None` for a
/// regular polygon (and for every other kind).
pub fn polygon_star_inner_radius(node: &SceneNode) -> Option<f32> {
    if node.kind != NodeKind::Polygon {
        return None;
    }
    node.arc_inner_radius
        .filter(|r| r.is_finite())
        .map(|r| r.clamp(STAR_MIN_INNER_RADIUS, STAR_MAX_INNER_RADIUS))
}

/// Vertices of a polygon scene node fitted into `rect`: the regular
/// polygon, or the star when the node carries an inner radius.
pub fn polygon_node_points(node: &SceneNode, rect: Rect) -> Vec<Point2D> {
    polygon_points(rect, node.polygon_sides, polygon_star_inner_radius(node))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polygon_nodes_pick_star_or_regular_geometry() {
        let rect = Rect::xywh(0.0, 0.0, 10.0, 10.0);
        let mut node = SceneNode::leaf("p", NodeKind::Polygon);
        node.polygon_sides = 6;
        assert_eq!(polygon_star_inner_radius(&node), None);
        assert_eq!(polygon_node_points(&node, rect).len(), 6);
        node.arc_inner_radius = Some(0.4);
        assert_eq!(polygon_star_inner_radius(&node), Some(0.4));
        assert_eq!(polygon_node_points(&node, rect).len(), 12);
        node.arc_inner_radius = Some(-1.0);
        assert_eq!(
            polygon_star_inner_radius(&node),
            Some(STAR_MIN_INNER_RADIUS)
        );
        // On an ellipse the same field is the donut hole, not a star.
        node.kind = NodeKind::Ellipse;
        assert_eq!(polygon_star_inner_radius(&node), None);
    }

    #[test]
    fn the_star_range_matches_the_editor() {
        use de_editor_core::star_shape as core;
        assert!((STAR_MIN_INNER_RADIUS as f64 - core::STAR_MIN_INNER_RADIUS).abs() < 1e-6);
        assert!((STAR_MAX_INNER_RADIUS as f64 - core::STAR_MAX_INNER_RADIUS).abs() < 1e-6);
    }
}
