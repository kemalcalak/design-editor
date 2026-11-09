//! Interaction mode of a browser node (#324) through the native host:
//! double-click enters, the page takes pointer, wheel and keys, Escape, a
//! press outside or a selection change leave. Fixture at doc x ≥ 400 so
//! presses land clear of the chat float and the tool rail.

use super::WidgetHostNative;
use de_editor_core::browser_input::{BrowserInputEvent, InputModifiers, MouseButton};
use de_editor_core::NodeId;
use de_editor_ui::widgets::browser_interact_flow::{ButtonOutcome, KeyInput, KeyOutcome};

const VIEWPORT_W: f32 = 1440.0;
const VIEWPORT_H: f32 = 900.0;

const PAGE: &str = r#"{"version":"1.0.0","children":[
  {"type":"browser","id":"b1","name":"Browser","x":400,"y":100,"width":400,"height":300,
   "url":"https://example.com"},
  {"type":"rectangle","id":"r1","name":"Rect","x":400,"y":600,"width":100,"height":100}
]}"#;

fn host() -> WidgetHostNative {
    let mut host = WidgetHostNative::new();
    let doc = jian_ops_schema::load_str(PAGE)
        .expect("fixture JSON parses")
        .value;
    *host.editor_state_mut() = de_editor_core::EditorState::from_document(doc);
    host.mark_paint_dirty_for_test();
    host.set_now_ms(10_000);
    host
}

fn screen_at(host: &WidgetHostNative, doc_x: f32, doc_y: f32) -> (f32, f32) {
    let (cx0, cy0, _cw, _ch) = host.canvas_region(VIEWPORT_W, VIEWPORT_H);
    let v = &host.editor_state().viewport;
    (
        cx0 + v.pan_x + doc_x * v.zoom,
        cy0 + v.pan_y + doc_y * v.zoom,
    )
}

fn click_doc(host: &mut WidgetHostNative, doc_x: f32, doc_y: f32) {
    let (x, y) = screen_at(host, doc_x, doc_y);
    host.apply_press(x, y, VIEWPORT_W, VIEWPORT_H);
    let _ = host.apply_release_with_viewport(VIEWPORT_W, VIEWPORT_H);
}

fn interactive(host: &WidgetHostNative) -> Option<NodeId> {
    host.editor_state().interactive_browser()
}

fn enter(host: &mut WidgetHostNative) {
    click_doc(host, 600.0, 250.0);
    assert_eq!(interactive(host), None, "one click only selects");
    assert_eq!(host.editor_state().selection.anchor, NodeId::new("b1"));
    click_doc(host, 600.0, 250.0);
    assert_eq!(interactive(host), Some(NodeId::new("b1")));
}

fn none() -> InputModifiers {
    InputModifiers::default()
}

#[test]
fn a_double_click_enters_interaction_mode() {
    let mut host = host();
    enter(&mut host);
    assert!(host.browser_interaction_active());
    let (x, y) = screen_at(&host, 600.0, 250.0);
    assert!(host.browser_over_page(x, y, VIEWPORT_W, VIEWPORT_H));
}

#[test]
fn a_click_in_the_page_goes_to_the_page_not_the_canvas() {
    let mut host = host();
    enter(&mut host);
    let before = host.editor_state().document_revision();
    let (x, y) = screen_at(&host, 500.0, 150.0);
    let press = host.browser_pointer_button(
        true,
        MouseButton::Left,
        x,
        y,
        VIEWPORT_W,
        VIEWPORT_H,
        none(),
    );
    assert_eq!(press, ButtonOutcome::Page);
    assert!(host.browser_pointer_move(x + 50.0, y, VIEWPORT_W, VIEWPORT_H, none()));
    let release = host.browser_pointer_button(
        false,
        MouseButton::Left,
        x + 50.0,
        y,
        VIEWPORT_W,
        VIEWPORT_H,
        none(),
    );
    assert_eq!(release, ButtonOutcome::Page);
    let (node, events) = host.take_browser_input().expect("queued");
    assert_eq!(node, "b1");
    assert!(matches!(
        events[0],
        BrowserInputEvent::MouseDown { x, y, .. } if (x - 100.0).abs() < 0.01 && (y - 50.0).abs() < 0.01
    ));
    assert_eq!(events.len(), 3);
    assert_eq!(
        host.editor_state().document_revision(),
        before,
        "the node did not move"
    );
}

#[test]
fn the_wheel_scrolls_the_page_and_leaves_the_canvas_still() {
    let mut host = host();
    enter(&mut host);
    let viewport = host.editor_state().viewport;
    let (x, y) = screen_at(&host, 600.0, 250.0);
    assert!(host.browser_wheel(x, y, 0.0, -40.0, VIEWPORT_W, VIEWPORT_H, none()));
    assert_eq!(host.editor_state().viewport, viewport);
    let (_, events) = host.take_browser_input().unwrap();
    assert!(matches!(events[0], BrowserInputEvent::Wheel { delta_y, .. } if delta_y > 0.0));
    let (ox, oy) = screen_at(&host, 450.0, 650.0);
    assert!(!host.browser_wheel(ox, oy, 0.0, -40.0, VIEWPORT_W, VIEWPORT_H, none()));
}

#[test]
fn escape_leaves_and_keys_reach_the_page() {
    let mut host = host();
    enter(&mut host);
    let key = |k: &str, code: &str| KeyInput {
        key: k.into(),
        code: code.into(),
        text: Some(k.into()),
        modifiers: none(),
    };
    assert_eq!(host.browser_key(key("x", "KeyX"), true), KeyOutcome::Page);
    assert!(host.browser_insert_text("é"));
    assert_eq!(host.take_browser_input().unwrap().1.len(), 2);
    assert_eq!(
        host.browser_key(key("Escape", "Escape"), true),
        KeyOutcome::Left
    );
    assert_eq!(interactive(&host), None);
    assert_eq!(
        host.editor_state().selection.anchor,
        NodeId::new("b1"),
        "still selected, still live"
    );
    assert_eq!(host.browser_key(key("x", "KeyX"), true), KeyOutcome::Editor);
}

#[test]
fn a_press_outside_or_a_selection_change_leaves() {
    let mut host = host();
    enter(&mut host);
    let (x, y) = screen_at(&host, 450.0, 650.0);
    assert_eq!(
        host.browser_pointer_button(
            true,
            MouseButton::Left,
            x,
            y,
            VIEWPORT_W,
            VIEWPORT_H,
            none()
        ),
        ButtonOutcome::Left
    );
    assert_eq!(interactive(&host), None);

    enter(&mut host);
    host.editor_state_mut()
        .set_single_selection(NodeId::new("r1"));
    assert!(host.sync_browser_interaction());
    assert_eq!(interactive(&host), None);
    assert!(!host.sync_browser_interaction());
}
