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

/// Floating chrome over the canvas (#141): the tool rail and the file
/// pill sit inside the canvas region, but shaders see the off-canvas
/// value there, the same rule canvas hover follows.
#[test]
fn floating_chrome_over_the_canvas_gives_shaders_no_pointer() {
    let mut host = editor_host();
    let rail = canvas_geometry::toolbar_rect_for(host.editor_state(), VIEWPORT_W, VIEWPORT_H);
    let on_rail = Point2D::new(
        rail.origin.x + rail.size.x / 2.0,
        rail.origin.y + rail.size.y / 2.0,
    );
    let pill = canvas_geometry::chrome_layout(host.editor_state(), VIEWPORT_W, VIEWPORT_H)
        .file_pill
        .expect("the file pill paints at this size")
        .rect;
    let on_pill = Point2D::new(pill.origin.x + 4.0, pill.origin.y + pill.size.y / 2.0);
    for point in [on_rail, on_pill] {
        assert!(host.over_canvas(point.x, point.y, VIEWPORT_W, VIEWPORT_H));
        let centre = canvas_point(&host);
        host.apply_cursor_move(centre.x, centre.y);
        assert_eq!(host.shader_pointer, Some(centre));
        host.apply_cursor_move(point.x, point.y);
        assert_eq!(host.shader_pointer, None, "chrome at {point:?}");
    }
}

#[test]
fn play_mode_gives_shaders_no_pointer() {
    let mut host = editor_host();
    host.editor_state_mut().editor_ui.preview.mode = true;
    let point = canvas_point(&host);
    host.apply_cursor_move(point.x, point.y);
    assert_eq!(host.shader_pointer, None);
}
