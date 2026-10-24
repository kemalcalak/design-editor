//! SVG export of text with a fill stack (#179).

use crate::layout_scene::{
    LayoutScene, NodeKind, SceneFillLayer, SceneGradient, SceneGradientStop, SceneImageFit,
    SceneNode, ScenePage,
};
use crate::svg_export::serialize_active_page_svg;
use crate::{Color, ImageAdjustments, ImageBlendMode, Rect};
use std::sync::Arc;

fn page(node: SceneNode) -> LayoutScene {
    LayoutScene {
        pages: vec![ScenePage {
            id: "p1".into(),
            name: "Page 1".into(),
            children: vec![node],
        }],
        active_page_index: 0,
    }
}

fn headline() -> SceneNode {
    let mut n = SceneNode::leaf("h", NodeKind::Text);
    n.bounds = Rect::xywh(0.0, 0.0, 300.0, 80.0);
    n.text = Some("PHOTO".into());
    n.font_size = 64.0;
    n.fill = Some(Color::BLACK);
    n
}

#[test]
fn plain_text_keeps_its_solid_fill() {
    let svg = serialize_active_page_svg(&page(headline())).expect("svg");
    assert!(svg.contains(r#"fill="rgb(0,0,0)">PHOTO</text>"#), "{svg}");
    assert!(!svg.contains("text-clip-"), "{svg}");
}

#[test]
fn image_and_multiply_layers_paint_inside_a_glyph_clip() {
    let mut n = headline();
    n.fill_layers = vec![
        SceneFillLayer::Solid {
            color: Color {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            blend_mode: ImageBlendMode::Multiply,
        },
        SceneFillLayer::Image {
            src: Arc::from("photo.png"),
            src_id: 7,
            fit: SceneImageFit::Fill,
            transform: None,
            original_size: None,
            tile_scale: 1.0,
            adjustments: ImageAdjustments::default(),
            opacity: 0.5,
            blend_mode: ImageBlendMode::Normal,
        },
    ];
    let svg = serialize_active_page_svg(&page(n)).expect("svg");
    let clip = svg
        .find(r#"<clipPath id="text-clip-n-68">"#)
        .expect("glyph clip");
    assert!(svg[clip..].contains(">PHOTO</text></clipPath>"), "{svg}");
    let group = svg
        .find(r#"<g clip-path="url(#text-clip-n-68)" style="isolation:isolate">"#)
        .expect("stack group");
    let image = svg[group..]
        .find(r#"href="photo.png""#)
        .expect("image layer");
    let tint = svg[group..]
        .find(r#"style="mix-blend-mode:multiply""#)
        .expect("multiply layer");
    assert!(image < tint, "back layer first: {svg}");
    assert!(svg[group..].contains(r#"opacity="0.5""#), "{svg}");
    assert!(
        !svg.contains(r#"fill="rgb(0,0,0)">PHOTO"#),
        "no solid copy: {svg}"
    );
}

#[test]
fn a_gradient_layer_gets_its_own_def() {
    let mut n = headline();
    n.opacity = 0.8;
    n.fill_layers = vec![SceneFillLayer::Gradient {
        gradient: SceneGradient::Linear {
            angle_deg: 90.0,
            opacity: 1.0,
            stops: vec![
                SceneGradientStop {
                    offset: 0.0,
                    color: Color::BLACK,
                },
                SceneGradientStop {
                    offset: 1.0,
                    color: Color::WHITE,
                },
            ],
        },
        blend_mode: ImageBlendMode::Normal,
    }];
    let svg = serialize_active_page_svg(&page(n)).expect("svg");
    assert!(
        svg.contains(r#"<linearGradient id="gradient-n-68-fill-0""#),
        "{svg}"
    );
    assert!(
        svg.contains(r#"fill="url(#gradient-n-68-fill-0)""#),
        "{svg}"
    );
    assert!(
        svg.contains(r#"style="isolation:isolate" opacity="0.8">"#),
        "{svg}"
    );
}
