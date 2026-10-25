//! The render host against a scripted renderer: what gets rendered, when,
//! and what lands in the canvas caches. Node ids are unique per test (the
//! paint registry is process-wide).

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

fn state_with_browser(id: &str, url: &str) -> EditorState {
    let doc = serde_json::from_value(serde_json::json!({
        "version": "1.0",
        "children": [{ "type": "browser", "id": id, "width": 390, "height": 844,
                       "url": url, "deviceId": "phone" }]
    }))
    .expect("document");
    let mut state = EditorState::new();
    state.replace_document(doc);
    state
}

/// Paint a canvas frame showing `ids`, then pump once.
fn frame(host: &mut BrowserRenderHost, state: &mut EditorState, ids: &[&str]) -> bool {
    browser_textures::begin_frame();
    for id in ids {
        browser_textures::note_visible(id);
    }
    host.pump(state)
}

fn pump_until_idle(host: &mut BrowserRenderHost, state: &mut EditorState) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !host.in_flight.is_empty() {
        host.pump(state);
        assert!(Instant::now() < deadline, "renders never landed");
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn a_visible_browser_renders_once_and_its_snapshot_lands_in_the_cache() {
    let _serial = serial();
    let id = "rh-visible";
    let mut state = state_with_browser(id, "https://example.com");
    let seen = Arc::new(std::sync::Mutex::new(Vec::<RenderSpec>::new()));
    let log = seen.clone();
    let mut host = BrowserRenderHost::with_script(Arc::new(move |spec| {
        log.lock().unwrap().push(spec.clone());
        Ok(PNG.to_vec())
    }));
    assert!(frame(&mut host, &mut state, &[id]), "dispatch is a change");
    assert_eq!(
        state.editor_ui.browser_live.status(id),
        Some(BrowserPageStatus::Loading)
    );
    pump_until_idle(&mut host, &mut state);
    let (key, texture) = browser_textures::ready(id).expect("snapshot ready");
    assert!(has_cached_image_bytes(texture));
    assert_eq!(
        state.editor_ui.browser_live.status(id),
        Some(BrowserPageStatus::Ready)
    );
    // Same request again: nothing new to render.
    frame(&mut host, &mut state, &[id]);
    assert!(!host.is_pending());
    assert_eq!(browser_textures::ready(id).map(|(k, _)| k), Some(key));
    let specs = seen.lock().unwrap();
    assert_eq!(specs.len(), 1);
    assert_eq!((specs[0].layout_width, specs[0].layout_height), (390, 844));
    assert!(specs[0].mobile);
}

#[test]
fn off_screen_and_held_pages_never_render() {
    let _serial = serial();
    let id = "rh-held";
    let mut state = state_with_browser(id, "https://example.com");
    let mut host = BrowserRenderHost::with_script(Arc::new(|_| panic!("must not render")));
    frame(&mut host, &mut state, &[]);
    assert!(!host.is_pending(), "not painted, not requested");
    state.hold_browser_pages_if_any();
    frame(&mut host, &mut state, &[id]);
    assert!(!host.is_pending(), "held pages wait for Load pages");
}

#[test]
fn a_change_waits_out_the_debounce_and_a_failure_is_not_retried() {
    let _serial = serial();
    let id = "rh-change";
    let mut state = state_with_browser(id, "https://example.com");
    let fail = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let failing = fail.clone();
    let mut host = BrowserRenderHost::with_script(Arc::new(move |_| {
        if failing.load(std::sync::atomic::Ordering::SeqCst) {
            Err(BrowserRenderError::Navigation("refused".into()))
        } else {
            Ok(PNG.to_vec())
        }
    }));
    frame(&mut host, &mut state, &[id]);
    pump_until_idle(&mut host, &mut state);
    state.set_single_selection(de_editor_core::NodeId::new(id));
    assert!(state.set_selected_browser_url("example.org"));
    frame(&mut host, &mut state, &[id]);
    assert!(host.in_flight.is_empty(), "a change waits for the debounce");
    assert!(host.is_pending());
    fail.store(true, std::sync::atomic::Ordering::SeqCst);
    std::thread::sleep(super::DEBOUNCE + Duration::from_millis(20));
    frame(&mut host, &mut state, &[id]);
    pump_until_idle(&mut host, &mut state);
    assert_eq!(
        state.editor_ui.browser_live.status(id),
        Some(BrowserPageStatus::Failed)
    );
    frame(&mut host, &mut state, &[id]);
    assert!(!host.is_pending(), "the failed key is not retried");
    assert!(state.reload_selected_browser());
    frame(&mut host, &mut state, &[id]);
    assert!(!host.in_flight.is_empty(), "Reload renders at once");
}

/// Perf guard (#20): a quiet frame with many visible browsers must cost
/// next to nothing — no document walk, no allocation per node. Prints the
/// measured per-pump time (run with `--nocapture` to read it).
#[test]
fn a_quiet_frame_with_fifty_visible_browsers_is_cheap() {
    let _serial = serial();
    let ids: Vec<String> = (0..50).map(|i| format!("rh-perf-{i}")).collect();
    let children: Vec<serde_json::Value> = ids
        .iter()
        .enumerate()
        .map(|(i, id)| {
            serde_json::json!({ "type": "browser", "id": id, "x": i * 400, "y": 0,
                                "width": 390, "height": 844, "url": "https://example.com" })
        })
        .collect();
    let doc = serde_json::from_value(serde_json::json!({ "version": "1.0", "children": children }))
        .expect("document");
    let mut state = EditorState::new();
    state.replace_document(doc);
    let mut host = BrowserRenderHost::with_script(Arc::new(|_| Ok(PNG.to_vec())));
    let refs: Vec<&str> = ids.iter().map(String::as_str).collect();
    // Render all fifty once.
    let deadline = Instant::now() + Duration::from_secs(10);
    while ids.iter().any(|id| browser_textures::ready(id).is_none()) {
        frame(&mut host, &mut state, &refs);
        assert!(Instant::now() < deadline, "snapshots never landed");
        std::thread::sleep(Duration::from_millis(1));
    }
    host.pump(&mut state);
    // Quiet frames: the canvas repaints the same fifty nodes, nothing changed.
    const FRAMES: u32 = 2000;
    let started = Instant::now();
    for _ in 0..FRAMES {
        frame(&mut host, &mut state, &refs);
    }
    let per_frame = started.elapsed() / FRAMES;
    eprintln!("[perf] quiet pump + 50 notes: {per_frame:?} per frame");
    assert!(!host.is_pending());
    assert!(
        per_frame < Duration::from_millis(1),
        "a quiet frame took {per_frame:?}"
    );
}
