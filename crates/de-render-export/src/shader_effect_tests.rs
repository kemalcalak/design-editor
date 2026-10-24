//! Exports carry shader effects (#6): the raster export paints through the
//! same scene painter as the canvas, so an object pass reaches the PNG.

use super::test_support::filled_rect;
use super::*;
use de_editor_ui::layout_scene::{Effect, SceneShaderUniform, ShaderEffect, ShaderEffectInput};
use de_editor_ui::{Color, Rect};

#[test]
fn an_export_runs_the_shader_effect_over_the_node() {
    let mut node = filled_rect("fx", 0.0, 0.0, 16.0, 8.0, Color::RED);
    // Swap red and blue over the node's own rendering.
    node.effects = vec![Effect::Shader(ShaderEffect {
        sksl: "uniform shader object; half4 main(float2 p){ return object.eval(p).bgra; }"
            .to_string(),
        uniforms: vec![SceneShaderUniform {
            name: "size".to_string(),
            values: vec![16.0, 8.0],
        }],
        input: ShaderEffectInput::Object,
    })];
    // 2x, so the effect also has to hold up off a unit scale.
    let png = render_raster_bytes(
        Rect::xywh(0.0, 0.0, 16.0, 8.0),
        RasterFormat::Png,
        2.0,
        0.0,
        |canvas| scene_painter::paint_nodes(canvas, std::slice::from_ref(&node)),
    )
    .expect("shader effect node renders");

    let image = skia_safe::Image::from_encoded(skia_safe::Data::new_copy(&png)).expect("decodes");
    assert_eq!((image.width(), image.height()), (32, 16));
    let info = skia_safe::ImageInfo::new(
        (32, 16),
        skia_safe::ColorType::RGBA8888,
        skia_safe::AlphaType::Unpremul,
        None,
    );
    let mut pixels = vec![0u8; 32 * 16 * 4];
    assert!(image.read_pixels(
        &info,
        &mut pixels,
        32 * 4,
        (0, 0),
        skia_safe::image::CachingHint::Disallow
    ));
    let center = &pixels[(8 * 32 + 16) * 4..(8 * 32 + 16) * 4 + 4];
    assert!(
        center[2] > 200 && center[0] < 60 && center[3] == 255,
        "red node exported blue: {center:?}"
    );
}
