//! Native browser-bar and section-scroll glue (#153).

use super::WidgetHostNative;
use de_editor_core::editor_ui_state::groups::PreviewBrowserControl;
use de_editor_core::{EditorState, PreviewDeviceKind};
use de_editor_ui::widgets::PreviewBrowserBar;
use de_editor_ui::Rect;
use std::sync::{LazyLock, Mutex, MutexGuard};

static TEST_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

fn test_lock() -> MutexGuard<'static, ()> {
    TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner())
}

const VIEWPORT: (f32, f32) = (1400.0, 900.0);

const SITE_JSON: &str = r##"{
    "version": "1.1", "formatVersion": "1.1", "id": "site",
    "app": { "name": "site", "version": "1", "id": "site" },
    "children": [
        { "type": "frame", "id": "home", "name": "Home", "screen": "/",
          "x": 0, "y": 0, "width": 1280, "height": 2400,
          "children": [
              { "type": "text", "id": "features-link", "x": 200, "y": 10,
                "width": 100, "height": 30, "content": "Features",
                "events": { "onTap": [ { "scroll_to": { "target": "features" } } ] } },
              { "type": "frame", "id": "features", "name": "Features",
                "x": 0, "y": 700, "width": 1280, "height": 400 }
          ] },
        { "type": "frame", "id": "pricing", "name": "Pricing", "screen": "/pricing",
          "x": 1400, "y": 0, "width": 1280, "height": 900 }
    ]
}"##;

fn site_host() -> WidgetHostNative {
    let doc: jian_ops_schema::PenDocument = serde_json::from_str(SITE_JSON).unwrap();
    let mut host = WidgetHostNative::new();
    host.install_imported_state(EditorState::from_document(doc));
    host.last_viewport_w = VIEWPORT.0;
    host.last_viewport_h = VIEWPORT.1;
    assert!(host.enter_preview(VIEWPORT));
    host.preview_mode_transition = None;
    assert!(host.preview.as_ref().unwrap().is_app_mode());
    host
}

fn navigate(host: &mut WidgetHostNative, path: &str, now: u64) {
    let session = host.preview.as_mut().unwrap();
    session.navigate_to_screen(path);
    assert!(session.reconcile(now).switched);
}

fn centre(r: Rect) -> (f32, f32) {
    (r.origin.x + r.size.x / 2.0, r.origin.y + r.size.y / 2.0)
}

#[test]
fn history_step_goes_back_and_reports_a_no_op_at_the_oldest_entry() {
    let _guard = test_lock();
    let mut host = site_host();
    assert!(!host.preview_history_step(false), "nothing to go back to");
    navigate(&mut host, "/pricing", 1_000);
    assert!(host.preview_history_step(false));
    assert!(host.preview.as_mut().unwrap().reconcile(10_000).switched);
    assert_eq!(host.preview.as_ref().unwrap().current_path_for_test(), "/");
    assert!(host.preview_history_step(true), "forward is available now");
}

#[test]
fn back_button_press_and_release_navigates() {
    let _guard = test_lock();
    let mut host = site_host();
    navigate(&mut host, "/pricing", 1_000);
    let canvas = host.preview_canvas_rect(VIEWPORT.0, VIEWPORT.1);
    let (x, y) = centre(PreviewBrowserBar::button_rect(
        canvas,
        PreviewBrowserControl::Back,
    ));
    assert!(host.over_preview_browser_bar(x, y, VIEWPORT.0, VIEWPORT.1));
    assert!(host.preview_browser_press(x, y, VIEWPORT.0, VIEWPORT.1));
    assert!(host.preview_browser_release());
    assert!(host.preview.as_mut().unwrap().reconcile(10_000).switched);
    assert_eq!(host.preview.as_ref().unwrap().current_path_for_test(), "/");
    assert_eq!(host.editor_state.editor_ui.preview.browser_pressed, None);
}

#[test]
fn a_press_on_the_address_is_swallowed_without_navigating() {
    let _guard = test_lock();
    let mut host = site_host();
    let canvas = host.preview_canvas_rect(VIEWPORT.0, VIEWPORT.1);
    let (x, y) = centre(PreviewBrowserBar::address_rect(canvas));
    assert!(host.preview_browser_press(x, y, VIEWPORT.0, VIEWPORT.1));
    assert!(!host.preview_browser_release(), "no button was armed");
    assert_eq!(host.preview.as_ref().unwrap().current_path_for_test(), "/");
}

#[test]
fn section_link_scrolls_the_desktop_frame() {
    let _guard = test_lock();
    let mut host = site_host();
    host.set_preview_device(PreviewDeviceKind::Desktop, VIEWPORT.0, VIEWPORT.1);
    assert_eq!(host.preview_scroll_y, 0.0);
    host.preview.as_mut().unwrap().dispatch_tap(250.0, 25.0);
    host.apply_preview_page_scroll_request(VIEWPORT.0, VIEWPORT.1);
    assert!(
        (host.preview_scroll_y - 700.0).abs() < 0.5,
        "the section's top meets the frame top, got {}",
        host.preview_scroll_y
    );
}

#[test]
fn section_link_pans_the_canvas_viewport() {
    let _guard = test_lock();
    let mut host = site_host();
    host.set_preview_device(PreviewDeviceKind::Canvas, VIEWPORT.0, VIEWPORT.1);
    host.preview.as_mut().unwrap().dispatch_tap(250.0, 25.0);
    host.apply_preview_page_scroll_request(VIEWPORT.0, VIEWPORT.1);
    let viewport = host.editor_state.viewport;
    let section_top_on_canvas = viewport.pan_y + 700.0 * viewport.zoom;
    assert!(
        (section_top_on_canvas - 24.0).abs() < 0.5,
        "section top lands near the canvas top, got {section_top_on_canvas}"
    );
}
