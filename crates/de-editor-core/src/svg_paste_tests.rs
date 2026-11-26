use super::*;
use crate::PenNodeExt;
use jian_ops_schema::node::PenNode;

const SVG: &str = r##"<?xml version="1.0"?>
<svg xmlns="http://www.w3.org/2000/svg" width="40" height="20" viewBox="0 0 40 20">
  <rect x="0" y="0" width="40" height="20" fill="#3366ff"/>
  <path d="M0 0 L40 20" stroke="#000000"/>
</svg>"##;

#[test]
fn pasted_svg_text_becomes_vectors_centred_on_the_point() {
    let mut state = EditorState::starter();
    let before = state.active_children().len();
    let mut next_id = 0;
    let count = state.paste_svg_centered(&mut next_id, SVG, (500.0, 300.0));
    assert_eq!(count, 2, "a rect and a path");
    assert_eq!(
        state.active_children().len(),
        before + 1,
        "one wrapping group"
    );
    let group = state.selected_node().expect("the pasted group is selected");
    assert!(matches!(group, PenNode::Group(_)));
    let has_path = group
        .children()
        .is_some_and(|c| c.iter().any(|n| matches!(n, PenNode::Path(_))));
    assert!(has_path, "the path stays a vector path");
    // The group's own offset moves its children (`aggregate_bounds`
    // reads the children's coordinates only).
    let b = aggregate_bounds(group);
    let (gx, gy) = (group.base().x.unwrap_or(0.0), group.base().y.unwrap_or(0.0));
    assert!(
        ((gx + b.x + b.w / 2.0) - 500.0).abs() < 0.5,
        "{b:?} at {gx}"
    );
    assert!(
        ((gy + b.y + b.h / 2.0) - 300.0).abs() < 0.5,
        "{b:?} at {gy}"
    );
}

#[test]
fn markup_without_shapes_pastes_nothing() {
    let mut state = EditorState::starter();
    let before = state.active_children().len();
    let mut next_id = 0;
    assert_eq!(
        state.paste_svg_centered(&mut next_id, "<svg/>", (0.0, 0.0)),
        0
    );
    assert_eq!(state.active_children().len(), before);
}
