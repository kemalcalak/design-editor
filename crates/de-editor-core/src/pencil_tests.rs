use crate::pen_node_ext::PenNodeExt;
use crate::render_backend::Point2D;
use crate::test_support::state_with;
use crate::walkers::find_node;
use crate::PencilSmoothing;
use jian_ops_schema::node::PenNode;
use jian_ops_schema::style::{StrokeCap, StrokeThickness};

fn draw_arc(s: &mut crate::EditorState, samples: usize) {
    s.begin_pencil_stroke(Point2D::new(0.0, 0.0));
    for i in 1..samples {
        let a = std::f32::consts::PI * i as f32 / (samples - 1) as f32;
        s.extend_pencil_stroke(Point2D::new(50.0 - 50.0 * a.cos(), 50.0 * a.sin()));
    }
}

#[test]
fn a_stroke_becomes_one_selected_path_and_one_undo_step() {
    let mut s = state_with(vec![]);
    let mut next_id = 1u64;
    let before = s.active_children().len();
    draw_arc(&mut s, 80);
    assert!(s.pencil_stroke_active());
    assert!(
        s.active_children().len() == before,
        "drag leaves the doc alone"
    );
    let id = s.finish_pencil_stroke(&mut next_id).expect("path inserted");
    assert!(!s.pencil_stroke_active());
    assert_eq!(s.selection.anchor, id);
    let Some(PenNode::Path(path)) = find_node(s.active_children(), &id) else {
        panic!("expected a path node");
    };
    let anchors = path.anchors.as_ref().expect("anchors for point editing");
    assert!(anchors.len() >= 2 && anchors.len() < 80, "fitted, not raw");
    assert_eq!(path.closed, Some(false));
    assert!(
        path.d.as_deref().is_some_and(|d| d.contains('C')),
        "curves baked into d"
    );
    let stroke = path.stroke.as_ref().expect("stroke");
    assert_eq!(stroke.thickness, StrokeThickness::Uniform(2.0));
    assert_eq!(stroke.cap, Some(StrokeCap::Round));
    assert_eq!(
        s.active_children()[0].id_str(),
        id.as_str(),
        "front of page"
    );

    assert!(s.undo());
    assert!(
        find_node(s.active_children(), &id).is_none(),
        "one undo removes it"
    );
    assert_eq!(s.active_children().len(), before);
}

#[test]
fn a_click_without_drag_inserts_nothing_and_no_history() {
    let mut s = state_with(vec![]);
    let mut next_id = 1u64;
    let depth = s.history.past.len();
    s.begin_pencil_stroke(Point2D::new(10.0, 10.0));
    s.extend_pencil_stroke(Point2D::new(10.2, 10.1)); // below sample spacing
    assert!(s.finish_pencil_stroke(&mut next_id).is_none());
    assert_eq!(s.history.past.len(), depth);
    assert!(s.active_children().is_empty());
    assert!(!s.pencil_stroke_active());
}

#[test]
fn cancel_drops_the_samples_without_touching_the_document() {
    let mut s = state_with(vec![]);
    draw_arc(&mut s, 20);
    assert!(s.cancel_pencil_stroke());
    assert!(!s.cancel_pencil_stroke());
    assert!(s.active_children().is_empty());
}

#[test]
fn sample_spacing_follows_the_zoom() {
    let mut s = state_with(vec![]);
    s.viewport.zoom = 4.0;
    s.begin_pencil_stroke(Point2D::new(0.0, 0.0));
    // 0.3 doc px = 1.2 screen px at 4x: recorded.
    assert!(s.extend_pencil_stroke(Point2D::new(0.3, 0.0)));
    s.viewport.zoom = 0.5;
    // 1.5 doc px = 0.75 screen px at 0.5x: dropped.
    assert!(!s.extend_pencil_stroke(Point2D::new(1.8, 0.0)));
}

#[test]
fn smoother_setting_keeps_fewer_anchors() {
    let count = |level: PencilSmoothing| {
        let mut s = state_with(vec![]);
        s.editor_ui.pencil_smoothing = level;
        s.begin_pencil_stroke(Point2D::new(0.0, 0.0));
        for i in 1..200 {
            let t = i as f32 * 0.05;
            // A wobbly line: the smooth setting should iron the wobble out.
            s.extend_pencil_stroke(Point2D::new(t * 20.0, (t * 7.0).sin() * 4.0));
        }
        let mut next_id = 1u64;
        let id = s.finish_pencil_stroke(&mut next_id).expect("path");
        match find_node(s.active_children(), &id) {
            Some(PenNode::Path(p)) => p.anchors.as_ref().map_or(0, Vec::len),
            _ => 0,
        }
    };
    assert!(count(PencilSmoothing::Smooth) <= count(PencilSmoothing::Precise));
}

#[test]
fn smoothing_ids_round_trip() {
    for level in PencilSmoothing::ALL {
        assert_eq!(PencilSmoothing::from_id(level.id()), Some(level));
    }
    assert_eq!(PencilSmoothing::default(), PencilSmoothing::Balanced);
}

#[test]
fn undo_mid_stroke_drops_the_stroke_and_keeps_redo() {
    let mut s = state_with(vec![]);
    let mut next_id = 1u64;
    draw_arc(&mut s, 40);
    let first = s.finish_pencil_stroke(&mut next_id).expect("first stroke");
    draw_arc(&mut s, 40);
    assert!(s.undo(), "undo the first stroke while drawing the second");
    assert!(!s.pencil_stroke_active(), "the in-flight stroke is dropped");
    assert!(s.finish_pencil_stroke(&mut next_id).is_none());
    assert!(s.redo(), "redo still restores the first stroke");
    assert!(find_node(s.active_children(), &first).is_some());
}
