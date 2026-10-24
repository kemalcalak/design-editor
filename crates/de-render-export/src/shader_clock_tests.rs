//! Exports pin time-driven shader fills to `time = 0`, or to the capture time
//! a caller scoped with `with_capture_time_ms` (#8). A live screenshot runs on
//! the UI thread between canvas frames, so without the pin its pixels would
//! depend on the canvas clock at the moment it was taken.

use super::test_support::filled_rect;
use super::*;
use de_editor_ui::layout_scene::{SceneFillType, SceneShader};
use de_editor_ui::widgets::canvas_shader_clock;
use de_editor_ui::{Color, Rect};

fn time_shader_png() -> Vec<u8> {
    let mut node = filled_rect("t", 0.0, 0.0, 8.0, 8.0, Color::WHITE);
    node.fill_type = SceneFillType::Shader;
    node.shader = Some(SceneShader {
        sksl:
            "uniform float time; half4 main(float2 p){ return half4(fract(time), 0.0, 0.0, 1.0); }"
                .to_string(),
        uniforms: Vec::new(),
        opacity: 1.0,
        fallback: Color::WHITE,
    });
    render_raster_bytes(
        Rect::xywh(0.0, 0.0, 8.0, 8.0),
        RasterFormat::Png,
        1.0,
        0.0,
        |canvas| scene_painter::paint_nodes(canvas, std::slice::from_ref(&node)),
    )
    .expect("shader node renders")
}

#[test]
fn a_screenshot_between_canvas_frames_renders_the_shader_at_time_zero() {
    canvas_shader_clock::begin_frame(250);
    let early = time_shader_png();
    canvas_shader_clock::begin_frame(750);
    let late = time_shader_png();
    assert_eq!(
        early, late,
        "the canvas clock leaked into the export: pixels depend on capture time"
    );
    assert!(
        !canvas_shader_clock::animated_shader_painted(),
        "an export must not mark the canvas frame as animated"
    );
}

/// Red channel of the shader's top-left pixel: `fract(time)` in 0..=255.
fn red_of(png: &[u8]) -> u8 {
    let image =
        skia_safe::Image::from_encoded(skia_safe::Data::new_copy(png)).expect("decode export PNG");
    let info = skia_safe::ImageInfo::new(
        (1, 1),
        skia_safe::ColorType::RGBA8888,
        skia_safe::AlphaType::Unpremul,
        None,
    );
    let mut pixel = [0u8; 4];
    assert!(image.read_pixels(
        &info,
        &mut pixel,
        4,
        (0, 0),
        skia_safe::image::CachingHint::Disallow,
    ));
    pixel[0]
}

#[test]
fn a_capture_time_drives_the_shader_clock_for_that_capture_only() {
    assert_eq!(red_of(&time_shader_png()), 0, "default capture is time = 0");
    // 1.5 s -> fract = 0.5 -> red ~ 128.
    let red = red_of(&with_capture_time_ms(1_500.0, time_shader_png));
    assert!(
        (126..=129).contains(&red),
        "expected fract(1.5) ~ 0.5, got {red}"
    );
    assert_eq!(
        red_of(&time_shader_png()),
        0,
        "the capture time leaked past its scope"
    );
    assert!(
        !canvas_shader_clock::animated_shader_painted(),
        "a timed export must not mark the canvas frame as animated"
    );
}

#[test]
fn a_capture_time_outside_the_clock_range_is_clamped_not_fed_to_the_shader() {
    let past_end = with_capture_time_ms(MAX_CAPTURE_TIME_MS + 1_000.0, time_shader_png);
    let at_end = with_capture_time_ms(MAX_CAPTURE_TIME_MS, time_shader_png);
    assert_eq!(past_end, at_end);
    let nan = with_capture_time_ms(f64::NAN, time_shader_png);
    assert_eq!(red_of(&nan), 0, "a non-finite time falls back to 0");
}

#[test]
fn a_panicking_render_restores_the_capture_time() {
    let unwound = std::panic::catch_unwind(|| with_capture_time_ms(1_500.0, || panic!("boom")));
    assert!(unwound.is_err());
    assert_eq!(red_of(&time_shader_png()), 0);
}
