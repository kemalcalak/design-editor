//! Exports pin time-driven shader fills to `time = 0` and pointer-reactive ones
//! to the idle `mouse` value. A live screenshot runs on the UI thread between
//! canvas frames, so without the pin its pixels would depend on the canvas
//! clock and pointer at the moment it was taken.

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

fn mouse_shader_png() -> Vec<u8> {
    let mut node = filled_rect("m", 0.0, 0.0, 8.0, 8.0, Color::WHITE);
    node.fill_type = SceneFillType::Shader;
    node.shader = Some(SceneShader {
        sksl: "uniform float2 mouse; half4 main(float2 p){ return half4(step(0.0, mouse.x), 0.0, 0.0, 1.0); }"
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
fn a_screenshot_renders_a_pointer_shader_at_the_idle_pointer() {
    use de_editor_ui::widgets::canvas_shader_pointer;
    use de_editor_ui::Point2D;
    canvas_shader_pointer::begin_frame(None);
    let idle = mouse_shader_png();
    // A canvas frame left the pointer over the node: the export must not see it.
    canvas_shader_pointer::begin_frame(Some(Point2D::new(4.0, 4.0)));
    let hovered = mouse_shader_png();
    assert_eq!(idle, hovered, "the canvas pointer leaked into the export");
    assert!(
        !canvas_shader_pointer::pointer_shader_painted(),
        "an export must not mark the canvas frame as pointer-reactive"
    );
}
