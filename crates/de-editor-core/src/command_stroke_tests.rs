#![cfg(test)]

use crate::command::EditorCommand;
use crate::node_id::NodeId;
use crate::test_support::{rect, state_with};
use crate::walkers::{find_node, find_node_mut};
use jian_ops_schema::node::PenNode;
use jian_ops_schema::style::{PenStroke, SidedThickness, StrokeThickness};

fn id(s: &str) -> NodeId {
    NodeId::new(s)
}

#[test]
fn set_node_stroke_width_preserves_sided_edges() {
    let mut s = state_with(vec![rect("n1", "r", 0.0, 0.0, 10.0, 10.0)]);
    let node = find_node_mut(s.active_children_mut(), &id("n1")).unwrap();
    let PenNode::Rectangle(r) = node else {
        panic!("rectangle");
    };
    r.container.stroke = Some(PenStroke {
        thickness: StrokeThickness::Sided(SidedThickness {
            bottom: Some(1.0),
            ..Default::default()
        }),
        align: None,
        join: None,
        cap: None,
        dash_pattern: None,
        dash_offset: None,
        fill: None,
    });

    assert!(s.apply(EditorCommand::SetNodeStrokeWidth {
        node_id: id("n1"),
        width: 6.0,
    }));

    let PenNode::Rectangle(r) = find_node(s.active_children(), &id("n1")).unwrap() else {
        panic!("rectangle");
    };
    let stroke = r.container.stroke.as_ref().expect("stroke");
    assert!(matches!(
        &stroke.thickness,
        StrokeThickness::Sided(sides)
            if sides.top.is_none()
                && sides.right.is_none()
                && sides.bottom == Some(6.0)
                && sides.left.is_none()
    ));
}

#[test]
fn commit_property_edit_writes_one_stroke_edge() {
    let mut s = state_with(vec![rect("n1", "r", 0.0, 0.0, 10.0, 10.0)]);
    let node = find_node_mut(s.active_children_mut(), &id("n1")).unwrap();
    let PenNode::Rectangle(r) = node else {
        panic!("rectangle");
    };
    r.container.stroke = Some(PenStroke {
        thickness: StrokeThickness::Uniform(1.0),
        align: None,
        join: None,
        cap: None,
        dash_pattern: None,
        dash_offset: None,
        fill: None,
    });

    s.set_single_selection(id("n1"));
    s.editor_ui.stroke_edit_mode = Some(crate::PaddingEditMode::Individual);
    assert!(s.commit_property_edit(crate::PropertyFocus::StrokeRightWidth, 3.0));

    let PenNode::Rectangle(r) = find_node(s.active_children(), &id("n1")).unwrap() else {
        panic!("rectangle");
    };
    let stroke = r.container.stroke.as_ref().expect("stroke");
    assert!(matches!(
        &stroke.thickness,
        StrokeThickness::Sided(sides)
            if sides.top == Some(1.0)
                && sides.right == Some(3.0)
                && sides.bottom == Some(1.0)
                && sides.left == Some(1.0)
    ));
}

#[test]
fn commit_property_edit_single_stroke_mode_writes_all_edges() {
    let mut s = state_with(vec![rect("n1", "r", 0.0, 0.0, 10.0, 10.0)]);
    let node = find_node_mut(s.active_children_mut(), &id("n1")).unwrap();
    let PenNode::Rectangle(r) = node else {
        panic!("rectangle");
    };
    r.container.stroke = Some(PenStroke {
        thickness: StrokeThickness::Uniform(1.0),
        align: None,
        join: None,
        cap: None,
        dash_pattern: None,
        dash_offset: None,
        fill: None,
    });

    s.set_single_selection(id("n1"));
    assert!(s.commit_property_edit(crate::PropertyFocus::StrokeTopWidth, 3.0));

    let PenNode::Rectangle(r) = find_node(s.active_children(), &id("n1")).unwrap() else {
        panic!("rectangle");
    };
    let stroke = r.container.stroke.as_ref().expect("stroke");
    assert!(matches!(
        &stroke.thickness,
        StrokeThickness::Sided(sides)
            if sides.top == Some(3.0)
                && sides.right == Some(3.0)
                && sides.bottom == Some(3.0)
                && sides.left == Some(3.0)
    ));
}

#[test]
fn stroke_commands_outline_a_text_node() {
    // Outlined text (#282): the stroke writes used to report success and
    // change nothing on text.
    let mut s = state_with(vec![crate::test_support::text(
        "t", "Label", 0.0, 0.0, 120.0, 30.0, "Outlined",
    )]);
    assert!(s.apply(EditorCommand::SetNodeStrokeHex {
        node_id: id("t"),
        hex: "#FF00FF".into(),
    }));
    assert!(s.apply(EditorCommand::SetNodeStrokeWidth {
        node_id: id("t"),
        width: 2.0,
    }));
    let node = find_node(s.active_children(), &id("t")).unwrap();
    assert_eq!(crate::fills::first_solid_stroke_hex(node), Some("#FF00FF"));
    let PenNode::Text(text) = node else {
        panic!("text");
    };
    let stroke = text.stroke.as_ref().expect("the text has a stroke");
    assert!(matches!(stroke.thickness, StrokeThickness::Uniform(w) if w == 2.0));
}
