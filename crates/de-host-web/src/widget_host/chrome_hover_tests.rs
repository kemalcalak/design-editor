//! Web wiring for the floating chrome (file pill + canvas action group):
//! hover, press routing and the tooltip pump, mirroring the native host.

use super::WidgetHost;
use de_editor_core::ChromeButton;
use de_editor_ui::widgets::chrome_tooltip::button_anchor;
use de_editor_ui::{Color, Point2D, Rect, RenderBackend, TextLayout};

fn hosted() -> WidgetHost {
    let mut host = WidgetHost::new();
    host.last_viewport_w = 1440.0;
    host.last_viewport_h = 900.0;
    host
}

fn centre(host: &WidgetHost, button: ChromeButton) -> Point2D {
    let rect = button_anchor(&host.editor_state, 1440.0, 900.0, button).expect("button paints");
    Point2D::new(
        rect.origin.x + rect.size.x / 2.0,
        rect.origin.y + rect.size.y / 2.0,
    )
}

#[test]
fn web_cursor_move_lights_the_hovered_chrome_button() {
    let mut host = hosted();
    let point = centre(&host, ChromeButton::OpenExportMenu);
    host.apply_cursor_move(point.x, point.y);
    assert_eq!(
        host.editor_state.editor_ui.chrome_button_hover,
        Some(ChromeButton::OpenExportMenu)
    );
    // The browser has no window controls to reveal.
    assert!(!host.editor_state.editor_ui.window_control_hover);
}

#[test]
fn web_file_pill_name_opens_the_file_menu() {
    let mut host = hosted();
    let point = centre(&host, ChromeButton::ToggleFileMenu);
    assert!(host.apply_press(point.x, point.y, 1440.0, 900.0));
    assert!(host.editor_state.editor_ui.file_menu_open);
}

#[test]
fn web_preview_button_press_is_routed_and_leaves_no_half_state() {
    let mut host = hosted();
    assert!(!host.editor_state.editor_ui.preview.mode);
    let point = centre(&host, ChromeButton::TogglePreview);

    assert!(host.apply_press(point.x, point.y, 1440.0, 900.0));

    // Entering preview needs a live CanvasKit context: the session's text
    // measurement is the browser's own Canvas2D (`BrowserMeasure::new(op_ck)`),
    // which a headless test cannot supply — `OpCk` is a wasm-bindgen bridge to
    // a JS object. So this asserts the half of the contract that IS reachable
    // here: the button routes, the press is consumed, and a host that could not
    // build the session stays consistently in design mode rather than flipping
    // the flag with no runtime behind it. The native twin
    // (`preview_input::toggle_preview`) covers the state transition itself, and
    // ties the flag to a successful session the same way.
    assert!(
        !host.editor_state.editor_ui.preview.mode,
        "no renderer, so preview must not report itself active"
    );
    assert!(
        host.preview.is_none(),
        "and no session may exist behind that flag"
    );
}

/// The browser host has no deadline scheduler, so the dwell has to be
/// observable for `tooltip_pump` to know it owes a rAF repaint. Without
/// that the tooltip would appear only if the user moved the mouse again
/// after waiting.
#[test]
fn web_chrome_hover_reports_a_pending_tooltip_until_it_is_due() {
    const DWELL: u64 = de_editor_ui::widgets::chrome_tooltip::TOOLTIP_DWELL_MS;
    let mut host = hosted();
    assert!(!host.chrome_tooltip_pending());

    host.set_now_ms(1_000);
    let point = centre(&host, ChromeButton::ToggleCodeView);
    host.apply_cursor_move(point.x, point.y);
    assert_eq!(
        host.editor_state.editor_ui.chrome_button_hover,
        Some(ChromeButton::ToggleCodeView)
    );
    assert_eq!(
        host.editor_state.editor_ui.chrome_hover_since_ms,
        Some(1_000)
    );
    assert!(host.chrome_tooltip_pending());

    // Once due there is nothing further to wake for, so the pump retires.
    host.set_now_ms(1_000 + DWELL);
    assert!(!host.chrome_tooltip_pending());
}

#[derive(Default)]
struct CaptureBackend {
    ovals: Vec<Rect>,
}

impl RenderBackend for CaptureBackend {
    fn begin_frame(&mut self) {}
    fn end_frame(&mut self) {}
    fn fill_rect(&mut self, _: Rect, _: Color) {}
    fn stroke_rect(&mut self, _: Rect, _: Color, _: f32) {}
    fn draw_text(&mut self, _: &TextLayout, _: Point2D) {}
    fn clip_rect(&mut self, _: Rect) {}
    fn stroke_line(&mut self, _: Point2D, _: Point2D, _: Color, _: f32) {}
    fn fill_round_rect(&mut self, _: Rect, _: f32, _: Color) {}
    fn stroke_round_rect(&mut self, _: Rect, _: f32, _: Color, _: f32) {}
    fn stroke_svg_path(&mut self, _: &str, _: Point2D, _: f32, _: Color, _: f32) {}
    fn fill_oval(&mut self, rect: Rect, _: Color) {
        self.ovals.push(rect);
    }
    fn save(&mut self) {}
    fn restore(&mut self) {}
    fn translate(&mut self, _: Point2D) {}
    fn resize(&mut self, _: u32, _: u32) {}
    fn dpi_scale(&self) -> f32 {
        1.0
    }
}

#[test]
fn web_does_not_paint_window_control_dots() {
    let mut host = WidgetHost::new();
    let mut backend = CaptureBackend::default();

    host.paint_editor(&mut backend, 1440.0, 900.0);

    let strip_ovals = backend
        .ovals
        .iter()
        .filter(|r| r.origin.x < 80.0 && r.origin.y < de_editor_ui::widgets::WINDOW_STRIP_H)
        .count();
    assert_eq!(strip_ovals, 0);
}
