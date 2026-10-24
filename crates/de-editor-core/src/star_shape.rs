//! Star shape parameters (#87).
//!
//! A star is a `PolygonNode` whose `inner_radius` is set: `polygon_count`
//! is its point count and `inner_radius` the valley radius as a fraction
//! of the outer radius (Figma `STAR` / `starInnerScale`). `None` keeps the
//! node a regular polygon. Every writer clamps through this module so the
//! panel, MCP and Figma import agree on the range.

/// Inner radius a fresh Star tool node starts with — Figma's default
/// (38.2%, the golden-ratio star).
pub const STAR_DEFAULT_INNER_RADIUS: f64 = 0.382;

/// Point count a fresh Star tool node starts with.
pub const STAR_DEFAULT_POINTS: u32 = 5;

/// Smallest stored inner radius. Zero would collapse every valley onto
/// the centre and leave a fill-less spike set.
pub const STAR_MIN_INNER_RADIUS: f64 = 0.01;

/// Largest stored inner radius: at 1.0 the valleys sit on the outer
/// circle, which draws a regular `2 × points`-gon.
pub const STAR_MAX_INNER_RADIUS: f64 = 1.0;

/// Clamp an inner radius into the stored range. `None` for a non-finite
/// value so callers reject it instead of writing NaN into the document.
pub fn clamp_star_inner_radius(value: f64) -> Option<f64> {
    value
        .is_finite()
        .then(|| value.clamp(STAR_MIN_INNER_RADIUS, STAR_MAX_INNER_RADIUS))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inner_radius_clamps_into_the_stored_range() {
        assert_eq!(clamp_star_inner_radius(0.5), Some(0.5));
        assert_eq!(clamp_star_inner_radius(0.0), Some(STAR_MIN_INNER_RADIUS));
        assert_eq!(clamp_star_inner_radius(7.0), Some(STAR_MAX_INNER_RADIUS));
        assert_eq!(clamp_star_inner_radius(f64::NAN), None);
        assert_eq!(clamp_star_inner_radius(f64::INFINITY), None);
    }

    fn polygon(state: &crate::EditorState) -> &jian_ops_schema::node::PolygonNode {
        match state.active_children().first() {
            Some(jian_ops_schema::node::PenNode::Polygon(p)) => p,
            other => panic!("expected a polygon, got {other:?}"),
        }
    }

    fn state_with(tool: crate::Tool) -> crate::EditorState {
        let mut state = crate::EditorState::new();
        let mut next = 100u64;
        let id = state
            .create_node_for_tool(tool, &mut next, 0.0, 0.0, 120.0, 120.0)
            .expect("shape created");
        state.selection.anchor = id;
        state
    }

    #[test]
    fn the_star_tool_creates_a_five_point_star_polygon() {
        let state = state_with(crate::Tool::Star);
        let star = polygon(&state);
        assert_eq!(star.polygon_count, STAR_DEFAULT_POINTS);
        assert_eq!(star.inner_radius, Some(STAR_DEFAULT_INNER_RADIUS));
        assert_eq!(star.base.name.as_deref(), Some("Star"));
        assert!(
            star.fill.is_some(),
            "a star gets the shape tools' default fill"
        );
        // The plain polygon tool still makes a regular polygon.
        assert_eq!(
            polygon(&state_with(crate::Tool::Polygon)).inner_radius,
            None
        );
    }

    #[test]
    fn the_panel_edits_star_points_and_inner_radius() {
        use crate::PropertyFocus;
        let mut state = state_with(crate::Tool::Star);
        assert!(state.commit_property_edit(PropertyFocus::PolygonSides, 8.0));
        assert!(state.commit_property_edit(PropertyFocus::StarInnerRadius, 55.0));
        let star = polygon(&state);
        assert_eq!(star.polygon_count, 8);
        assert_eq!(star.inner_radius, Some(0.55));
        // Out-of-range percentages clamp; non-finite ones are rejected.
        assert!(state.commit_property_edit(PropertyFocus::StarInnerRadius, 0.0));
        assert_eq!(polygon(&state).inner_radius, Some(STAR_MIN_INNER_RADIUS));
        assert!(!state.commit_property_edit(PropertyFocus::StarInnerRadius, f32::NAN));
        assert_eq!(polygon(&state).inner_radius, Some(STAR_MIN_INNER_RADIUS));
    }

    #[test]
    fn mcp_data_patches_turn_a_polygon_into_a_star_and_back() {
        use crate::command::EditorCommand;
        let mut state = state_with(crate::Tool::Polygon);
        let node_id = state.selection.anchor.clone();
        assert!(state.apply(EditorCommand::PatchNodeData {
            node_id: node_id.clone(),
            patch_json: r#"{"polygonCount":6,"innerRadius":0.3}"#.into(),
            page_id: None,
        }));
        assert_eq!(polygon(&state).polygon_count, 6);
        assert_eq!(polygon(&state).inner_radius, Some(0.3));
        assert!(state.apply(EditorCommand::PatchNodeData {
            node_id,
            patch_json: r#"{"innerRadius":null}"#.into(),
            page_id: None,
        }));
        assert_eq!(polygon(&state).inner_radius, None);
    }

    #[test]
    fn insert_node_builds_a_star_kind() {
        let node = crate::command_node::build_leaf_node("star", "s", "Star", 0, 0, 40, 40)
            .expect("star kind builds");
        let jian_ops_schema::node::PenNode::Polygon(star) = node else {
            panic!("a star is a polygon");
        };
        assert_eq!(star.inner_radius, Some(STAR_DEFAULT_INNER_RADIUS));
        assert!(crate::command_node::kind_is_valid("star"));
    }

    #[test]
    fn the_inner_radius_edit_rejects_non_polygons() {
        let mut state = state_with(crate::Tool::Rect);
        assert!(!state.commit_property_edit(crate::PropertyFocus::StarInnerRadius, 40.0));
    }
}
