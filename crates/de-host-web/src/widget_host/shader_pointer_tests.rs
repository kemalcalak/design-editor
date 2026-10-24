//! The web twin of native's `shader_pointer_tests`: floating chrome over
//! the canvas gives `uniform float2 mouse;` shaders the off-canvas value,
//! the same rule canvas hover follows (#141).

use super::WidgetHost;
use de_editor_ui::widgets::host_canvas_geometry as canvas_geometry;
use de_editor_ui::Point2D;

const VIEWPORT_W: f32 = 1440.0;
const VIEWPORT_H: f32 = 900.0;

fn editor_host() -> WidgetHost {
    let mut host = WidgetHost::new();
    host.last_viewport_w = VIEWPORT_W;
    host.last_viewport_h = VIEWPORT_H;
    host
}

fn centre(host: &WidgetHost) -> Point2D {
    let (x, y, w, h) = host.canvas_region(VIEWPORT_W, VIEWPORT_H);
    Point2D::new(x + w / 2.0, y + h / 2.0)
}

#[test]
fn floating_chrome_over_the_canvas_gives_shaders_no_pointer() {
    let mut host = editor_host();
    let rail = canvas_geometry::toolbar_rect_for(&host.editor_state, VIEWPORT_W, VIEWPORT_H);
    let on_rail = Point2D::new(
        rail.origin.x + rail.size.x / 2.0,
        rail.origin.y + rail.size.y / 2.0,
    );
    let pill = canvas_geometry::chrome_layout(&host.editor_state, VIEWPORT_W, VIEWPORT_H)
        .file_pill
        .expect("the file pill paints at this size")
        .rect;
    let on_pill = Point2D::new(pill.origin.x + 4.0, pill.origin.y + pill.size.y / 2.0);
    for point in [on_rail, on_pill] {
        assert!(host.over_canvas(point.x, point.y, VIEWPORT_W, VIEWPORT_H));
        let canvas = centre(&host);
        host.apply_cursor_move(canvas.x, canvas.y);
        assert_eq!(host.shader_pointer, Some(canvas));
        host.apply_cursor_move(point.x, point.y);
        assert_eq!(host.shader_pointer, None, "chrome at {point:?}");
    }
}

#[test]
fn the_chat_panel_gives_shaders_no_pointer() {
    let mut host = editor_host();
    let chat = host
        .ai_chat_rect(VIEWPORT_W, VIEWPORT_H)
        .expect("the composer pill shows on a fresh editor");
    let on_chat = Point2D::new(
        chat.origin.x + chat.size.x / 2.0,
        chat.origin.y + chat.size.y / 2.0,
    );
    let canvas = centre(&host);
    host.apply_cursor_move(canvas.x, canvas.y);
    assert_eq!(host.shader_pointer, Some(canvas));
    host.apply_cursor_move(on_chat.x, on_chat.y);
    assert_eq!(host.shader_pointer, None);
}
