use super::*;
use crate::test_raster::coverage;
use de_editor_core::render_backend::Rect;
use jian_scene::layout_scene::ScenePage;

fn shape(id: &str, kind: NodeKind, x: f32, y: f32, w: f32, h: f32) -> SceneNode {
    let mut n = SceneNode::leaf(id, kind);
    n.bounds = Rect::xywh(x, y, w, h);
    n
}

fn scene(children: Vec<SceneNode>) -> LayoutScene {
    LayoutScene {
        pages: vec![ScenePage {
            id: "p".into(),
            name: "P".into(),
            children,
        }],
        active_page_index: 0,
    }
}

fn ids(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

#[test]
fn union_of_two_circles_keeps_curves() {
    let s = scene(vec![
        shape("a", NodeKind::Ellipse, 0.0, 0.0, 100.0, 100.0),
        shape("b", NodeKind::Ellipse, 60.0, 0.0, 100.0, 100.0),
    ]);
    let path = compute_boolean(&s, &ids(&["a", "b"]), BooleanOperation::Union).unwrap();
    let result = PathResult::from_path(&path).unwrap();
    let segments = cubic_segment_count(&result.subpaths);
    assert!((1..=8).contains(&segments), "{segments} cubic segments");
    let anchors: usize = result.subpaths.iter().map(|s| s.anchors.len()).sum();
    assert!(anchors <= 10, "{anchors} anchors, not a polyline");
    // Matches skia's own result pixel for pixel (within AA).
    let ours = coverage(
        &subpaths_to_path(&result.subpaths, result.even_odd),
        170,
        110,
    ) as i64;
    let skia = coverage(&path, 170, 110) as i64;
    assert!((ours - skia).abs() <= 6, "{ours} vs {skia}");
}

#[test]
fn every_op_matches_skia() {
    let s = scene(vec![
        shape("a", NodeKind::Rect, 0.0, 0.0, 40.0, 40.0),
        shape("b", NodeKind::Ellipse, 20.0, 20.0, 40.0, 40.0),
    ]);
    let expected = [
        (BooleanOperation::Union, 1600 + 1257 - 314),
        (BooleanOperation::Subtract, 1600 - 314),
        (BooleanOperation::Intersect, 314),
        (BooleanOperation::Exclude, 1600 + 1257 - 2 * 314),
    ];
    for (op, px) in expected {
        let path = compute_boolean(&s, &ids(&["a", "b"]), op).unwrap();
        let got = coverage(&path, 64, 64) as i64;
        assert!((got - px).abs() <= 12, "{op:?}: {got} vs {px}");
    }
}

#[test]
fn boolean_needs_two_operands_with_area() {
    let s = scene(vec![
        shape("a", NodeKind::Rect, 0.0, 0.0, 10.0, 10.0),
        shape("t", NodeKind::Text, 0.0, 0.0, 10.0, 10.0),
    ]);
    assert!(compute_boolean(&s, &ids(&["a"]), BooleanOperation::Union).is_none());
    assert!(compute_boolean(&s, &ids(&["a", "t"]), BooleanOperation::Union).is_none());
}

#[test]
fn boolean_of_children_skips_hidden_ones() {
    let mut hidden = shape("h", NodeKind::Rect, 100.0, 100.0, 10.0, 10.0);
    hidden.hidden = true;
    let kids = vec![
        shape("a", NodeKind::Rect, 0.0, 0.0, 10.0, 10.0),
        shape("b", NodeKind::Rect, 5.0, 0.0, 10.0, 10.0),
        hidden,
    ];
    let p = boolean_of_children(BooleanOperation::Union, &kids).unwrap();
    assert_eq!(coverage(&p, 20, 20), 150);
}

#[test]
fn local_path_data_is_relative_to_its_box() {
    let s = scene(vec![shape("a", NodeKind::Ellipse, 30.0, 40.0, 20.0, 10.0)]);
    let path = flatten_nodes(&s, &ids(&["a"])).unwrap();
    let (d, [x, y, w, h]) = local_path_data(&path).unwrap();
    assert_eq!((x, y, w, h), (30.0, 40.0, 20.0, 10.0));
    assert!(d.starts_with('M') && d.contains('C'), "{d}");
}

#[test]
fn flatten_unites_several_nodes() {
    let s = scene(vec![
        shape("a", NodeKind::Rect, 0.0, 0.0, 10.0, 10.0),
        shape("b", NodeKind::Rect, 5.0, 0.0, 10.0, 10.0),
    ]);
    let p = flatten_nodes(&s, &ids(&["a", "b"])).unwrap();
    assert_eq!(coverage(&p, 20, 20), 150);
}

#[test]
fn layout_origin_skips_groups() {
    let mut frame = shape("f", NodeKind::Frame, 100.0, 50.0, 200.0, 200.0);
    let mut group = shape("g", NodeKind::Group, 0.0, 0.0, 0.0, 0.0);
    group.children = vec![shape("r", NodeKind::Rect, 110.0, 60.0, 10.0, 10.0)];
    frame.children = vec![group, shape("s", NodeKind::Rect, 120.0, 70.0, 5.0, 5.0)];
    let s = scene(vec![
        frame,
        shape("top", NodeKind::Rect, 0.0, 0.0, 1.0, 1.0),
    ]);
    assert_eq!(layout_origin(&s, "r"), (100.0, 50.0));
    assert_eq!(layout_origin(&s, "s"), (100.0, 50.0));
    assert_eq!(layout_origin(&s, "top"), (0.0, 0.0));
    assert_eq!(layout_origin(&s, "f"), (0.0, 0.0));
}

#[test]
fn flattening_a_lone_line_keeps_it_open() {
    let mut line = shape("l", NodeKind::Line, 0.0, 0.0, 40.0, 0.0);
    line.points = vec![
        de_editor_core::render_backend::Point2D::new(0.0, 0.0),
        de_editor_core::render_backend::Point2D::new(40.0, 0.0),
    ];
    let s = scene(vec![line]);
    let p = flatten_nodes(&s, &ids(&["l"])).unwrap();
    let subs = path_to_subpaths(&p);
    assert_eq!(subs.len(), 1);
    assert!(!subs[0].closed, "an open line stays a line");
}
