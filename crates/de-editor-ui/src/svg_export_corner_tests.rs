//! SVG export of corner outlines (#389): smoothed rects, rounded polygons
//! and paths are written as their outline and clip to it.

use super::*;
use crate::layout_scene::{SceneCornerPath, SceneStroke, SceneStrokeAlign};
use std::sync::Arc;

fn scene_with(children: Vec<SceneNode>) -> LayoutScene {
    LayoutScene {
        pages: vec![ScenePage {
            id: "p1".into(),
            name: "Page 1".into(),
            children,
            image_thumbs: Default::default(),
        }],
        active_page_index: 0,
    }
}

fn outline(d: &str, stroke_d: &str) -> Option<Arc<SceneCornerPath>> {
    Some(Arc::new(SceneCornerPath {
        d: d.into(),
        bounds: Rect::xywh(0.0, 0.0, 40.0, 20.0),
        stroke_d: stroke_d.into(),
        stroke_bounds: Rect::ZERO,
        hit_ring: Vec::new(),
    }))
}

fn smoothed_rect(id: &str) -> SceneNode {
    let mut node = SceneNode::leaf(id, NodeKind::Rect);
    node.bounds = Rect::xywh(10.0, 20.0, 40.0, 20.0);
    node.corner_radius = 8.0;
    node.corner_radii = Some([8.0, 0.0, 4.0, 2.0]);
    node.corner_smoothing = 0.6;
    node.fill = Some(Color::rgba_u8(255, 0, 0, 1.0));
    node.corner_path = outline("M 0 0 L 40 0 C 40 10 40 20 30 20 L 0 20 Z", "");
    node
}

#[test]
fn a_smoothed_rect_is_written_as_its_placed_outline() {
    let id = "r";
    let body = serialize_node_svg(&scene_with(vec![smoothed_rect(id)]), id).expect("svg");
    assert!(!body.contains("<rect"), "{body}");
    assert!(
        body.contains(r#"d="M 10 20 L 50 20 C 50 30 50 40 40 40 L 10 40 Z""#),
        "{body}"
    );
    // Every radius is in the outline, so nothing is reported as lost.
    let warnings = serialize_nodes_svg_with(&[smoothed_rect(id)], &SvgExportOptions::default())
        .expect("svg")
        .warnings;
    assert!(
        !warnings.iter().any(|w| w.contains("per-corner")),
        "{warnings:?}"
    );
}

#[test]
fn an_aligned_stroke_writes_its_centre_line() {
    let id = "r";
    let mut node = smoothed_rect(id);
    node.stroke = Some(SceneStroke {
        color: Color::BLACK,
        width: 2.0,
        sides: None,
        align: SceneStrokeAlign::Inside,
        style: Default::default(),
    });
    node.corner_path = outline("M 0 0 L 40 0 L 40 20 Z", "M 1 1 L 39 1 L 39 19 Z");
    let body = serialize_node_svg(&scene_with(vec![node]), id).expect("svg");
    assert!(body.contains(r#"d="M 11 21 L 49 21 L 49 39 Z""#), "{body}");
}

#[test]
fn a_clipping_frame_clips_to_its_outline() {
    let mut child = SceneNode::leaf("c", NodeKind::Rect);
    child.bounds = Rect::xywh(0.0, 0.0, 400.0, 10.0);
    child.fill = Some(Color::BLACK);
    let mut frame = smoothed_rect("f");
    frame.kind = NodeKind::Frame;
    frame.clip_content = true;
    frame.children = vec![child];
    let body = serialize_active_page_svg(&scene_with(vec![frame])).expect("svg");
    let clip_id = format!("clip-{}", svg_id("f"));
    assert!(
        body.contains(&format!(
            r#"<clipPath id="{clip_id}"><path d="M 10 20 L 50 20"#
        )),
        "{body}"
    );
    assert!(
        body.contains(&format!(r#"clip-path="url(#{clip_id})""#)),
        "{body}"
    );
    assert!(!body.contains(" rx="), "{body}");
}

#[test]
fn rounded_polygons_and_paths_are_written_as_their_outline() {
    let mut poly = SceneNode::leaf("poly", NodeKind::Polygon);
    poly.bounds = Rect::xywh(100.0, 0.0, 40.0, 20.0);
    poly.polygon_sides = 3;
    poly.corner_radius = 4.0;
    poly.fill = Some(Color::BLACK);
    poly.corner_path = outline("M 20 2 C 21 2 39 18 38 19 L 2 19 C 1 18 19 2 20 2 Z", "");
    let body = serialize_node_svg(&scene_with(vec![poly]), "poly").expect("svg");
    assert!(!body.contains("<polygon"), "{body}");
    assert!(body.contains(r#"d="M 120 2 C 121 2"#), "{body}");

    let mut path = SceneNode::leaf("path", NodeKind::Path);
    path.bounds = Rect::xywh(0.0, 50.0, 40.0, 20.0);
    path.svg_path = Some("M0 0 L40 0 L40 20".into());
    path.corner_radius = 5.0;
    path.stroke = Some(SceneStroke {
        color: Color::BLACK,
        width: 1.0,
        sides: None,
        align: SceneStrokeAlign::Center,
        style: Default::default(),
    });
    path.corner_path = outline("M 0 0 L 35 0 C 38 0 40 2 40 5 L 40 20", "");
    let body = serialize_node_svg(&scene_with(vec![path]), "path").expect("svg");
    assert!(
        body.contains(r#"d="M 0 50 L 35 50 C 38 50 40 52 40 55 L 40 70""#),
        "{body}"
    );
    assert!(!body.contains("non-scaling-stroke"), "{body}");
}

#[test]
fn path_data_is_escaped_exactly_once() {
    // An imported `d` with markup characters comes out escaped once,
    // never twice, and `PathD` is the one place that escapes.
    let mut plain = SceneNode::leaf("p", NodeKind::Path);
    plain.bounds = Rect::xywh(0.0, 0.0, 40.0, 20.0);
    plain.svg_path = Some(r#"M0 0 L40 20"&<"#.into());
    plain.fill = Some(Color::rgba_u8(0, 0, 0, 1.0));
    let body = serialize_node_svg(&scene_with(vec![plain]), "p").expect("svg");
    assert!(body.contains("&quot;&amp;&lt;"), "{body}");
    assert!(!body.contains("&amp;quot;"), "{body}");
    assert_eq!(
        super::outline::PathD::new(r#"M0 0"<"#).to_string(),
        "M0 0&quot;&lt;"
    );
}
