//! Present from any page (#157): Cmd/Ctrl+Enter presents a document that
//! is not tagged as a deck, toggles back out, and switches over from the
//! interactive preview.

use super::super::WidgetHostNative;
use de_editor_core::EditorState;
use std::sync::{LazyLock, Mutex, MutexGuard};

static TEST_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

fn test_lock() -> MutexGuard<'static, ()> {
    TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner())
}

/// Two app screens with no scenario tag: an ordinary design, not a deck.
const TWO_SCREENS: &str = r##"{
    "version": "1.0.0",
    "children": [
        { "type": "frame", "id": "home", "x": 0, "y": 0,
          "width": 390, "height": 844,
          "fill": [{"type":"solid","color":"#ffffff"}], "children": [] },
        { "type": "rectangle", "id": "note", "x": 0, "y": 900,
          "width": 40, "height": 40 },
        { "type": "frame", "id": "detail", "x": 500, "y": 0,
          "width": 390, "height": 844,
          "fill": [{"type":"solid","color":"#eeeeee"}], "children": [] }
    ]
}"##;

fn host_with(source: &str) -> WidgetHostNative {
    let document = jian_ops_schema::load_str(source)
        .expect("parse present fixture")
        .value;
    let mut host = WidgetHostNative::new();
    host.install_imported_state(EditorState::from_document(document));
    host
}

fn board_on_screen(host: &WidgetHostNative) -> Option<String> {
    host.editor_state
        .preview_slideshow()
        .and_then(|slideshow| slideshow.current_board())
        .map(str::to_string)
}

#[test]
fn present_runs_any_page_as_slides_and_toggles_off() {
    let _guard = test_lock();
    let mut host = host_with(TWO_SCREENS);

    assert!(host.toggle_present((1200.0, 800.0)));
    assert!(host.preview_slideshow_active());
    assert_eq!(board_on_screen(&host).as_deref(), Some("home"));
    assert_eq!(
        host.editor_state.preview_slideshow().map(|s| s.len()),
        Some(2),
        "only top-level frames are slides"
    );
    assert!(host.preview_slideshow_step(1));
    assert_eq!(board_on_screen(&host).as_deref(), Some("detail"));

    assert!(host.toggle_present((1200.0, 800.0)));
    assert!(!host.preview_active());
    assert!(!host.editor_state.editor_ui.preview.present);

    // Plain Preview of the same document is still the interactive one.
    assert!(host.enter_preview((1200.0, 800.0)));
    assert!(!host.preview_slideshow_active());
}

#[test]
fn present_switches_over_from_the_interactive_preview() {
    let _guard = test_lock();
    let mut host = host_with(TWO_SCREENS);
    assert!(host.enter_preview((1200.0, 800.0)));
    assert!(!host.preview_slideshow_active());

    assert!(host.toggle_present((1200.0, 800.0)));
    assert!(host.preview_active());
    assert!(host.preview_slideshow_active());
}

#[test]
fn a_page_without_frames_does_not_present() {
    let _guard = test_lock();
    let mut host = host_with(
        r#"{"version":"1.0.0","children":[
            {"type":"rectangle","id":"r","width":10,"height":10}]}"#,
    );
    assert!(!host.toggle_present((1200.0, 800.0)));
    assert!(!host.preview_active());
    assert!(!host.editor_state.editor_ui.preview.present);
}
