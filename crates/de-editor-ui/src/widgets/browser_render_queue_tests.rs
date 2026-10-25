//! The shared scheduling, driven by hand: what is requested, when, and
//! what lands in the canvas caches. Node ids are unique per test and the
//! tests hold the paint registry's lock (its visible set is process-wide).

use std::time::{Duration, Instant};

use de_editor_core::browser_live::BrowserPageStatus;
use de_editor_core::EditorState;

use super::{BrowserRenderOutcome, BrowserRenderQueue, DEBOUNCE_MS};
use crate::widgets::browser_textures::{self, lock_for_tests};
use crate::widgets::canvas_viewport_image::has_cached_image_bytes;

const PNG: &[u8] = b"\x89PNG\r\n\x1a\nfake";

fn state_with(children: serde_json::Value) -> EditorState {
    let doc = serde_json::from_value(serde_json::json!({ "version": "1.0", "children": children }))
        .expect("document");
    let mut state = EditorState::new();
    state.replace_document(doc);
    state
}

fn phone(id: &str) -> serde_json::Value {
    serde_json::json!({ "type": "browser", "id": id, "width": 390, "height": 844,
                        "url": "https://example.com", "deviceId": "phone" })
}

/// Paint a canvas frame showing `ids`, then pump at `now`.
fn frame(
    queue: &mut BrowserRenderQueue,
    state: &mut EditorState,
    ids: &[&str],
    now: f64,
) -> Vec<de_editor_core::browser_live::BrowserRenderRequest> {
    browser_textures::begin_frame(false);
    for id in ids {
        browser_textures::note_visible(id);
    }
    queue.pump(state, now).1
}

#[test]
fn a_visible_browser_is_requested_once_and_its_snapshot_lands() {
    let _lock = lock_for_tests();
    let id = "rq-visible";
    let mut state = state_with(serde_json::json!([phone(id)]));
    let mut queue = BrowserRenderQueue::new(2);
    let requests = frame(&mut queue, &mut state, &[id], 0.0);
    assert_eq!(requests.len(), 1);
    let request = &requests[0];
    assert_eq!((request.layout_width, request.layout_height), (390, 844));
    assert_eq!(
        state.editor_ui.browser_live.status(id),
        Some(BrowserPageStatus::Loading)
    );
    assert!(
        frame(&mut queue, &mut state, &[id], 1.0).is_empty(),
        "in flight"
    );
    queue.finish(
        &mut state,
        id,
        request.key,
        BrowserRenderOutcome::Ready(PNG.to_vec()),
    );
    let (key, texture) = browser_textures::ready(id).expect("ready");
    assert_eq!(key, request.key);
    assert!(has_cached_image_bytes(texture));
    assert!(frame(&mut queue, &mut state, &[id], 2.0).is_empty());
    assert!(!queue.is_pending());
}

#[test]
fn off_screen_and_held_pages_are_never_requested() {
    let _lock = lock_for_tests();
    let id = "rq-held";
    let mut state = state_with(serde_json::json!([phone(id)]));
    let mut queue = BrowserRenderQueue::new(2);
    assert!(frame(&mut queue, &mut state, &[], 0.0).is_empty());
    state.hold_browser_pages_if_any();
    assert!(frame(&mut queue, &mut state, &[id], 1.0).is_empty());
    assert!(!queue.is_pending());
}

#[test]
fn a_change_waits_out_the_debounce_failures_stick_and_reload_is_immediate() {
    let _lock = lock_for_tests();
    let id = "rq-change";
    let mut state = state_with(serde_json::json!([phone(id)]));
    let mut queue = BrowserRenderQueue::new(2);
    let first = frame(&mut queue, &mut state, &[id], 0.0).remove(0);
    queue.finish(
        &mut state,
        id,
        first.key,
        BrowserRenderOutcome::Ready(PNG.to_vec()),
    );
    state.set_single_selection(de_editor_core::NodeId::new(id));
    assert!(state.set_selected_browser_url("example.org"));
    assert!(
        frame(&mut queue, &mut state, &[id], 10.0).is_empty(),
        "debounced"
    );
    assert!(queue.is_pending());
    let changed = frame(&mut queue, &mut state, &[id], 10.0 + DEBOUNCE_MS).remove(0);
    assert_ne!(changed.key, first.key);
    queue.finish(&mut state, id, changed.key, BrowserRenderOutcome::Blocked);
    assert_eq!(
        state.editor_ui.browser_live.status(id),
        Some(BrowserPageStatus::Blocked)
    );
    assert_eq!(
        browser_textures::ready(id).map(|(key, _)| key),
        Some(first.key),
        "the old snapshot stays up"
    );
    assert!(
        frame(&mut queue, &mut state, &[id], 1000.0).is_empty(),
        "not retried"
    );
    assert!(state.reload_selected_browser());
    assert_eq!(
        frame(&mut queue, &mut state, &[id], 1001.0).len(),
        1,
        "Reload is immediate"
    );
}

#[test]
fn at_most_the_cap_runs_at_once() {
    let _lock = lock_for_tests();
    let ids = ["rq-cap-a", "rq-cap-b", "rq-cap-c"];
    let mut state = state_with(serde_json::json!(ids
        .iter()
        .map(|id| phone(id))
        .collect::<Vec<_>>()));
    let mut queue = BrowserRenderQueue::new(2);
    let first = frame(&mut queue, &mut state, &ids, 0.0);
    assert_eq!(first.len(), 2);
    assert_eq!(queue.in_flight(), 2);
    queue.finish(
        &mut state,
        &first[0].node_id,
        first[0].key,
        BrowserRenderOutcome::Failed,
    );
    let next = frame(&mut queue, &mut state, &ids, 1.0);
    assert_eq!(next.len(), 1);
    assert_eq!(next[0].node_id, "rq-cap-c");
}

/// Perf guard (#20): a quiet frame with many visible browsers must cost
/// next to nothing: no document walk, no allocation per node. Prints the
/// measured time (`--nocapture`).
#[test]
fn a_quiet_frame_with_fifty_visible_browsers_is_cheap() {
    let _lock = lock_for_tests();
    let ids: Vec<String> = (0..50).map(|i| format!("rq-perf-{i}")).collect();
    let children: Vec<serde_json::Value> = ids.iter().map(|id| phone(id)).collect();
    let mut state = state_with(serde_json::json!(children));
    let refs: Vec<&str> = ids.iter().map(String::as_str).collect();
    let mut queue = BrowserRenderQueue::new(50);
    for request in frame(&mut queue, &mut state, &refs, 0.0) {
        let id = request.node_id.clone();
        queue.finish(
            &mut state,
            &id,
            request.key,
            BrowserRenderOutcome::Ready(PNG.to_vec()),
        );
    }
    frame(&mut queue, &mut state, &refs, 1.0);
    const FRAMES: u32 = 2000;
    let started = Instant::now();
    for i in 0..FRAMES {
        assert!(frame(&mut queue, &mut state, &refs, 2.0 + f64::from(i)).is_empty());
    }
    let per_frame = started.elapsed() / FRAMES;
    eprintln!("[perf] quiet pump + 50 notes: {per_frame:?} per frame");
    assert!(
        per_frame < Duration::from_millis(1),
        "a quiet frame took {per_frame:?}"
    );
}

#[test]
fn clearing_the_address_drops_the_old_page() {
    let _lock = lock_for_tests();
    let id = "rq-cleared";
    let mut state = state_with(serde_json::json!([phone(id)]));
    let mut queue = BrowserRenderQueue::new(2);
    let first = frame(&mut queue, &mut state, &[id], 0.0).remove(0);
    queue.finish(
        &mut state,
        id,
        first.key,
        BrowserRenderOutcome::Ready(PNG.to_vec()),
    );
    state.set_single_selection(de_editor_core::NodeId::new(id));
    assert!(state.set_selected_browser_url(""));
    browser_textures::begin_frame(false);
    browser_textures::note_visible(id);
    let (changed, requests) = queue.pump(&mut state, 1.0);
    assert!(changed && requests.is_empty());
    assert!(browser_textures::ready(id).is_none());
}
