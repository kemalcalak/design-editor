//! Exports and screenshots carry a shader fill's image texture (#9): the
//! headless decode pump makes the child's raster resident before the real
//! paint, so the export never ships the transparent not-yet-decoded child.

use super::test_support::filled_rect;
use super::*;
use base64::Engine as _;
use de_editor_core::shader_images::{encode_image_id, register_image_source};
use de_editor_ui::layout_scene::{SceneFillType, SceneShader, SceneShaderUniform};
use de_editor_ui::{Color, Rect};

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

#[test]
fn an_export_samples_the_shader_image_texture() {
    let id = register_image_source(&red_blue_data_url());
    let mut node = filled_rect("img", 0.0, 0.0, 16.0, 8.0, Color::WHITE);
    node.fill_type = SceneFillType::Shader;
    node.shader = Some(SceneShader {
        sksl: "uniform shader photo; half4 main(float2 p){ return photo.eval(p); }".to_string(),
        uniforms: vec![SceneShaderUniform {
            name: "photo".to_string(),
            values: encode_image_id(id).to_vec(),
        }],
        opacity: 1.0,
        fallback: Color::GREEN,
    });
    let png = render_raster_bytes(
        Rect::xywh(0.0, 0.0, 16.0, 8.0),
        RasterFormat::Png,
        1.0,
        0.0,
        |canvas| scene_painter::paint_nodes(canvas, std::slice::from_ref(&node)),
    )
    .expect("shader node renders");

    let image = skia_safe::Image::from_encoded(skia_safe::Data::new_copy(&png)).expect("decodes");
    let info = skia_safe::ImageInfo::new(
        (16, 8),
        skia_safe::ColorType::RGBA8888,
        skia_safe::AlphaType::Unpremul,
        None,
    );
    let mut pixels = vec![0u8; 16 * 8 * 4];
    assert!(image.read_pixels(
        &info,
        &mut pixels,
        16 * 4,
        (0, 0),
        skia_safe::image::CachingHint::Disallow
    ));
    let at = |x: usize, y: usize| -> &[u8] { &pixels[(y * 16 + x) * 4..(y * 16 + x) * 4 + 4] };
    let (left, right) = (at(2, 4), at(13, 4));
    assert!(
        left[0] > 200 && left[2] < 60 && left[3] == 255,
        "left is red: {left:?}"
    );
    assert!(
        right[2] > 200 && right[0] < 60 && right[3] == 255,
        "right is blue: {right:?}"
    );
}
