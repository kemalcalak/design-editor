//! Preview routing against browser `b1` (400 × 300, page laid out at
//! 400 × 300 CSS px) painted by a preview frame at screen (400, 300) –
//! (800, 600). The record is per thread; the paint registry's lock keeps
//! the process-wide visible set to one test at a time.

use de_editor_core::browser_input::{BrowserInputEvent, InputModifiers, MouseButton};
use de_editor_core::EditorState;

use super::super::{BrowserInteraction, KeyInput, KeyOutcome};
use crate::widgets::browser_preview;
use crate::widgets::browser_textures::{self, lock_for_tests};
use crate::{Point2D, Rect};

fn previewing() -> EditorState {
    let doc = serde_json::from_value(serde_json::json!({
        "version": "1.0",
        "children": [{ "type": "browser", "id": "b1", "x": 100, "y": 100,
                       "width": 400, "height": 300, "url": "https://example.com" }],
    }))
    .expect("document");
    let mut state = EditorState::new();
    state.replace_document(doc);
    state.editor_ui.preview.mode = true;
    state
}

/// A preview frame that showed `b1`, `live` or still.
fn painted(live: bool) {
    browser_preview::begin_frame();
    browser_preview::with_clip(
        Rect {
            origin: Point2D::new(0.0, 0.0),
            size: Point2D::new(2000.0, 1200.0),
        },
        || {
            browser_preview::note(
                "b1",
                Rect {
                    origin: Point2D::new(400.0, 300.0),
                    size: Point2D::new(400.0, 300.0),
                },
                live,
            )
        },
    );
    browser_preview::end_frame();
    browser_textures::take_visible();
}

fn none() -> InputModifiers {
    InputModifiers::default()
}

fn key(key: &str) -> KeyInput {
    KeyInput {
        key: key.into(),
        code: format!("Key{}", key.to_uppercase()),
        text: Some(key.into()),
        modifiers: none(),
    }
}

fn taken(flow: &mut BrowserInteraction) -> Vec<BrowserInputEvent> {
    flow.take()
        .map(|(node, events)| {
            assert_eq!(node, "b1");
            events
        })
        .unwrap_or_default()
}

#[test]
fn a_live_preview_page_takes_the_pointer_without_a_mode() {
    let _lock = lock_for_tests();
    let state = previewing();
    painted(true);
    let mut flow = BrowserInteraction::new();
    assert!(flow.preview_over_page(&state, 450.0, 350.0));
    assert!(!flow.preview_over_page(&state, 100.0, 100.0));
    assert!(flow.preview_pointer_move(&state, 450.0, 350.0, none()));
    assert!(flow.preview_pointer_button(&state, true, MouseButton::Left, 450.0, 350.0, none(), 0));
    // A drag out of the box stays with the page until the release.
    assert!(flow.preview_pointer_move(&state, 900.0, 700.0, none()));
    assert!(flow.preview_pointer_button(
        &state,
        false,
        MouseButton::Left,
        900.0,
        700.0,
        none(),
        10
    ));
    let events = taken(&mut flow);
    assert!(matches!(
        events.first(),
        Some(BrowserInputEvent::MouseMove { x, y, .. }) if (*x, *y) == (50.0, 50.0)
    ));
    assert!(matches!(
        events.last(),
        Some(BrowserInputEvent::MouseUp { x, y, .. }) if (*x, *y) == (500.0, 400.0)
    ));
    assert!(
        !flow.preview_pointer_move(&state, 900.0, 700.0, none()),
        "released"
    );
}

#[test]
fn a_wheel_scrolls_the_page_one_screen_px_per_px() {
    let _lock = lock_for_tests();
    let state = previewing();
    painted(true);
    let mut flow = BrowserInteraction::new();
    assert!(flow.preview_wheel(&state, 450.0, 350.0, 0.0, -100.0, none()));
    assert!(matches!(
        taken(&mut flow).as_slice(),
        [BrowserInputEvent::Wheel { delta_y, .. }] if *delta_y == 100.0
    ));
    assert!(
        !flow.preview_wheel(&state, 100.0, 100.0, 0.0, -100.0, none()),
        "off the page"
    );
}

#[test]
fn a_press_gives_the_page_the_keys_until_escape_or_a_press_elsewhere() {
    let _lock = lock_for_tests();
    let state = previewing();
    painted(true);
    let mut flow = BrowserInteraction::new();
    assert_eq!(flow.preview_key(&state, key("a"), true), KeyOutcome::Editor);
    flow.preview_pointer_button(&state, true, MouseButton::Left, 450.0, 350.0, none(), 0);
    flow.preview_pointer_button(&state, false, MouseButton::Left, 450.0, 350.0, none(), 5);
    assert_eq!(flow.preview_focus(), Some("b1"));
    assert_eq!(flow.preview_key(&state, key("a"), true), KeyOutcome::Page);
    assert!(flow.preview_insert_text(&state, "é"));
    assert_eq!(
        flow.preview_key(&state, key("Escape"), true),
        KeyOutcome::Left
    );
    assert_eq!(flow.preview_key(&state, key("a"), true), KeyOutcome::Editor);
    flow.preview_pointer_button(&state, true, MouseButton::Left, 450.0, 350.0, none(), 900);
    flow.preview_pointer_button(&state, false, MouseButton::Left, 450.0, 350.0, none(), 905);
    assert!(!flow.preview_pointer_button(
        &state,
        true,
        MouseButton::Left,
        10.0,
        10.0,
        none(),
        2000
    ));
    assert_eq!(
        flow.preview_focus(),
        None,
        "a press elsewhere takes the keys back"
    );
}

#[test]
fn a_still_page_or_live_pages_off_take_no_input() {
    let _lock = lock_for_tests();
    let mut state = previewing();
    painted(false);
    let mut flow = BrowserInteraction::new();
    assert!(!flow.preview_pointer_button(&state, true, MouseButton::Left, 450.0, 350.0, none(), 0));
    painted(true);
    state.editor_ui.browser_live.set_live_pages(false);
    assert!(!flow.preview_pointer_move(&state, 450.0, 350.0, none()));
}

#[test]
fn closing_the_preview_releases_the_page() {
    let _lock = lock_for_tests();
    let mut state = previewing();
    painted(true);
    let mut flow = BrowserInteraction::new();
    flow.preview_pointer_button(&state, true, MouseButton::Left, 450.0, 350.0, none(), 0);
    taken(&mut flow);
    assert!(!flow.sync_preview(&state), "still previewing");
    state.editor_ui.preview.mode = false;
    assert!(flow.sync_preview(&state));
    assert!(matches!(
        taken(&mut flow).as_slice(),
        [BrowserInputEvent::MouseUp { .. }]
    ));
    assert_eq!(flow.preview_focus(), None);
    assert!(browser_preview::page("b1").is_none());
    assert!(!flow.sync_preview(&state));
}

#[test]
fn the_editor_canvas_keeps_its_own_drag() {
    let mut state = previewing();
    state.editor_ui.preview.mode = false;
    let mut flow = BrowserInteraction::new();
    // A press held by the canvas interaction mode, not by the preview.
    flow.held = MouseButton::Left.bit();
    flow.node = Some("b1".into());
    assert!(!flow.sync_preview(&state));
    assert!(flow.captures_pointer(), "the canvas drag goes on");
}

#[test]
fn a_page_gone_mid_press_still_gets_its_release() {
    let _lock = lock_for_tests();
    let state = previewing();
    painted(true);
    let mut flow = BrowserInteraction::new();
    flow.preview_pointer_button(&state, true, MouseButton::Left, 450.0, 350.0, none(), 0);
    taken(&mut flow);
    browser_preview::clear();
    assert!(flow.preview_pointer_button(&state, false, MouseButton::Left, 450.0, 350.0, none(), 5));
    assert!(matches!(
        taken(&mut flow).as_slice(),
        [BrowserInputEvent::MouseUp { x, y, .. }] if (*x, *y) == (50.0, 50.0)
    ));
    assert!(!flow.captures_pointer());
}
