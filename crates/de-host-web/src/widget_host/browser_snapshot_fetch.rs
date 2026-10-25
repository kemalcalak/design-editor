//! Live browser pages in the web editor (#20). A browser tab cannot render
//! another site into the canvas, so the shared `BrowserRenderQueue` hands
//! its requests to the daemon's `POST /api/browser/snapshot`, which renders
//! with headless Chrome. Called once per repaint with the host borrowed;
//! replies land in a queue the next repaint drains, woken through
//! `repaint_coalescer` (no per-paint network and no polling). Without a
//! daemon, or a browser on it, the node keeps its placeholder.

use std::cell::RefCell;
use std::collections::VecDeque;

use base64::Engine as _;
use de_editor_core::browser_live::BrowserRenderRequest;
use de_editor_ui::widgets::browser_render_queue::{BrowserRenderOutcome, BrowserRenderQueue};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use super::WidgetHost;

/// Snapshots requested from the daemon at once.
const MAX_IN_FLIGHT: usize = 2;
/// A page may load for 15 s on the daemon; allow for the render around it.
const SNAPSHOT_TIMEOUT_MS: u32 = 45_000;

thread_local! {
    static QUEUE: RefCell<BrowserRenderQueue> = RefCell::new(BrowserRenderQueue::new(MAX_IN_FLIGHT));
    /// Daemon replies waiting for the next repaint: (node id, key, outcome).
    static LANDED: RefCell<VecDeque<(String, u64, BrowserRenderOutcome)>> =
        const { RefCell::new(VecDeque::new()) };
}

/// Per-repaint step, after the paint that noted the visible browser
/// nodes: land finished snapshots, then ask the daemon for the visible
/// pages that need one. Returns whether a status or snapshot changed (the
/// caller repaints).
pub(crate) fn pump(host: &mut WidgetHost) -> bool {
    let now_ms = js_sys::Date::now();
    let landed: Vec<_> = LANDED.with(|q| q.borrow_mut().drain(..).collect());
    let (changed, requests, waiting) = QUEUE.with(|queue| {
        let mut queue = queue.borrow_mut();
        let state = host.editor_state_mut();
        for (node_id, key, outcome) in &landed {
            queue.finish(state, node_id, *key, outcome.clone());
        }
        let (status_changed, requests) = queue.pump(state, now_ms);
        (
            status_changed || !landed.is_empty(),
            requests,
            queue.is_waiting(),
        )
    });
    for request in requests {
        if !request_snapshot(&request) {
            LANDED.with(|q| {
                q.borrow_mut().push_back((
                    request.node_id,
                    request.key,
                    BrowserRenderOutcome::Failed,
                ));
            });
            crate::repaint_coalescer::request();
        }
    }
    if changed {
        host.mark_editor_state_dirty();
    }
    if waiting {
        // A debounced change comes due without another input event; a
        // request in flight wakes the next repaint when it lands.
        crate::repaint_coalescer::request_after(
            de_editor_ui::widgets::browser_render_queue::DEBOUNCE_MS as u64,
        );
    }
    changed
}

/// POST one request to the daemon. False when the request never left.
fn request_snapshot(request: &BrowserRenderRequest) -> bool {
    let Ok(body) = serde_json::to_string(&request.snapshot_spec()) else {
        return false;
    };
    let url = crate::daemon_base::daemon_url("/api/browser/snapshot");
    let Ok(xhr) = web_sys::XmlHttpRequest::new() else {
        return false;
    };
    if xhr.open_with_async("POST", &url, true).is_err() {
        return false;
    }
    crate::live_sync::attach_daemon_headers(&xhr, &url);
    xhr.set_timeout(SNAPSHOT_TIMEOUT_MS);
    let _ = xhr.set_request_header("Content-Type", "application/json");
    let (node_id, key) = (request.node_id.clone(), request.key);
    let reply = xhr.clone();
    let onloadend = Closure::<dyn FnMut()>::once_into_js(move || {
        let status = reply.status().unwrap_or(0);
        let text = reply.response_text().ok().flatten().unwrap_or_default();
        let outcome = outcome_of(status, &text);
        LANDED.with(|q| q.borrow_mut().push_back((node_id, key, outcome)));
        crate::repaint_coalescer::request();
    });
    xhr.set_onloadend(Some(onloadend.unchecked_ref()));
    xhr.send_with_opt_str(Some(&body)).is_ok()
}

/// The daemon's answer as a render outcome.
pub(crate) fn outcome_of(status: u16, text: &str) -> BrowserRenderOutcome {
    let reply: serde_json::Value = serde_json::from_str(text).unwrap_or_default();
    let png = reply
        .get("dataBase64")
        .and_then(serde_json::Value::as_str)
        .and_then(|data| base64::engine::general_purpose::STANDARD.decode(data).ok());
    match (
        status,
        png,
        reply.get("reason").and_then(serde_json::Value::as_str),
    ) {
        (200, Some(png), _) => BrowserRenderOutcome::Ready(png),
        (_, _, Some("blocked")) => BrowserRenderOutcome::Blocked,
        (_, _, Some("noBrowser")) => BrowserRenderOutcome::NoBrowser,
        _ => BrowserRenderOutcome::Failed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn daemon_replies_map_to_outcomes() {
        let ok = format!(
            r#"{{"ok":true,"dataBase64":"{}"}}"#,
            base64::engine::general_purpose::STANDARD.encode(b"PNG")
        );
        assert_eq!(
            outcome_of(200, &ok),
            BrowserRenderOutcome::Ready(b"PNG".to_vec())
        );
        assert_eq!(
            outcome_of(403, r#"{"ok":false,"reason":"blocked"}"#),
            BrowserRenderOutcome::Blocked
        );
        assert_eq!(
            outcome_of(503, r#"{"ok":false,"reason":"noBrowser"}"#),
            BrowserRenderOutcome::NoBrowser
        );
        assert_eq!(outcome_of(0, ""), BrowserRenderOutcome::Failed);
        assert_eq!(outcome_of(200, "{}"), BrowserRenderOutcome::Failed);
    }
}
