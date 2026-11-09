//! Preview pages (#324) through the native host: a browser node in the
//! previewed screen paints as a page, takes a press and the keys once its
//! live frame is on screen, and the device switcher stays in front.

#![cfg(all(test, feature = "gl-host", not(target_os = "windows")))]

use super::WidgetHostNative;
use de_editor_core::browser_input::{BrowserInputEvent, InputModifiers, MouseButton};
use de_editor_core::EditorState;
use de_editor_ui::widgets::browser_interact_flow::{KeyInput, KeyOutcome};
use de_editor_ui::widgets::{browser_preview, browser_textures, PreviewDeviceSwitcher};
use de_editor_ui::{Color, Point2D, Rect, RenderBackend, TextLayout};

const VIEWPORT_W: f32 = 1000.0;
const VIEWPORT_H: f32 = 700.0;
const NODE: &str = "bpi-page";

fn document() -> jian_ops_schema::PenDocument {
    jian_ops_schema::load_str(
        r##"{
            "version":"1.1","formatVersion":"1.1","id":"preview-pages",
            "app":{"name":"preview-pages","version":"1","id":"preview-pages"},
            "children":[{
                "type":"frame","id":"screen","x":0,"y":0,"width":1000,"height":700,
                "fill":[{"type":"solid","color":"#ffffff"}],
                "children":[{
                    "type":"browser","id":"bpi-page","x":560,"y":120,"width":350,"height":300,
                    "url":"https://example.com"
                }]
            }]
        }"##,
    )
    .expect("preview page fixture parses")
    .value
}

/// Every image counts as resident, so a live frame paints.
struct StubBackend;

impl RenderBackend for StubBackend {
    fn begin_frame(&mut self) {}
    fn end_frame(&mut self) {}
    fn fill_rect(&mut self, _: Rect, _: Color) {}
    fn stroke_rect(&mut self, _: Rect, _: Color, _: f32) {}
    fn draw_text(&mut self, _: &TextLayout, _: Point2D) {}
    fn clip_rect(&mut self, _: Rect) {}
    fn save(&mut self) {}
    fn restore(&mut self) {}
    fn translate(&mut self, _: Point2D) {}
    fn stroke_line(&mut self, _: Point2D, _: Point2D, _: Color, _: f32) {}
    fn fill_round_rect(&mut self, _: Rect, _: f32, _: Color) {}
    fn stroke_round_rect(&mut self, _: Rect, _: f32, _: Color, _: f32) {}
    fn stroke_svg_path(&mut self, _: &str, _: Point2D, _: f32, _: Color, _: f32) {}
    fn resize(&mut self, _: u32, _: u32) {}
    fn dpi_scale(&self) -> f32 {
        1.0
    }
    fn image_resident(&mut self, _: u64) -> bool {
        true
    }
}

fn previewing() -> WidgetHostNative {
    let mut host = WidgetHostNative::new();
    host.install_imported_state(EditorState::from_document(document()));
    // Nothing floats over the device frame.
    host.editor_state_mut().editor_ui.sidebar_open = false;
    host.last_viewport_w = VIEWPORT_W;
    host.last_viewport_h = VIEWPORT_H;
    host.set_now_ms(600_000);
    assert!(host.enter_preview((VIEWPORT_W, VIEWPORT_H)));
    host.settle_mode_transition();
    host
}

/// One preview frame through the device frame, as the paint arm runs it.
fn paint(host: &mut WidgetHostNative) {
    let canvas = host.preview_canvas_rect(VIEWPORT_W, VIEWPORT_H);
    browser_preview::begin_frame();
    host.paint_device_frame(&mut StubBackend, canvas);
    browser_preview::end_frame();
}

fn none() -> InputModifiers {
    InputModifiers::default()
}

#[test]
fn a_live_preview_page_takes_a_press_and_the_keys_and_the_chrome_stays_in_front() {
    let mut host = previewing();
    browser_textures::forget_live(NODE);
    paint(&mut host);
    let still = browser_preview::page(NODE).expect("the preview painted the page");
    assert!(!still.live, "no stream yet");
    let centre = Point2D::new(
        still.shown.origin.x + still.shown.size.x * 0.5,
        still.shown.origin.y + still.shown.size.y * 0.5,
    );
    assert!(
        !host.preview_page_pointer_button(
            true,
            MouseButton::Left,
            centre.x,
            centre.y,
            VIEWPORT_W,
            VIEWPORT_H,
            none()
        ),
        "a page still loading takes no input"
    );

    browser_textures::set_live_frame(NODE, 1);
    paint(&mut host);
    assert!(browser_preview::page(NODE).is_some_and(|page| page.live));
    assert!(host.preview_over_page(centre.x, centre.y, VIEWPORT_W, VIEWPORT_H));
    assert!(host.preview_page_pointer_button(
        true,
        MouseButton::Left,
        centre.x,
        centre.y,
        VIEWPORT_W,
        VIEWPORT_H,
        none()
    ));
    assert!(host.preview_page_pointer_button(
        false,
        MouseButton::Left,
        centre.x,
        centre.y,
        VIEWPORT_W,
        VIEWPORT_H,
        none()
    ));
    let key = KeyInput {
        key: "a".into(),
        code: "KeyA".into(),
        text: Some("a".into()),
        modifiers: none(),
    };
    assert_eq!(host.preview_page_key(key, true), KeyOutcome::Page);
    let (node, events) = host.take_browser_input().expect("queued for the page");
    assert_eq!(node, NODE);
    assert!(matches!(
        events.first(),
        Some(BrowserInputEvent::MouseDown { .. })
    ));

    // The device switcher paints over the preview: never the page's.
    let canvas = host.preview_canvas_rect(VIEWPORT_W, VIEWPORT_H);
    let pill = PreviewDeviceSwitcher::pill_rect(canvas);
    let on_pill = Point2D::new(
        pill.origin.x + pill.size.x * 0.5,
        pill.origin.y + pill.size.y * 0.5,
    );
    assert!(host.over_preview_chrome(on_pill.x, on_pill.y, VIEWPORT_W, VIEWPORT_H));
    assert!(!host.preview_over_page(on_pill.x, on_pill.y, VIEWPORT_W, VIEWPORT_H));

    // Closing the preview lets the page go.
    host.exit_preview();
    host.set_now_ms(610_000);
    host.settle_mode_transition();
    host.sync_preview_pages();
    assert!(!host.editor_state().editor_ui.preview.mode);
    assert!(browser_preview::page(NODE).is_none());
    browser_textures::forget_live(NODE);
}
