//! Figma `STAR` → parametric star polygon (#87).
//!
//! A Figma star carries its point count (`count`) and inner radius
//! (`starInnerScale`, a fraction of the outer radius). It used to be
//! flattened into a path, losing both; it now becomes a `PolygonNode`
//! with `polygon_count` + `inner_radius`, which the editor paints with
//! the same box-fitted geometry Figma uses.
//!
//! Stars the parametric node cannot reproduce keep the vector path:
//! rounded corners (no rounded-star renderer yet) and masks (the vector
//! converter owns Figma's path-mask semantics).

use crate::common::{common_props, resolve_height, resolve_width, ConversionContext};
use crate::kiwi::FigValue;
use crate::mappers::{map_figma_effects, map_figma_fills, map_figma_stroke};
use crate::mask::figma_mask_type;
use crate::tree::TreeNode;
use jian_ops_schema::node::{PenNode, PolygonNode};

// de-figma does not depend on de-editor-core, so the stored range is
// restated here; `the_star_range_matches_the_editor` keeps them equal to
// `de_editor_core::star_shape`.
const STAR_DEFAULT_INNER_RADIUS: f64 = 0.382;
const STAR_MIN_INNER_RADIUS: f64 = 0.01;
const STAR_MAX_INNER_RADIUS: f64 = 1.0;

fn clamp_star_inner_radius(value: f64) -> Option<f64> {
    value
        .is_finite()
        .then(|| value.clamp(STAR_MIN_INNER_RADIUS, STAR_MAX_INNER_RADIUS))
}

/// True when `figma` is a star the parametric polygon reproduces.
pub(crate) fn is_parametric_star(figma: &FigValue) -> bool {
    figma.get_str("type") == Some("STAR")
        && figma
            .get_f64("count")
            .is_some_and(|c| c.is_finite() && c >= 3.0)
        && figma.get_f64("cornerRadius").unwrap_or(0.0) <= 0.0
        && figma_mask_type(figma).is_none()
}

/// Convert a parametric Figma star (see [`is_parametric_star`]).
pub(crate) fn convert_star(
    tree: &TreeNode,
    parent_stack_mode: Option<&str>,
    ctx: &mut ConversionContext<'_>,
) -> PenNode {
    let id = ctx.generate_id();
    let figma = &tree.figma;
    let points = figma
        .get_f64("count")
        .unwrap_or(5.0)
        .round()
        .clamp(3.0, 100.0) as u32;
    let inner_radius = figma
        .get_f64("starInnerScale")
        .and_then(clamp_star_inner_radius)
        .unwrap_or(STAR_DEFAULT_INNER_RADIUS);
    PenNode::Polygon(PolygonNode {
        base: common_props(figma, id),
        polygon_count: points,
        width: Some(resolve_width(figma, parent_stack_mode, ctx)),
        height: Some(resolve_height(figma, parent_stack_mode, ctx)),
        corner_radius: None,
        inner_radius: Some(inner_radius),
        fill: map_figma_fills(figma.get_array("fillPaints")),
        stroke: map_figma_stroke(figma),
        effects: map_figma_effects(figma.get_array("effects")),
        state: None,
        bindings: None,
        events: None,
        lifecycle: None,
        semantics: None,
        gestures: None,
        route: None,
        // Figma import never authors responsive size constraints.
        limits: Default::default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::FigLayoutMode;
    use std::collections::HashMap;

    fn obj(pairs: Vec<(&str, FigValue)>) -> FigValue {
        FigValue::Object(pairs.into_iter().map(|(k, v)| (k.into(), v)).collect())
    }

    fn star(extra: Vec<(&str, FigValue)>) -> TreeNode {
        let mut pairs = vec![
            ("type", FigValue::Str("STAR".into())),
            ("name", FigValue::Str("Star 1".into())),
            (
                "size",
                obj(vec![
                    ("x", FigValue::Float(80.0)),
                    ("y", FigValue::Float(60.0)),
                ]),
            ),
            ("count", FigValue::Uint(7)),
            ("starInnerScale", FigValue::Float(0.5)),
        ];
        pairs.extend(extra);
        TreeNode {
            figma: obj(pairs),
            children: vec![],
        }
    }

    fn without(mut tree: TreeNode, key: &str) -> TreeNode {
        if let FigValue::Object(fields) = &mut tree.figma {
            fields.retain(|(k, _)| &**k != key);
        }
        tree
    }

    fn ctx<'tree>() -> ConversionContext<'tree> {
        ConversionContext {
            component_map: HashMap::new(),
            symbol_tree: HashMap::new(),
            warnings: Vec::new(),
            id_counter: 1,
            blobs: Vec::new(),
            layout_mode: FigLayoutMode::Preserve,
            instance_assignments: HashMap::new(),
            instance_expansions: Default::default(),
        }
    }

    #[test]
    fn a_figma_star_keeps_its_point_count_and_inner_radius() {
        let tree = star(vec![]);
        let mut ctx = ctx();
        let node = crate::converters::convert_node(&tree, None, &mut ctx).expect("star converts");
        let PenNode::Polygon(polygon) = node else {
            panic!("expected a star polygon, got {node:?}");
        };
        assert_eq!(polygon.polygon_count, 7);
        assert_eq!(polygon.inner_radius, Some(0.5));
        assert_eq!(polygon.base.name.as_deref(), Some("Star 1"));
    }

    #[test]
    fn star_fields_are_clamped_and_defaulted() {
        let tree = without(without(star(vec![]), "starInnerScale"), "count");
        let tree = TreeNode {
            figma: match tree.figma {
                FigValue::Object(mut fields) => {
                    fields.push(("count".into(), FigValue::Uint(400)));
                    FigValue::Object(fields)
                }
                other => other,
            },
            children: vec![],
        };
        let mut ctx = ctx();
        let PenNode::Polygon(polygon) = convert_star(&tree, None, &mut ctx) else {
            panic!("expected a polygon");
        };
        assert_eq!(polygon.polygon_count, 100);
        assert_eq!(polygon.inner_radius, Some(STAR_DEFAULT_INNER_RADIUS));
    }

    #[test]
    fn rounded_masking_or_countless_stars_stay_vector() {
        assert!(is_parametric_star(&star(vec![]).figma));
        let rounded = star(vec![("cornerRadius", FigValue::Float(4.0))]);
        assert!(!is_parametric_star(&rounded.figma));
        let masking = star(vec![("mask", FigValue::Bool(true))]);
        assert!(!is_parametric_star(&masking.figma));
        assert!(!is_parametric_star(&without(star(vec![]), "count").figma));
    }

    #[test]
    fn the_star_range_matches_the_editor() {
        use de_editor_core::star_shape as core;
        assert_eq!(STAR_DEFAULT_INNER_RADIUS, core::STAR_DEFAULT_INNER_RADIUS);
        assert_eq!(STAR_MIN_INNER_RADIUS, core::STAR_MIN_INNER_RADIUS);
        assert_eq!(STAR_MAX_INNER_RADIUS, core::STAR_MAX_INNER_RADIUS);
    }
}
