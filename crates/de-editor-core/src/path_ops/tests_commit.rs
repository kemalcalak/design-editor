//! `PathCommand::Commit` through the canvas entry point.

use super::*;
use crate::vector_edit::{corner, Subpath};
use crate::Tool;
use jian_ops_schema::node::{PathFillRule, PenNode};
use jian_ops_schema::sizing::SizingBehavior;

fn state(json: &str) -> EditorState {
    let doc = jian_ops_schema::load_str(json)
        .expect("fixture parses")
        .value;
    EditorState::from_document(doc)
}

fn triangle() -> Vec<Subpath> {
    vec![Subpath {
        anchors: vec![
            corner((10.0, 10.0)),
            corner((0.0, 0.0)),
            corner((10.0, 0.0)),
        ],
        closed: true,
    }]
}

fn commit(source_ids: Vec<NodeId>, subpaths: Vec<Subpath>, kind: PathCommitKind) -> CanvasPath {
    CanvasPath {
        source_ids,
        subpaths,
        even_odd: true,
        origin: (0.0, 0.0),
        kind,
    }
}

#[test]
fn commit_swaps_sources_for_one_path() {
    let mut s = EditorState::new();
    let mut next = 100u64;
    let a = s
        .create_node_for_tool(Tool::Pen, &mut next, 0.0, 0.0, 1.0, 1.0)
        .unwrap();
    let b = s
        .create_node_for_tool(Tool::Pen, &mut next, 5.0, 5.0, 1.0, 1.0)
        .unwrap();
    let depth = s.history.past.len();
    assert!(s.commit_canvas_path(
        commit(vec![a, b], triangle(), PathCommitKind::Boolean),
        &mut next
    ));
    assert_eq!(s.active_children().len(), 1);
    assert_eq!(s.history.past.len(), depth + 1);
    let PenNode::Path(path) = &s.active_children()[0] else {
        panic!("result should be a Path");
    };
    assert_eq!(path.base.id, "n102");
    assert_eq!(next, 103);
    assert_eq!(s.selection.set, vec![NodeId::new("n102")]);
    assert_eq!(path.base.name.as_deref(), Some("Boolean Result"));
    assert_eq!((path.base.x, path.base.y), (Some(0.0), Some(0.0)));
    assert_eq!(path.width, Some(SizingBehavior::Number(10.0)));
    assert_eq!(path.fill_rule, Some(PathFillRule::Evenodd));
    assert!(path.d.as_deref().is_some_and(|d| d.ends_with('Z')));
}

#[test]
fn commit_keeps_curves_and_maps_into_the_parent_frame() {
    let mut s = state(
        r##"{"version":"1.0.0","children":[
          {"type":"frame","id":"f","x":100,"y":50,"width":200,"height":200,"children":[
            {"type":"ellipse","id":"e","x":10,"y":10,"width":20,"height":20}]}]}"##,
    );
    let mut arc = corner((110.0, 70.0));
    arc.handle_out = Some(jian_ops_schema::node::PenPathHandle { x: 0.0, y: -11.0 });
    let mut end = corner((130.0, 60.0));
    end.handle_in = Some(jian_ops_schema::node::PenPathHandle { x: -11.0, y: 0.0 });
    let subs = vec![Subpath {
        anchors: vec![arc, end, corner((130.0, 70.0))],
        closed: true,
    }];
    let mut next = 100u64;
    let mut c = commit(vec![NodeId::new("e")], subs, PathCommitKind::Flatten);
    c.origin = (100.0, 50.0);
    assert!(s.commit_canvas_path(c, &mut next));
    let frame = &s.active_children()[0];
    let kids = crate::pen_node_ext::PenNodeExt::children(frame).unwrap();
    assert_eq!(kids.len(), 1, "the result takes the ellipse's slot");
    let PenNode::Path(path) = &kids[0] else {
        panic!("a path")
    };
    assert_eq!(path.base.x, Some(10.0));
    // The curve dips just above its end anchor: bounds include extrema.
    assert!(
        (path.base.y.unwrap() - 10.0).abs() < 0.1,
        "{:?}",
        path.base.y
    );
    assert!(
        path.d.as_deref().unwrap().contains('C'),
        "the curve survives"
    );
    assert_eq!(path.base.name.as_deref(), Some("Vector"));
}

#[test]
fn commit_rejects_empty_input_and_keeps_the_counter() {
    let mut s = EditorState::new();
    let mut next = 100u64;
    let a = s
        .create_node_for_tool(Tool::Pen, &mut next, 0.0, 0.0, 1.0, 1.0)
        .unwrap();
    let before = next;
    assert!(!s.commit_canvas_path(
        commit(vec![], triangle(), PathCommitKind::Boolean),
        &mut next
    ));
    let lone = vec![Subpath::polyline(&[(1.0, 1.0)])];
    assert!(!s.commit_canvas_path(commit(vec![a], lone, PathCommitKind::Boolean), &mut next));
    assert_eq!(next, before, "a rejected commit mints no id");
}

#[test]
fn commit_inherits_the_first_source_style() {
    let mut s = state(
        r##"{"version":"1.0.0","children":[
          {"type":"rectangle","id":"n10","x":0,"y":0,"width":20,"height":20,
           "fill":[{"type":"solid","color":"#ff0000"}]},
          {"type":"rectangle","id":"n11","x":10,"y":0,"width":20,"height":20}]}"##,
    );
    let mut next = 100u64;
    let ids = vec![NodeId::new("n10"), NodeId::new("n11")];
    assert!(s.commit_canvas_path(commit(ids, triangle(), PathCommitKind::Boolean), &mut next));
    let PenNode::Path(path) = &s.active_children()[0] else {
        panic!()
    };
    assert!(path.fill.is_some());
}

#[test]
fn commit_refuses_a_frame_with_children_but_bakes_a_boolean_group() {
    let json = r##"{"version":"1.0.0","children":[
      {"type":"frame","id":"n10","x":0,"y":0,"width":20,"height":20,
       "children":[{"type":"rectangle","id":"n12","width":5,"height":5}]},
      {"type":"boolean","id":"b","name":"Union","op":"union",
       "children":[{"type":"rectangle","id":"r1","width":5,"height":5},
                   {"type":"ellipse","id":"e1","x":3,"width":5,"height":5}]}]}"##;
    let mut s = state(json);
    let before = s.doc.clone();
    let mut next = 100u64;
    assert!(!s.commit_canvas_path(
        commit(
            vec![NodeId::new("n10")],
            triangle(),
            PathCommitKind::Flatten
        ),
        &mut next
    ));
    assert_eq!(s.doc, before);
    assert!(s.commit_canvas_path(
        commit(vec![NodeId::new("b")], triangle(), PathCommitKind::Flatten),
        &mut next
    ));
    let PenNode::Path(path) = &s.active_children()[1] else {
        panic!("the boolean group is baked into a path")
    };
    assert_eq!(
        path.base.name.as_deref(),
        Some("Union"),
        "keeps the layer name"
    );
    assert!(crate::walkers::find_node(s.active_children(), &NodeId::new("r1")).is_none());
}

#[test]
fn outline_stroke_paints_with_the_stroke_and_keeps_a_filled_source() {
    let json = r##"{"version":"1.0.0","children":[
      {"type":"rectangle","id":"r","width":20,"height":20,
       "fill":[{"type":"solid","color":"#00ff00"}],
       "stroke":{"thickness":2,"fill":[{"type":"solid","color":"#0000ff"}]}},
      {"type":"line","id":"l","x":0,"y":40,"width":20,"height":0,
       "stroke":{"thickness":2,"fill":[{"type":"solid","color":"#0000ff"}]}}]}"##;
    let mut s = state(json);
    let mut next = 100u64;
    assert!(s.commit_canvas_path(
        commit(
            vec![NodeId::new("r")],
            triangle(),
            PathCommitKind::OutlineStroke
        ),
        &mut next
    ));
    let kids = s.active_children();
    assert_eq!(
        kids.len(),
        3,
        "the filled rect stays, the outline goes above it"
    );
    let PenNode::Rectangle(rect) = &kids[0] else {
        panic!()
    };
    assert!(rect.container.stroke.is_none() && rect.container.fill.is_some());
    let PenNode::Path(outline) = &kids[1] else {
        panic!()
    };
    assert!(outline.stroke.is_none());
    let color = |p: &jian_ops_schema::node::PathNode| match p.fill.as_deref() {
        Some([jian_ops_schema::style::PenFill::Solid(s)]) => s.color.clone(),
        other => panic!("{other:?}"),
    };
    assert_eq!(color(outline), "#0000ff");
    // A line has no fill: its outline replaces it.
    assert!(s.commit_canvas_path(
        commit(
            vec![NodeId::new("l")],
            triangle(),
            PathCommitKind::OutlineStroke
        ),
        &mut next
    ));
    assert_eq!(s.active_children().len(), 3);
    assert!(crate::walkers::find_node(s.active_children(), &NodeId::new("l")).is_none());
}

#[test]
fn a_stale_commit_is_rejected() {
    let mut s = state(
        r##"{"version":"1.0.0","children":[{"type":"rectangle","id":"r","width":5,"height":5}]}"##,
    );
    let cmd = PathCommand::Commit(Box::new(PathCommit {
        source_ids: vec![NodeId::new("r")],
        subpaths: triangle(),
        even_odd: false,
        origin: (0.0, 0.0),
        new_id: NodeId::new("p1"),
        revision: s.document_revision() + 7,
        kind: PathCommitKind::Flatten,
    }));
    assert!(!s.apply(crate::EditorCommand::Path(cmd)));
}
