use super::*;
use crate::test_raster::{coverage, covered};
use jian_scene::layout_scene::{SceneStroke, SceneStrokeAlign, StrokeStyle};

fn node(kind: NodeKind, x: f32, y: f32, w: f32, h: f32) -> SceneNode {
    let mut n = SceneNode::leaf("n", kind);
    n.bounds = Rect::xywh(x, y, w, h);
    n
}

#[test]
fn rect_corner_radius_rounds_the_outline() {
    let mut r = node(NodeKind::Rect, 0.0, 0.0, 40.0, 40.0);
    r.corner_radius = 10.0;
    let path = node_outline(&r).expect("outline");
    assert!(!covered(&path, 1.0, 1.0), "corner is cut");
    assert!(covered(&path, 20.0, 20.0));
    let px = coverage(&path, 40, 40) as f32;
    // 1600 − (4 − π)·100 ≈ 1514.
    assert!((px - 1514.0).abs() < 12.0, "{px}");
}

#[test]
fn per_corner_radii_round_only_their_corner() {
    let mut r = node(NodeKind::Rect, 0.0, 0.0, 40.0, 40.0);
    r.corner_radii = Some([20.0, 0.0, 0.0, 0.0]);
    let path = node_outline(&r).unwrap();
    assert!(!covered(&path, 1.0, 1.0));
    assert!(covered(&path, 39.5, 0.5));
}

#[test]
fn ellipse_arcs_and_donuts_follow_the_authored_sector() {
    let mut e = node(NodeKind::Ellipse, 0.0, 0.0, 40.0, 40.0);
    e.arc_start_angle = Some(0.0);
    e.arc_sweep_angle = Some(90.0);
    let pie = node_outline(&e).unwrap();
    assert!(covered(&pie, 30.0, 30.0), "the +x / +y quarter");
    assert!(!covered(&pie, 10.0, 10.0));
    e.arc_sweep_angle = Some(360.0);
    e.arc_inner_radius = Some(0.5);
    let ring = node_outline(&e).unwrap();
    assert!(!covered(&ring, 20.0, 20.0), "donut hole");
    assert!(covered(&ring, 2.0, 20.0));
}

#[test]
fn star_outline_uses_the_inner_radius() {
    let mut s = node(NodeKind::Polygon, 0.0, 0.0, 100.0, 100.0);
    s.polygon_sides = 5;
    let pentagon = coverage(&node_outline(&s).unwrap(), 100, 100);
    s.arc_inner_radius = Some(0.4);
    let star = coverage(&node_outline(&s).unwrap(), 100, 100);
    assert!(
        star < pentagon * 3 / 4,
        "star {star} vs pentagon {pentagon}"
    );
}

#[test]
fn a_stroked_line_contributes_its_stroke() {
    let mut l = node(NodeKind::Line, 0.0, 10.0, 40.0, 0.0);
    l.points = vec![Point2D::new(0.0, 10.0), Point2D::new(40.0, 10.0)];
    assert!(node_outline(&l).is_none(), "no stroke, no area");
    l.stroke = Some(SceneStroke {
        color: de_editor_core::Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
        width: 4.0,
        sides: None,
        align: SceneStrokeAlign::Center,
        style: StrokeStyle::default(),
    });
    let band = node_outline(&l).unwrap();
    let b = band.compute_tight_bounds();
    assert!((b.height() - 4.0).abs() < 0.01 && (b.width() - 40.0).abs() < 0.01);
}

#[test]
fn rotation_turns_the_outline_about_the_centre() {
    let mut r = node(NodeKind::Rect, 0.0, 0.0, 40.0, 10.0);
    r.rotation = std::f32::consts::FRAC_PI_2;
    let b = node_outline(&r).unwrap().compute_tight_bounds();
    assert!((b.width() - 10.0).abs() < 0.01 && (b.height() - 40.0).abs() < 0.01);
    assert!((b.left() - 15.0).abs() < 0.01);
}

#[test]
fn hidden_and_text_nodes_have_no_outline() {
    let mut r = node(NodeKind::Rect, 0.0, 0.0, 10.0, 10.0);
    r.hidden = true;
    assert!(node_outline(&r).is_none());
    assert!(node_outline(&node(NodeKind::Text, 0.0, 0.0, 10.0, 10.0)).is_none());
}

#[test]
fn a_group_outline_is_the_union_of_its_children() {
    let mut g = node(NodeKind::Group, 0.0, 0.0, 0.0, 0.0);
    g.children = vec![
        node(NodeKind::Rect, 0.0, 0.0, 10.0, 10.0),
        node(NodeKind::Rect, 20.0, 0.0, 10.0, 10.0),
    ];
    let path = node_outline(&g).unwrap();
    assert_eq!(coverage(&path, 30, 10), 200);
}

/// A scene corner outline (#389) built the way the loader builds one.
fn corner_path(d: String) -> Option<std::sync::Arc<jian_scene::layout_scene::SceneCornerPath>> {
    Some(std::sync::Arc::new(
        jian_scene::layout_scene::SceneCornerPath {
            d,
            ..Default::default()
        },
    ))
}

#[test]
fn smoothed_and_rounded_outlines_replace_the_plain_shape() {
    use de_editor_core::corner_geometry::{outline_path_data, polygon_outline, rect_outline};
    // A smoothed square at (10, 10): the corner is cut deeper than the
    // circular one, so it covers less than the plain rounded square.
    let mut r = node(NodeKind::Rect, 10.0, 10.0, 40.0, 40.0);
    r.corner_radius = 10.0;
    let plain = coverage(&node_outline(&r).unwrap(), 60, 60) as f32;
    let smooth = rect_outline(40.0, 40.0, [10.0; 4], 1.0).unwrap();
    r.corner_path = corner_path(outline_path_data(&[smooth]));
    let path = node_outline(&r).unwrap();
    assert!(covered(&path, 30.0, 30.0));
    assert!(!covered(&path, 11.0, 11.0));
    let px = coverage(&path, 60, 60) as f32;
    assert!(px < plain && px > 1400.0, "{px} vs {plain}");
    // A rounded triangle loses its sharp tips.
    let mut p = node(NodeKind::Polygon, 0.0, 0.0, 40.0, 40.0);
    p.polygon_sides = 3;
    p.corner_radius = 6.0;
    let sharp = coverage(&node_outline(&p).unwrap(), 40, 40);
    let tri: Vec<(f64, f64)> = polygon_points(Rect::xywh(0.0, 0.0, 40.0, 40.0), 3, None)
        .iter()
        .map(|q| (f64::from(q.x), f64::from(q.y)))
        .collect();
    p.corner_path = corner_path(outline_path_data(&[
        polygon_outline(&tri, 6.0, 0.0).unwrap()
    ]));
    let rounded_path = node_outline(&p).unwrap();
    let rounded = coverage(&rounded_path, 40, 40);
    // Three 60° corners at r = 6 each lose r²·(cot 30° − π/3) ≈ 24.7 px.
    assert!(
        (sharp - rounded) as f32 > 60.0 && (sharp - rounded) < 90,
        "{rounded} vs {sharp}"
    );
    let apex = tri[0];
    assert!(!covered(&rounded_path, apex.0 as f32, apex.1 as f32 + 0.5));
}
