//! Cursor moves feed `uniform float2 mouse;` shader fills: the host records
//! the canvas pointer and owes a repaint only while such a shader is shown.

use super::*;
use de_editor_ui::widgets::canvas_shader_pointer;
use de_editor_ui::Point2D;

const VIEWPORT_W: f32 = 1440.0;
const VIEWPORT_H: f32 = 900.0;

fn editor_host() -> WidgetHostNative {
    let mut host = WidgetHostNative::new();
    host.editor_state_mut().editor_ui.home.visible = false;
    host.last_viewport_w = VIEWPORT_W;
    host.last_viewport_h = VIEWPORT_H;
    host
}

/// What a canvas frame with a pointer shader on screen leaves behind.
fn paint_a_pointer_shader() {
    canvas_shader_pointer::begin_frame(None, false);
    canvas_shader_pointer::mouse_uniform_value(
        "uniform float2 mouse;",
        Rect::xywh(0.0, 0.0, 10.0, 10.0),
        &[],
    );
    canvas_shader_pointer::end_frame();
}

fn canvas_point(host: &WidgetHostNative) -> Point2D {
    let (x, y, w, h) = canvas_geometry::canvas_region(host.editor_state(), VIEWPORT_W, VIEWPORT_H);
    Point2D::new(x + w / 2.0, y + h / 2.0)
}

#[test]
fn a_canvas_move_records_the_pointer_and_repaints_a_pointer_shader() {
    let mut host = editor_host();
    paint_a_pointer_shader();
    let point = canvas_point(&host);
    assert!(host.apply_cursor_move(point.x, point.y));
    assert_eq!(host.shader_pointer, Some(point));
    assert!(
        host.apply_cursor_left(),
        "leaving repaints to the idle value"
    );
    assert_eq!(host.shader_pointer, None);
}

#[test]
fn without_a_pointer_shader_the_pointer_is_tracked_silently() {
    let mut host = editor_host();
    canvas_shader_pointer::begin_frame(None, false);
    let point = canvas_point(&host);
    host.apply_cursor_move(point.x, point.y);
    assert_eq!(host.shader_pointer, Some(point));
    assert!(!host.apply_cursor_left());
}

#[test]
fn play_mode_gives_shaders_no_pointer() {
    let mut host = editor_host();
    host.editor_state_mut().editor_ui.preview.mode = true;
    let point = canvas_point(&host);
    host.apply_cursor_move(point.x, point.y);
    assert_eq!(host.shader_pointer, None);
}
