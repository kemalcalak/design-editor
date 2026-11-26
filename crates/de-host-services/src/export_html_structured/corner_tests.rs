//! Structured HTML export of corner outlines (#389).

use super::{shape, vector};
use de_editor_ui::layout_scene::{
    NodeKind, SceneCornerPath, SceneNode, SceneStroke, SceneStrokeAlign,
};
use de_editor_ui::{Color, Point2D, Rect};
use std::sync::Arc;

fn outline(d: &str, stroke_d: &str) -> Option<Arc<SceneCornerPath>> {
    Some(Arc::new(SceneCornerPath {
        d: d.into(),
        bounds: Rect::xywh(0.0, 0.0, 40.0, 20.0),
        stroke_d: stroke_d.into(),
        stroke_bounds: Rect::ZERO,
        hit_ring: Vec::new(),
    }))
}

fn stroke(align: SceneStrokeAlign) -> Option<SceneStroke> {
    Some(SceneStroke {
        color: Color::BLACK,
        width: 2.0,
        sides: None,
        align,
        style: Default::default(),
    })
}

fn boxed(n: &SceneNode) -> String {
    let rect = Rect::xywh(0.0, 0.0, n.bounds.size.x, n.bounds.size.y);
    let mut out = String::new();
    shape::open(&mut out, n, rect, false);
    shape::close(&mut out, n, rect, false);
    out
}

#[test]
fn a_smoothed_box_clips_its_background_layer_to_the_outline() {
    let mut n = SceneNode::leaf("r", NodeKind::Frame);
    n.bounds = Rect::xywh(10.0, 10.0, 40.0, 20.0);
    n.corner_radius = 8.0;
    n.fill = Some(Color::RED);
    n.clip_content = true;
    n.stroke = stroke(SceneStrokeAlign::Inside);
    n.corner_path = outline("M 0 0 L 40 0 L 40 20 Z", "M 1 1 L 39 1 L 39 19 Z");
    let html = boxed(&n);
    let clip = "clip-path:path('M 0 0 L 40 0 L 40 20 Z')";
    // The box clips its children; the background lives in a clipped layer.
    assert_eq!(html.matches(clip).count(), 2, "{html}");
    assert!(!html.contains("overflow:hidden"), "{html}");
    let (box_style, rest) = html.split_once('>').expect("box");
    assert!(!box_style.contains("background-color"), "{html}");
    assert!(rest.contains("background-color"), "{html}");
    // The stroke follows its aligned centre line in an inline SVG.
    assert!(
        html.contains(r#"<path d="M 1 1 L 39 1 L 39 19 Z" fill="none""#),
        "{html}"
    );
    assert!(!html.contains("border-style"), "{html}");
}

#[test]
fn a_plain_rounded_box_keeps_css_border_radius() {
    let mut n = SceneNode::leaf("r", NodeKind::Rect);
    n.bounds = Rect::xywh(0.0, 0.0, 40.0, 20.0);
    n.corner_radius = 8.0;
    n.fill = Some(Color::RED);
    n.stroke = stroke(SceneStrokeAlign::Center);
    let html = boxed(&n);
    assert!(!html.contains("clip-path"), "{html}");
    assert!(html.contains("border-radius:8px"), "{html}");
    assert!(html.contains("border-style:solid"), "{html}");
}

#[test]
fn rounded_polygons_and_paths_write_their_placed_outline() {
    let mut poly = SceneNode::leaf("p", NodeKind::Polygon);
    poly.bounds = Rect::xywh(100.0, 50.0, 40.0, 20.0);
    poly.polygon_sides = 3;
    poly.corner_radius = 4.0;
    poly.fill = Some(Color::BLACK);
    poly.corner_path = outline("M 20 2 L 38 19 L 2 19 Z", "");
    let mut out = String::new();
    vector::emit_polygon(&mut out, &poly, Point2D::ZERO);
    assert!(!out.contains("<polygon"), "{out}");
    assert!(
        out.contains(r#"<path d="M 120 52 L 138 69 L 102 69 Z""#),
        "{out}"
    );

    let mut path = SceneNode::leaf("q", NodeKind::Path);
    path.bounds = Rect::xywh(0.0, 0.0, 40.0, 20.0);
    path.points = vec![
        Point2D::new(0.0, 0.0),
        Point2D::new(40.0, 0.0),
        Point2D::new(40.0, 20.0),
    ];
    path.fill = Some(Color::RED);
    path.stroke = stroke(SceneStrokeAlign::Center);
    path.corner_path = outline("M 0 0 L 35 0 C 38 0 40 2 40 5 L 40 20", "");
    let mut out = String::new();
    vector::emit_path(&mut out, &path, Point2D::ZERO);
    assert!(
        out.contains(r#"d="M 0 0 L 35 0 C 38 0 40 2 40 5 L 40 20""#),
        "{out}"
    );
    // An open pen path paints no fill, as on the canvas.
    assert!(out.contains(r#"fill="none" stroke="rgb(0,0,0)""#), "{out}");
}
