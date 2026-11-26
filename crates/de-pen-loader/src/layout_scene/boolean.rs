//! Boolean groups in the scene (#388).
//!
//! A `PenNode::Boolean` arrives as a group payload whose kind is
//! `boolean:<op>`. Once its children are scene nodes (document space),
//! the group becomes a `Path` whose `svg_path` is the boolean of their
//! outlines (`de_path_ops`, curves kept), painted with the group's own
//! fill and stroke. The children stay in the scene for selection and the
//! layer tree, hidden so they neither paint, hit-test nor export: what
//! the canvas shows, clicks and exports is the computed path. Without
//! skia (`skia-measure` off) the group paints its children as a plain
//! group.

use jian_scene::layout_scene::SceneNode;

/// Payload kind prefix of a boolean group (`boolean:union`, …).
pub(crate) const KIND_PREFIX: &str = "boolean:";

/// Turn a boolean group's scene node into its computed path.
pub(super) fn resolve_boolean_group(kind: &str, node: &mut SceneNode) {
    let Some(op) = kind
        .strip_prefix(KIND_PREFIX)
        .and_then(jian_ops_schema::node::BooleanOperation::parse)
    else {
        return;
    };
    #[cfg(feature = "skia-measure")]
    resolve_with_skia(op, node);
    #[cfg(not(feature = "skia-measure"))]
    let _ = (op, node);
}

#[cfg(feature = "skia-measure")]
fn resolve_with_skia(op: jian_ops_schema::node::BooleanOperation, node: &mut SceneNode) {
    use de_editor_core::render_backend::Rect;
    use jian_scene::layout_scene::NodeKind;
    let Some(path) = de_path_ops::boolean_of_children(op, &node.children) else {
        return;
    };
    let Some((d, [x, y, w, h])) = de_path_ops::local_path_data(&path) else {
        return;
    };
    node.kind = NodeKind::Path;
    node.svg_path = Some(d);
    node.bounds = Rect::xywh(x, y, w, h);
    node.path_closed = true;
    node.even_odd_fill = matches!(
        path.fill_type(),
        skia_safe::PathFillType::EvenOdd | skia_safe::PathFillType::InverseEvenOdd
    );
    for child in &mut node.children {
        child.hidden = true;
    }
    node.aggregate_bounds_cache = SceneNode::compute_aggregate_bounds(node.bounds, &node.children);
}
