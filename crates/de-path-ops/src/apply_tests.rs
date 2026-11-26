//! End to end on an `EditorState`: the selection, the scene the canvas
//! would build for it, and the document write. The scene is built by hand
//! here (the loader depends on this crate); its boxes match the document.

use super::*;
use crate::test_raster::coverage;
use crate::{cubic_segment_count, subpaths_to_path};
use de_editor_core::path_ops::{OffsetJoin, PathOperation};
use de_editor_core::render_backend::Rect;
use de_editor_core::vector_edit::node_subpaths;
use de_editor_core::{BooleanOp, PenNodeExt};
use jian_ops_schema::node::PenNode;
use jian_scene::layout_scene::{
    NodeKind, SceneNode, ScenePage, SceneStroke, SceneStrokeAlign, StrokeStyle,
};

const DOC: &str = r##"{"version":"1.0.0","children":[
  {"type":"ellipse","id":"c1","x":0,"y":0,"width":100,"height":100,
   "fill":[{"type":"solid","color":"#ff0000"}]},
  {"type":"ellipse","id":"c2","x":60,"y":0,"width":100,"height":100},
  {"type":"rectangle","id":"r","x":200,"y":0,"width":40,"height":40,
   "stroke":{"thickness":4,"fill":[{"type":"solid","color":"#0000ff"}]}}]}"##;

fn state() -> EditorState {
    EditorState::from_document(jian_ops_schema::load_str(DOC).unwrap().value)
}

fn shape(id: &str, kind: NodeKind, x: f32, w: f32) -> SceneNode {
    let mut n = SceneNode::leaf(id, kind);
    n.bounds = Rect::xywh(x, 0.0, w, if w > 50.0 { 100.0 } else { 40.0 });
    n
}

fn scene() -> LayoutScene {
    let mut r = shape("r", NodeKind::Rect, 200.0, 40.0);
    r.stroke = Some(SceneStroke {
        color: de_editor_core::Color {
            r: 0.0,
            g: 0.0,
            b: 1.0,
            a: 1.0,
        },
        width: 4.0,
        sides: None,
        align: SceneStrokeAlign::Center,
        style: StrokeStyle::default(),
    });
    LayoutScene {
        pages: vec![ScenePage {
            id: "p".into(),
            name: "P".into(),
            children: vec![
                shape("c1", NodeKind::Ellipse, 0.0, 100.0),
                shape("c2", NodeKind::Ellipse, 60.0, 100.0),
                r,
            ],
        }],
        active_page_index: 0,
    }
}

fn select(s: &mut EditorState, ids: &[&str]) {
    s.selection.set = ids.iter().map(|i| NodeId::new(*i)).collect();
    s.selection.anchor = s.selection.set[0].clone();
}

fn run(s: &mut EditorState, op: PathOperation) -> bool {
    let mut next = 100;
    apply_path_operation(s, &scene(), op, &mut next, 0)
}

#[test]
fn boolean_creates_a_group_then_switches_its_op() {
    let mut s = state();
    select(&mut s, &["c2", "c1"]);
    assert!(run(&mut s, PathOperation::Boolean(BooleanOp::Union)));
    let group = NodeId::new("n100");
    assert_eq!(s.boolean_op_of(&group), Some(BooleanOp::Union));
    assert!(run(&mut s, PathOperation::Boolean(BooleanOp::Exclude)));
    assert_eq!(s.boolean_op_of(&group), Some(BooleanOp::Exclude));
}

#[test]
fn flatten_of_two_circles_bakes_at_most_eight_cubics() {
    let mut s = state();
    select(&mut s, &["c1", "c2"]);
    assert!(run(&mut s, PathOperation::Flatten));
    let node = &s.active_children()[0];
    assert!(matches!(node, PenNode::Path(_)));
    let subs = node_subpaths(node).unwrap();
    let cubics = cubic_segment_count(&subs);
    assert!((1..=8).contains(&cubics), "{cubics}");
    // Pixel check: the baked path covers what the two circles cover.
    let px = coverage(&subpaths_to_path(&subs, true), 170, 110) as f64;
    let lens = 5000.0 * 0.6f64.acos() - 30.0 * 80.0;
    let expected = 2.0 * std::f64::consts::PI * 2500.0 - lens;
    assert!(
        (px - expected).abs() / expected < 0.03,
        "{px} vs {expected}"
    );
    assert!(s.active_children().iter().all(|c| c.id_str() != "c2"));
}

#[test]
fn outline_stroke_and_offset_replace_the_shape() {
    let mut s = state();
    select(&mut s, &["r"]);
    assert!(run(&mut s, PathOperation::OutlineStroke));
    let last = s.active_children().last().unwrap();
    let PenNode::Path(p) = last else {
        panic!("{last:?}")
    };
    assert!(p.stroke.is_none() && p.fill.is_some());
    let mut s = state();
    select(&mut s, &["r"]);
    let grow = PathOperation::Offset {
        delta: 5.0,
        join: OffsetJoin::Miter,
    };
    assert!(run(&mut s, grow));
    let PenNode::Path(p) = s.active_children().last().unwrap() else {
        panic!()
    };
    assert_eq!(p.base.x, Some(195.0));
}

#[test]
fn simplify_and_reverse_skip_non_paths() {
    let mut s = state();
    select(&mut s, &["r"]);
    assert!(!run(&mut s, PathOperation::Simplify));
    assert!(!run(&mut s, PathOperation::Reverse));
}

#[test]
fn nothing_selected_does_nothing() {
    let mut s = state();
    s.selection.set.clear();
    assert!(!run(&mut s, PathOperation::Flatten));
}
