//! The desktop host with a scripted renderer: requests reach the worker
//! threads and the snapshots come back into the canvas caches. The
//! scheduling rules themselves are tested on the shared queue
//! (`de_editor_ui::widgets::browser_render_queue`).

use std::sync::Arc;
use std::time::{Duration, Instant};

use de_browser_render::{BrowserRenderError, RenderSpec};
use de_editor_core::browser_live::BrowserPageStatus;
use de_editor_core::EditorState;
use de_editor_ui::widgets::browser_textures;
use de_editor_ui::widgets::canvas_viewport_image::has_cached_image_bytes;

use super::BrowserRenderHost;

const PNG: &[u8] = b"\x89PNG\r\n\x1a\nfake";

/// The paint registry's visible set is process-wide: one test at a time.
fn serial() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

fn state_with_browser(id: &str) -> EditorState {
    let doc = serde_json::from_value(serde_json::json!({
        "version": "1.0",
        "children": [{ "type": "browser", "id": id, "width": 390, "height": 844,
                       "url": "https://example.com", "deviceId": "phone" }]
    }))
    .expect("document");
    let mut state = EditorState::new();
    state.replace_document(doc);
    state
}

/// Paint frames showing `id` and pump until nothing is pending.
fn run(host: &mut BrowserRenderHost, state: &mut EditorState, id: &str) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        browser_textures::begin_frame(false);
        browser_textures::note_visible(id);
        host.pump(state);
        if !host.is_pending() {
            return;
        }
        assert!(Instant::now() < deadline, "renders never landed");
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn a_worker_renders_the_visible_page_into_the_canvas_cache() {
    let _serial = serial();
    let id = "rh-visible";
    let mut state = state_with_browser(id);
    let seen = Arc::new(std::sync::Mutex::new(Vec::<RenderSpec>::new()));
    let log = seen.clone();
    let mut host = BrowserRenderHost::with_script(Arc::new(move |spec| {
        log.lock().unwrap().push(spec.clone());
        Ok(PNG.to_vec())
    }));
    run(&mut host, &mut state, id);
    let (_, texture) = browser_textures::ready(id).expect("snapshot ready");
    assert!(has_cached_image_bytes(texture));
    assert_eq!(
        state.editor_ui.browser_live.status(id),
        Some(BrowserPageStatus::Ready)
    );
    let specs = seen.lock().unwrap();
    assert_eq!(specs.len(), 1);
    assert_eq!((specs[0].layout_width, specs[0].layout_height), (390, 844));
    assert!(specs[0].mobile);
}

#[test]
fn a_missing_browser_shows_as_no_browser() {
    let _serial = serial();
    let id = "rh-no-browser";
    let mut state = state_with_browser(id);
    let mut host = BrowserRenderHost::with_script(Arc::new(|_| Err(BrowserRenderError::NoBrowser)));
    run(&mut host, &mut state, id);
    assert_eq!(
        state.editor_ui.browser_live.status(id),
        Some(BrowserPageStatus::NoBrowser)
    );
    assert!(browser_textures::ready(id).is_none());
}
