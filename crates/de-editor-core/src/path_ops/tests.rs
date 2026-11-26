//! Boolean groups, Simplify and Reverse.

use super::*;
use crate::pen_node_ext::PenNodeExt;
use crate::walkers::find_node;
use jian_ops_schema::node::PenNode;

fn state(json: &str) -> EditorState {
    let doc = jian_ops_schema::load_str(json)
        .expect("fixture parses")
        .value;
    EditorState::from_document(doc)
}

const SHAPES: &str = r##"{"version":"1.0.0","children":[
  {"type":"rectangle","id":"a","x":0,"y":0,"width":20,"height":20,
   "fill":[{"type":"solid","color":"#ff0000"}]},
  {"type":"text","id":"t","content":"hi"},
  {"type":"ellipse","id":"b","x":10,"y":0,"width":20,"height":20},
  {"type":"frame","id":"f","x":100,"y":0,"width":50,"height":50,"children":[
    {"type":"rectangle","id":"inner","width":5,"height":5}]}]}"##;

fn ids(v: &[&str]) -> Vec<NodeId> {
    v.iter().map(|s| NodeId::new(*s)).collect()
}

#[test]
fn boolean_group_wraps_siblings_in_paint_order_and_keeps_them_editable() {
    let mut s = state(SHAPES);
    let mut next = 100u64;
    let depth = s.history.past.len();
    // Selection order does not matter: the bottom node is the base.
    assert!(s.create_canvas_boolean_group(ids(&["b", "a"]), BooleanOp::Subtract, &mut next));
    assert_eq!(s.history.past.len(), depth + 1);
    let kids = s.active_children();
    assert_eq!(kids.len(), 3, "a + b became one group at a's slot");
    let PenNode::Boolean(group) = &kids[0] else {
        panic!("{:?}", kids[0])
    };
    assert_eq!(group.op, BooleanOp::Subtract);
    assert_eq!(group.group.base.name.as_deref(), Some("Subtract"));
    let children: Vec<&str> = kids[0]
        .children()
        .unwrap()
        .iter()
        .map(|c| c.id_str())
        .collect();
    assert_eq!(children, vec!["a", "b"]);
    assert!(
        group.group.container.fill.is_some(),
        "takes the base's fill"
    );
    assert_eq!(s.selection.set, vec![NodeId::new("n100")]);
    assert_eq!(
        s.boolean_op_of(&NodeId::new("n100")),
        Some(BooleanOp::Subtract)
    );
    assert!(s.is_boolean_child(&NodeId::new("a")));
    assert!(!s.is_boolean_child(&NodeId::new("t")));
    // Undo puts the sources back.
    assert!(s.undo());
    assert!(find_node(s.active_children(), &NodeId::new("n100")).is_none());
    assert_eq!(s.active_children().len(), 4);
}

#[test]
fn boolean_group_needs_two_siblings() {
    let mut s = state(SHAPES);
    let mut next = 100u64;
    let before = s.doc.clone();
    assert!(!s.create_canvas_boolean_group(ids(&["a"]), BooleanOp::Union, &mut next));
    assert!(!s.create_canvas_boolean_group(ids(&["a", "inner"]), BooleanOp::Union, &mut next));
    assert!(!s.create_canvas_boolean_group(ids(&["a", "nope"]), BooleanOp::Union, &mut next));
    assert_eq!(s.doc, before);
    assert_eq!(next, 100);
}

#[test]
fn set_boolean_op_switches_and_renames_a_default_name() {
    let mut s = state(SHAPES);
    let mut next = 100u64;
    assert!(s.create_canvas_boolean_group(ids(&["a", "b"]), BooleanOp::Union, &mut next));
    let id = NodeId::new("n100");
    assert!(s.apply_canvas_path_command(PathCommand::SetBooleanOp {
        node_id: id.clone(),
        op: BooleanOp::Exclude,
    }));
    assert_eq!(s.boolean_op_of(&id), Some(BooleanOp::Exclude));
    let name = find_node(s.active_children(), &id)
        .unwrap()
        .base()
        .name
        .clone();
    assert_eq!(name.as_deref(), Some("Exclude"));
    assert!(!s.apply_canvas_path_command(PathCommand::SetBooleanOp {
        node_id: id,
        op: BooleanOp::Exclude,
    }));
    assert!(!s.apply_canvas_path_command(PathCommand::SetBooleanOp {
        node_id: NodeId::new("a"),
        op: BooleanOp::Union,
    }));
}

#[test]
fn ungroup_releases_a_boolean_group() {
    let mut s = state(SHAPES);
    let mut next = 100u64;
    assert!(s.create_canvas_boolean_group(ids(&["a", "b"]), BooleanOp::Union, &mut next));
    assert!(s.ungroup_selected());
    assert!(s.active_children().iter().any(|c| c.id_str() == "a"));
}

#[test]
fn boolean_group_round_trips_through_the_schema() {
    let mut s = state(SHAPES);
    let mut next = 100u64;
    assert!(s.create_canvas_boolean_group(ids(&["a", "b"]), BooleanOp::Intersect, &mut next));
    let json = serde_json::to_value(&s.active_children()[0]).unwrap();
    assert_eq!(json["type"], "boolean");
    assert_eq!(json["op"], "intersect");
    assert_eq!(json["children"].as_array().map(Vec::len), Some(2));
}

const PATH: &str = r##"{"version":"1.0.0","children":[
  {"type":"path","id":"p","x":0,"y":0,"width":30,"height":10,
   "anchors":[{"x":0,"y":0},{"x":10,"y":0},{"x":20,"y":0.01},{"x":30,"y":10}],
   "closed":false}]}"##;

fn anchors(s: &EditorState) -> Vec<(f64, f64)> {
    let PenNode::Path(p) = &s.active_children()[0] else {
        panic!()
    };
    p.anchors
        .as_ref()
        .unwrap()
        .iter()
        .map(|a| (a.x, a.y))
        .collect()
}

#[test]
fn simplify_drops_redundant_points_in_one_step() {
    let mut s = state(PATH);
    let depth = s.history.past.len();
    assert!(s.apply_canvas_path_command(PathCommand::Simplify {
        node_id: NodeId::new("p"),
        tolerance: DEFAULT_SIMPLIFY_TOLERANCE,
    }));
    assert_eq!(anchors(&s).len(), 3);
    assert_eq!(s.history.past.len(), depth + 1);
    // Nothing left to remove: no write, no undo step.
    assert!(!s.apply_canvas_path_command(PathCommand::Simplify {
        node_id: NodeId::new("p"),
        tolerance: DEFAULT_SIMPLIFY_TOLERANCE,
    }));
}

#[test]
fn reverse_flips_the_anchor_order() {
    let mut s = state(PATH);
    assert!(s.apply_canvas_path_command(PathCommand::Reverse {
        node_id: NodeId::new("p"),
    }));
    let pts = anchors(&s);
    assert_eq!(pts.first(), Some(&(30.0, 10.0)));
    assert_eq!(pts.last(), Some(&(0.0, 0.0)));
}

#[test]
fn simplify_and_reverse_refuse_non_paths() {
    let mut s = state(SHAPES);
    assert!(!s.apply_canvas_path_command(PathCommand::Reverse {
        node_id: NodeId::new("a"),
    }));
    assert!(!s.apply_canvas_path_command(PathCommand::Simplify {
        node_id: NodeId::new("a"),
        tolerance: 1.0,
    }));
}

#[test]
fn selection_in_paint_order_sorts_bottom_first() {
    let mut s = state(SHAPES);
    s.selection.set = ids(&["b", "inner", "a"]);
    assert_eq!(s.selection_in_paint_order(), ids(&["a", "b", "inner"]));
}
