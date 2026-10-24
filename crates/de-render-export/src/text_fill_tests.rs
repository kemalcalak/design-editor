//! Raster export of text with an image or gradient fill (#179): the fill
//! stack shows through the glyphs and nowhere else.

use super::*;
use base64::Engine as _;
use de_editor_ui::layout_scene::{
    NodeKind, SceneFillLayer, SceneGradient, SceneGradientStop, SceneImageFit, SceneNode,
};
use de_editor_ui::{Color, ImageAdjustments, ImageBlendMode, Rect};
use std::sync::Arc;

const W: usize = 240;
const H: usize = 80;

/// A 2×1 PNG, red on the left and blue on the right, as a `data:` URL.
fn red_blue_data_url() -> String {
    let mut surface = skia_safe::surfaces::raster_n32_premul((2, 1)).expect("surface");
    let canvas = surface.canvas();
    let mut paint = skia_safe::Paint::default();
    paint.set_color(skia_safe::Color::RED);
    canvas.draw_rect(skia_safe::Rect::from_xywh(0.0, 0.0, 1.0, 1.0), &paint);
    paint.set_color(skia_safe::Color::BLUE);
    canvas.draw_rect(skia_safe::Rect::from_xywh(1.0, 0.0, 1.0, 1.0), &paint);
    let png = surface
        .image_snapshot()
        .encode(None, skia_safe::EncodedImageFormat::PNG, None)
        .expect("png encodes");
    format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(png.as_bytes())
    )
}

fn headline() -> SceneNode {
    let mut n = SceneNode::leaf("h", NodeKind::Text);
    n.bounds = Rect::xywh(0.0, 0.0, W as f32, H as f32);
    n.text = Some("HHHH".into());
    n.font_size = 64.0;
    n.font_weight = 900;
    n.fill = Some(Color::BLACK);
    n
}

/// Unpremultiplied RGBA pixels of `node` exported at 1×.
fn pixels(node: &SceneNode) -> Vec<u8> {
    let png = render_raster_bytes(
        Rect::xywh(0.0, 0.0, W as f32, H as f32),
        RasterFormat::Png,
        1.0,
        0.0,
        |canvas| scene_painter::paint_nodes(canvas, std::slice::from_ref(node)),
    )
    .expect("text renders");
    let image = skia_safe::Image::from_encoded(skia_safe::Data::new_copy(&png)).expect("decodes");
    let info = skia_safe::ImageInfo::new(
        (W as i32, H as i32),
        skia_safe::ColorType::RGBA8888,
        skia_safe::AlphaType::Unpremul,
        None,
    );
    let mut out = vec![0u8; W * H * 4];
    assert!(image.read_pixels(
        &info,
        &mut out,
        W * 4,
        (0, 0),
        skia_safe::image::CachingHint::Disallow
    ));
    out
}

/// A pixel position.
type Px = (usize, usize);

/// Pixels the plain black glyphs fully cover, and ones they leave empty.
fn coverage() -> (Vec<Px>, Vec<Px>) {
    let solid = pixels(&headline());
    let (mut inside, mut outside) = (Vec::new(), Vec::new());
    for y in 0..H {
        for x in 0..W {
            match solid[(y * W + x) * 4 + 3] {
                255 => inside.push((x, y)),
                0 => outside.push((x, y)),
                _ => {}
            }
        }
    }
    assert!(inside.len() > 200, "glyphs paint: {}", inside.len());
    (inside, outside)
}

fn at(px: &[u8], (x, y): Px) -> [u8; 4] {
    let i = (y * W + x) * 4;
    [px[i], px[i + 1], px[i + 2], px[i + 3]]
}

#[test]
fn an_image_fill_shows_through_the_glyphs_only() {
    let (inside, outside) = coverage();
    let mut node = headline();
    node.fill_layers = vec![SceneFillLayer::Image {
        src: Arc::from(red_blue_data_url()),
        src_id: 0,
        fit: SceneImageFit::Stretch,
        transform: None,
        original_size: None,
        tile_scale: 1.0,
        adjustments: ImageAdjustments::default(),
        opacity: 1.0,
        blend_mode: ImageBlendMode::Normal,
    }];
    let px = pixels(&node);
    let quarter = W / 4;
    let left = inside
        .iter()
        .find(|(x, _)| *x < quarter)
        .expect("left glyph");
    let right = inside
        .iter()
        .find(|(x, _)| *x > W - quarter)
        .expect("right glyph");
    let (l, r) = (at(&px, *left), at(&px, *right));
    assert!(
        l[0] > 200 && l[2] < 60 && l[3] == 255,
        "left glyph red: {l:?}"
    );
    assert!(
        r[2] > 200 && r[0] < 60 && r[3] == 255,
        "right glyph blue: {r:?}"
    );
    let leaked = outside.iter().filter(|p| at(&px, **p)[3] > 0).count();
    assert_eq!(leaked, 0, "no image outside the glyphs");
}

#[test]
fn a_gradient_fill_runs_across_the_glyphs() {
    let (inside, outside) = coverage();
    let mut node = headline();
    node.fill_layers = vec![SceneFillLayer::Gradient {
        gradient: SceneGradient::Linear {
            angle_deg: 90.0,
            opacity: 1.0,
            stops: vec![
                SceneGradientStop {
                    offset: 0.0,
                    color: Color {
                        r: 1.0,
                        g: 0.0,
                        b: 0.0,
                        a: 1.0,
                    },
                },
                SceneGradientStop {
                    offset: 1.0,
                    color: Color {
                        r: 0.0,
                        g: 0.0,
                        b: 1.0,
                        a: 1.0,
                    },
                },
            ],
        },
        blend_mode: ImageBlendMode::Normal,
    }];
    let px = pixels(&node);
    let first = *inside.iter().min_by_key(|(x, _)| *x).expect("glyph");
    let last = *inside.iter().max_by_key(|(x, _)| *x).expect("glyph");
    let (l, r) = (at(&px, first), at(&px, last));
    assert!(l[0] > r[0] && r[2] > l[2], "red to blue: {l:?} -> {r:?}");
    let leaked = outside.iter().filter(|p| at(&px, **p)[3] > 0).count();
    assert_eq!(leaked, 0, "no gradient outside the glyphs");
}

#[test]
fn a_multiply_tint_lands_on_the_image_inside_the_glyphs() {
    let (inside, _) = coverage();
    let mut node = headline();
    node.fill_layers = vec![
        SceneFillLayer::Solid {
            color: Color {
                r: 0.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            },
            blend_mode: ImageBlendMode::Multiply,
        },
        SceneFillLayer::Image {
            src: Arc::from(red_blue_data_url()),
            src_id: 0,
            fit: SceneImageFit::Stretch,
            transform: None,
            original_size: None,
            tile_scale: 1.0,
            adjustments: ImageAdjustments::default(),
            opacity: 1.0,
            blend_mode: ImageBlendMode::Normal,
        },
    ];
    let px = pixels(&node);
    // Cyan × red = black on the left; cyan × blue = blue on the right.
    let left = *inside.iter().min_by_key(|(x, _)| *x).expect("glyph");
    let right = *inside.iter().max_by_key(|(x, _)| *x).expect("glyph");
    let (l, r) = (at(&px, left), at(&px, right));
    assert!(
        l[0] < 60 && l[1] < 60 && l[2] < 60,
        "left multiplied dark: {l:?}"
    );
    assert!(r[2] > 200 && r[0] < 60, "right stays blue: {r:?}");
}
