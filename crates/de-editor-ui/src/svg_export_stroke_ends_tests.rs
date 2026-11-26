//! SVG export of stroke end geometry and miter limit (#389).

use super::*;
use crate::layout_scene::{
    SceneStroke, SceneStrokeAlign, SceneStrokeGeometry, StrokeEnd, StrokeJoin, StrokeStyle,
};
use std::sync::Arc;

fn scene_with(children: Vec<SceneNode>) -> LayoutScene {
    LayoutScene {
        pages: vec![ScenePage {
            id: "p1".into(),
            name: "Page 1".into(),
            children,
        }],
        active_page_index: 0,
    }
}

fn stroke(style: StrokeStyle) -> SceneStroke {
    SceneStroke {
        color: Color::rgba_u8(255, 0, 0, 0.5),
        width: 2.0,
        sides: None,
        align: SceneStrokeAlign::Center,
        style,
    }
}

fn arrow_line(id: &str) -> SceneNode {
    let mut line = SceneNode::leaf(id, NodeKind::Line);
    line.bounds = Rect::xywh(10.0, 20.0, 100.0, 0.0);
    line.stroke = Some(stroke(StrokeStyle {
        end: Some(StrokeEnd::LineArrow),
        ..StrokeStyle::default()
    }));
    line.stroke_geometry = Some(Arc::new(SceneStrokeGeometry {
        stroke_d: "M 0 0 L 100 0".into(),
        fill_d: "M 0 -1 A 1 1 0 0 0 -1 0 A 1 1 0 0 0 0 1 Z".into(),
        arms_d: "M 92 7 L 100 0 L 92 -7".into(),
        head_hits: Vec::new(),
    }));
    line
}

#[test]
fn a_line_with_ends_exports_its_geometry_as_placed_paths() {
    let id = "arrow";
    let body = serialize_node_svg(&scene_with(vec![arrow_line(id)]), id).expect("svg");
    assert!(!body.contains("<line"), "{body}");
    assert!(!body.contains("<marker"), "{body}");
    assert_eq!(
        body.matches(r#"transform="translate(10 20)""#).count(),
        3,
        "{body}"
    );
    assert!(body.contains(r#"d="M 0 0 L 100 0""#), "{body}");
    assert!(body.contains(r#"stroke-linecap="butt""#), "{body}");
    // The arms are mitred and never dashed; the head fill keeps the alpha.
    assert!(body.contains(r#"stroke-linejoin="miter""#), "{body}");
    assert!(body.contains(r#"fill-opacity="0.5""#), "{body}");
}

#[test]
fn an_open_path_keeps_its_fill_under_the_stroke_geometry() {
    let id = "open";
    let mut path = arrow_line(id);
    path.kind = NodeKind::Path;
    path.bounds = Rect::xywh(0.0, 0.0, 100.0, 50.0);
    path.svg_path = Some("M0 0 L100 50".into());
    path.fill = Some(Color::BLACK);
    let body = serialize_node_svg(&scene_with(vec![path]), id).expect("svg");
    assert!(body.contains(r#"d="M0 0 L100 50""#), "{body}");
    assert!(body.contains(r#"d="M 92 7 L 100 0 L 92 -7""#), "{body}");
}

#[test]
fn miter_limit_exports_as_stroke_miterlimit() {
    let id = "miter";
    let mut path = SceneNode::leaf(id, NodeKind::Path);
    path.bounds = Rect::xywh(0.0, 0.0, 80.0, 40.0);
    path.svg_path = Some("M0 0 L40 40 L80 0".into());
    path.stroke = Some(stroke(StrokeStyle {
        join: Some(StrokeJoin::Miter),
        miter_limit: Some(10.0),
        ..StrokeStyle::default()
    }));
    let body = serialize_node_svg(&scene_with(vec![path]), id).expect("svg");
    assert!(body.contains(r#"stroke-miterlimit="10""#), "{body}");
}

#[test]
fn a_dotted_line_with_ends_keeps_round_dots_and_fills_heads_nonzero() {
    use crate::layout_scene::{StrokeCap, StrokeDash};
    let id = "dots";
    let mut line = arrow_line(id);
    line.stroke = Some(stroke(StrokeStyle {
        cap: Some(StrokeCap::Round),
        dash: StrokeDash::new(&[0.0, 4.0], 0.0),
        end: Some(StrokeEnd::LineArrow),
        ..StrokeStyle::default()
    }));
    let body = serialize_node_svg(&scene_with(vec![line]), id).expect("svg");
    let centre = body
        .split("<path")
        .find(|part| part.contains(r#"d="M 0 0 L 100 0""#))
        .expect("centre line");
    // Zero-length dashes only draw with the round cap.
    assert!(centre.contains(r#"stroke-linecap="round""#), "{centre}");
    assert!(centre.contains(r#"stroke-dasharray="0 4""#), "{centre}");
    assert!(body.contains(r#"fill-rule="nonzero""#), "{body}");
}
