//! Outlined text in the SVG export (#282).

use crate::layout_scene::{
    LayoutScene, NodeKind, SceneNode, ScenePage, SceneStroke, SceneStrokeAlign, SceneWidget,
    StrokeDash, StrokeStyle,
};
use crate::svg_export::serialize_node_svg;
use crate::{Color, Rect};

fn outlined(align: SceneStrokeAlign) -> SceneNode {
    let mut n = SceneNode::leaf("t", NodeKind::Text);
    n.bounds = Rect::xywh(0.0, 0.0, 200.0, 40.0);
    n.text = Some("Outlined".into());
    n.font_size = 24.0;
    n.fill = Some(Color::BLACK);
    n.stroke = Some(SceneStroke {
        color: Color {
            r: 1.0,
            g: 0.0,
            b: 1.0,
            a: 1.0,
        },
        width: 2.0,
        sides: None,
        align,
        style: StrokeStyle::default(),
    });
    n
}

fn svg(n: SceneNode) -> String {
    let scene = LayoutScene {
        pages: vec![ScenePage {
            id: "p1".into(),
            name: "Page 1".into(),
            children: vec![n],
        }],
        active_page_index: 0,
    };
    serialize_node_svg(&scene, "t").expect("svg")
}

/// Byte offsets of the filled text and of the outline text.
fn positions(body: &str, outline_width: &str) -> (usize, usize) {
    let fill = body.find(r#"fill="rgb(0,0,0)""#).expect("the fill text");
    let outline = body
        .find(&format!(
            r#"fill="none" stroke="rgb(255,0,255)" stroke-width="{outline_width}""#
        ))
        .unwrap_or_else(|| panic!("an outline {outline_width} wide in {body}"));
    (fill, outline)
}

#[test]
fn a_centred_outline_follows_the_fill_at_its_width() {
    let body = svg(outlined(SceneStrokeAlign::Center));
    let (fill, outline) = positions(&body, "2");
    assert!(outline > fill, "{body}");
}

#[test]
fn an_outside_outline_is_twice_the_width_under_the_fill() {
    let body = svg(outlined(SceneStrokeAlign::Outside));
    let (fill, outline) = positions(&body, "4");
    assert!(outline < fill, "{body}");
}

#[test]
fn an_inside_outline_is_clipped_to_the_glyphs() {
    let body = svg(outlined(SceneStrokeAlign::Inside));
    let (fill, outline) = positions(&body, "4");
    assert!(outline > fill, "{body}");
    let open = r#"<clipPath id=""#;
    let start = body.find(open).expect("a glyph clip") + open.len();
    let id = &body[start..start + body[start..].find('"').unwrap()];
    assert!(id.starts_with("text-outline-clip-"), "{body}");
    assert!(
        body.contains(&format!(r#"<g clip-path="url(#{id})"><text"#)),
        "{body}"
    );
}

#[test]
fn the_outline_carries_the_dash() {
    let mut n = outlined(SceneStrokeAlign::Center);
    n.stroke.as_mut().unwrap().style.dash = StrokeDash::new(&[3.0, 1.0], 0.0);
    assert!(svg(n).contains(r#"stroke-dasharray="3 1""#));
}

#[test]
fn widget_text_is_not_outlined() {
    let mut n = outlined(SceneStrokeAlign::Center);
    n.widget = Some(SceneWidget {
        kind: "text_input".into(),
        ..Default::default()
    });
    assert!(!svg(n).contains(r#"fill="none""#));
}

#[test]
fn decorations_are_not_outlined() {
    let mut n = outlined(SceneStrokeAlign::Center);
    n.underline = true;
    let body = svg(n);
    let outline = body.split(r#"fill="none""#).nth(1).expect("an outline");
    let outline = &outline[..outline.find("</text>").unwrap()];
    assert!(!outline.contains("text-decoration"), "{body}");
    assert!(
        body.contains("text-decoration"),
        "the fill keeps it: {body}"
    );
}
