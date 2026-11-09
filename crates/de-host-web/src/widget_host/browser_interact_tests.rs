//! Interaction mode of a browser node (#324) through the web host: a
//! double-click enters, the page takes pointer, wheel and keys, Escape or a
//! press outside leaves. Fixture at doc x ≥ 400 so presses land clear of
//! the chat float and the tool rail.

use super::{PageRoute, WidgetHost};
use de_editor_core::browser_input::{BrowserInputEvent, InputModifiers, MouseButton};
use de_editor_core::{NodeId, Tool};

const VW: f32 = 1440.0;
const VH: f32 = 900.0;

const PAGE: &str = r#"{"version":"1.0.0","children":[
  {"type":"browser","id":"b1","name":"Browser","x":400,"y":100,"width":400,"height":300,
   "url":"https://example.com"},
  {"type":"rectangle","id":"r1","name":"Rect","x":400,"y":600,"width":100,"height":100}
]}"#;

fn host() -> WidgetHost {
    let mut host = WidgetHost::new();
    let doc = jian_ops_schema::load_str(PAGE)
        .expect("fixture JSON parses")
        .value;
    host.editor_state = de_editor_core::EditorState::from_document(doc);
    host.editor_state.editor_ui.sidebar_open = false;
    host.editor_state.tool = Tool::Select;
    host.editor_state_dirty = true;
    host.set_now_ms(10_000);
    host
}

fn screen_at(host: &WidgetHost, doc_x: f32, doc_y: f32) -> (f32, f32) {
    let (cx0, cy0, _, _) = host.canvas_region(VW, VH);
    let v = &host.editor_state.viewport;
    (
        cx0 + v.pan_x + doc_x * v.zoom,
        cy0 + v.pan_y + doc_y * v.zoom,
    )
}

fn click_doc(host: &mut WidgetHost, doc_x: f32, doc_y: f32) {
    let (x, y) = screen_at(host, doc_x, doc_y);
    host.apply_press(x, y, VW, VH);
    let _ = host.apply_release_with_viewport(VW, VH);
}

fn interactive(host: &WidgetHost) -> Option<NodeId> {
    host.editor_state.interactive_browser()
}

fn enter(host: &mut WidgetHost) {
    click_doc(host, 600.0, 250.0);
    assert_eq!(interactive(host), None, "one click only selects");
    click_doc(host, 600.0, 250.0);
    assert_eq!(interactive(host), Some(NodeId::new("b1")));
}

fn none() -> InputModifiers {
    InputModifiers::default()
}

#[test]
fn a_double_click_enters_and_the_page_takes_the_pointer() {
    let mut host = host();
    enter(&mut host);
    assert!(host.browser_interaction_active());
    let before = host.editor_state.document_revision();
    let (x, y) = screen_at(&host, 500.0, 150.0);
    let press = host.browser_pointer_button(true, MouseButton::Left, x, y, VW, VH, none());
    assert_eq!(press, PageRoute::Page);
    assert!(host.browser_pointer_move(x + 50.0, y, VW, VH, none()));
    let release =
        host.browser_pointer_button(false, MouseButton::Left, x + 50.0, y, VW, VH, none());
    assert_eq!(release, PageRoute::Page);
    let (node, events) = host.take_browser_input().expect("queued");
    assert_eq!(node, "b1");
    assert!(matches!(
        events[0],
        BrowserInputEvent::MouseDown { x, y, .. }
            if (x - 100.0).abs() < 0.01 && (y - 50.0).abs() < 0.01
    ));
    assert_eq!(events.len(), 3);
    assert_eq!(host.editor_state.document_revision(), before, "not moved");
}

#[test]
fn the_wheel_scrolls_the_page_not_the_canvas() {
    let mut host = host();
    enter(&mut host);
    let viewport = host.editor_state.viewport;
    let (x, y) = screen_at(&host, 600.0, 250.0);
    assert!(host.browser_wheel(x, y, 0.0, -40.0, VW, VH, none()));
    assert_eq!(host.editor_state.viewport, viewport);
    let (_, events) = host.take_browser_input().unwrap();
    assert!(matches!(events[0], BrowserInputEvent::Wheel { delta_y, .. } if delta_y > 0.0));
    let (ox, oy) = screen_at(&host, 450.0, 650.0);
    assert!(!host.browser_wheel(ox, oy, 0.0, -40.0, VW, VH, none()));
}

#[test]
fn keys_reach_the_page_and_escape_leaves() {
    let mut host = host();
    enter(&mut host);
    let route = |host: &mut WidgetHost, key: &str, code: &str| {
        host.browser_key(key.into(), code.into(), Some(key.into()), none(), true)
    };
    assert_eq!(route(&mut host, "v", "KeyV"), PageRoute::Page);
    assert_eq!(host.editor_state.tool, Tool::Select, "no tool shortcut");
    assert!(host.browser_insert_text("é"));
    assert_eq!(host.take_browser_input().unwrap().1.len(), 2);
    assert_eq!(route(&mut host, "Escape", "Escape"), PageRoute::Left);
    assert_eq!(interactive(&host), None);
    assert_eq!(route(&mut host, "v", "KeyV"), PageRoute::Editor);
}

#[test]
fn a_press_outside_or_a_selection_change_leaves() {
    let mut host = host();
    enter(&mut host);
    let (x, y) = screen_at(&host, 450.0, 650.0);
    let press = host.browser_pointer_button(true, MouseButton::Left, x, y, VW, VH, none());
    assert_eq!(press, PageRoute::Left);
    assert_eq!(interactive(&host), None);

    enter(&mut host);
    host.editor_state.set_single_selection(NodeId::new("r1"));
    assert!(host.sync_browser_interaction());
    assert_eq!(interactive(&host), None);
    assert!(!host.sync_browser_interaction());
}

#[test]
fn a_button_held_when_the_tab_blurs_is_released_on_the_page() {
    let mut host = host();
    enter(&mut host);
    let (x, y) = screen_at(&host, 500.0, 150.0);
    host.browser_pointer_button(true, MouseButton::Left, x, y, VW, VH, none());
    host.take_browser_input();
    host.browser_release_pointer();
    let (_, events) = host.take_browser_input().expect("a release");
    assert!(matches!(events[..], [BrowserInputEvent::MouseUp { .. }]));
}
