use super::*;
use de_editor_core::host_wheel_routing::{MACOS_WHEEL_LINE_PX, WHEEL_LINE_PX};
use winit::dpi::PhysicalPosition;

const VIEWPORT: (f32, f32) = (1200.0, 800.0);

fn windows_default(_axis: WheelAxis) -> (NotchScale, f32) {
    (NotchScale::Lines(3.0), WHEEL_LINE_PX)
}

fn route(delta: MouseScrollDelta, shift: bool, zoom: bool) -> WheelRoute {
    wheel_route(delta, shift, zoom, 2.0, VIEWPORT, windows_default)
}

fn assert_pan(route: WheelRoute, dx: f32, dy: f32) {
    let WheelRoute::Pan {
        dx: got_x,
        dy: got_y,
    } = route
    else {
        panic!("expected a pan, got {route:?}");
    };
    assert!((got_x - dx).abs() < 1e-3, "dx {got_x} != {dx}");
    assert!((got_y - dy).abs() < 1e-3, "dy {got_y} != {dy}");
}

#[test]
fn windows_wheel_notch_pans_100_px() {
    // Before #303 this notch zoomed the canvas and moved panels 16 px.
    assert_pan(
        route(MouseScrollDelta::LineDelta(0.0, 1.0), false, false),
        0.0,
        100.0,
    );
    assert_pan(
        route(MouseScrollDelta::LineDelta(0.0, -3.0), false, false),
        0.0,
        -300.0,
    );
}

#[test]
fn precision_touchpad_fraction_scales_linearly() {
    // Windows precision touchpads arrive as fractional `LineDelta`s.
    assert_pan(
        route(MouseScrollDelta::LineDelta(0.0, 0.1), false, false),
        0.0,
        10.0,
    );
}

#[test]
fn windows_horizontal_wheel_is_no_longer_dropped() {
    // `WM_MOUSEHWHEEL` (tilt wheel, two-finger sideways) reports x.
    assert_pan(
        route(MouseScrollDelta::LineDelta(-1.0, 0.0), false, false),
        -100.0,
        0.0,
    );
}

#[test]
fn shift_wheel_pans_horizontally() {
    assert_pan(
        route(MouseScrollDelta::LineDelta(0.0, 1.0), true, false),
        100.0,
        0.0,
    );
}

#[test]
fn shift_wheel_uses_the_horizontal_setting_and_page() {
    let per_axis = |axis: WheelAxis| match axis {
        WheelAxis::Vertical => (NotchScale::Lines(3.0), WHEEL_LINE_PX),
        WheelAxis::Horizontal => (NotchScale::Page, WHEEL_LINE_PX),
    };
    let route = wheel_route(
        MouseScrollDelta::LineDelta(0.0, 1.0),
        true,
        false,
        1.0,
        VIEWPORT,
        per_axis,
    );
    assert_pan(route, VIEWPORT.0, 0.0);
}

#[test]
fn zoom_and_idle_axes_skip_the_settings_read() {
    let reads = std::cell::Cell::new(0);
    let counting = |axis: WheelAxis| {
        reads.set(reads.get() + 1);
        windows_default(axis)
    };
    wheel_route(
        MouseScrollDelta::LineDelta(0.0, 1.0),
        false,
        true,
        1.0,
        VIEWPORT,
        counting,
    );
    assert_eq!(reads.get(), 0, "zoom reads no lines setting");
    wheel_route(
        MouseScrollDelta::LineDelta(0.0, 1.0),
        false,
        false,
        1.0,
        VIEWPORT,
        counting,
    );
    assert_eq!(reads.get(), 1, "only the moving axis is read");
}

#[test]
fn modifier_wheel_zooms_at_the_old_rate() {
    assert_eq!(
        route(MouseScrollDelta::LineDelta(0.0, 1.0), false, true),
        WheelRoute::Zoom { delta_y: 16.0 }
    );
    assert_eq!(
        route(MouseScrollDelta::LineDelta(0.0, -0.5), true, true),
        WheelRoute::Zoom { delta_y: -8.0 }
    );
}

#[test]
fn page_scroll_setting_moves_a_viewport() {
    let page = |_axis: WheelAxis| (NotchScale::Page, WHEEL_LINE_PX);
    let route = wheel_route(
        MouseScrollDelta::LineDelta(0.0, -1.0),
        false,
        false,
        1.0,
        VIEWPORT,
        page,
    );
    assert_pan(route, 0.0, -800.0);
}

#[test]
fn macos_line_is_40_px() {
    let mac = |_axis: WheelAxis| (NotchScale::Lines(1.0), MACOS_WHEEL_LINE_PX);
    let route = wheel_route(
        MouseScrollDelta::LineDelta(0.0, 2.5),
        false,
        false,
        2.0,
        VIEWPORT,
        mac,
    );
    assert_pan(route, 0.0, 100.0);
}

#[test]
fn trackpad_pixels_stay_one_to_one_in_logical_px() {
    let delta = MouseScrollDelta::PixelDelta(PhysicalPosition::new(30.0, -60.0));
    assert_pan(route(delta, false, false), 15.0, -30.0);
    assert_eq!(
        route(delta, false, true),
        WheelRoute::Zoom { delta_y: -30.0 }
    );
}

fn app_over_canvas() -> DesktopApp {
    let mut app = DesktopApp::new(None);
    app.host.editor_state_mut().editor_ui.home.visible = false;
    app.viewport_width = VIEWPORT.0;
    app.viewport_height = VIEWPORT.1;
    app.cursor_x = VIEWPORT.0 / 2.0;
    app.cursor_y = VIEWPORT.1 / 2.0;
    app
}

#[test]
fn desktop_wheel_pans_the_canvas_by_one_notch() {
    let mut app = app_over_canvas();
    let before = app.host.editor_state().viewport;
    app.on_mouse_wheel(MouseScrollDelta::LineDelta(0.0, 1.0));
    let after = &app.host.editor_state().viewport;
    let (scale, line_px) = platform_notch_scale(WheelAxis::Vertical);
    let expected = line_delta_px(1.0, scale, line_px, VIEWPORT.1);
    assert_eq!(after.zoom, before.zoom, "a plain wheel must not zoom");
    assert_eq!(after.pan_x, before.pan_x);
    assert!(
        (after.pan_y - before.pan_y - expected).abs() < 1e-3,
        "pan moved {} px, expected {expected}",
        after.pan_y - before.pan_y
    );
}

#[test]
fn desktop_shift_wheel_pans_sideways_and_modifier_zooms() {
    let mut app = app_over_canvas();
    app.shift_modifier = true;
    let before = app.host.editor_state().viewport;
    app.on_mouse_wheel(MouseScrollDelta::LineDelta(0.0, -1.0));
    let after = app.host.editor_state().viewport;
    assert_eq!(after.pan_y, before.pan_y);
    assert!(after.pan_x < before.pan_x, "shift+wheel pans horizontally");

    app.shift_modifier = false;
    app.zoom_modifier = true;
    app.on_mouse_wheel(MouseScrollDelta::LineDelta(0.0, 1.0));
    assert!(
        app.host.editor_state().viewport.zoom > after.zoom,
        "Cmd/Ctrl+wheel up zooms in"
    );
}
