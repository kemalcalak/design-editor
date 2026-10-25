//! `POST /api/browser/snapshot` on the serve-web daemon: the web editor's
//! live browser pages (#20). The browser tab cannot render another site
//! itself, so it asks the daemon, which renders with its own headless
//! Chrome (`de_browser_render`, the shared browser) and answers with the
//! PNG (base64 JSON, like the export routes). The request body is
//! `de_editor_core::browser_live::BrowserSnapshotSpec`, the shape the web
//! host sends; addresses go through `browser_url_policy`.
//!
//! The route runs on its connection thread without the state lock; at most
//! [`MAX_RENDERS`] render at once, and a request that waits longer than
//! [`SLOT_WAIT`] for a turn is answered "busy" instead of holding its
//! connection.

use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

use base64::Engine as _;
use de_browser_render::{BrowserPool, BrowserRenderError, RenderSpec};
use de_editor_core::browser_live::BrowserSnapshotSpec;
use serde_json::json;

/// Renders at once; more requests wait.
const MAX_RENDERS: usize = 2;
/// Longest wait for a render slot.
const SLOT_WAIT: Duration = Duration::from_secs(30);
const MAX_LAYOUT_EDGE: u32 = 4096;
const MAX_SCALE: f64 = 4.0;

/// Why a snapshot request was refused before rendering. Plain enum with a
/// hand-written `Display`, like the daemon's other errors.
#[derive(Debug)]
pub(crate) enum SnapshotRequestError {
    Body(String),
    Url,
    Size,
    Scale,
}

impl std::fmt::Display for SnapshotRequestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Body(detail) => write!(f, "body must be the snapshot JSON: {detail}"),
            Self::Url => f.write_str("url must be an http, https or file address"),
            Self::Size => write!(
                f,
                "layoutWidth and layoutHeight must be 1..={MAX_LAYOUT_EDGE}"
            ),
            Self::Scale => write!(f, "scale must be in (0, {MAX_SCALE}]"),
        }
    }
}

/// The renderer's spec for a wire request.
pub fn render_spec(spec: &BrowserSnapshotSpec) -> RenderSpec {
    RenderSpec {
        url: spec.url.clone(),
        layout_width: spec.layout_width,
        layout_height: spec.layout_height,
        scale: spec.scale,
        mobile: spec.mobile,
        scroll_x: spec.scroll_x.max(0.0),
        scroll_y: spec.scroll_y.max(0.0),
    }
}

/// Answer one snapshot request: `(HTTP status, JSON body)`.
pub(crate) fn snapshot_reply(body: &str) -> (&'static str, String) {
    let spec = match parse_spec(body) {
        Ok(spec) => spec,
        Err(error) => return ("400 Bad Request", error_body(&error.to_string(), None)),
    };
    if !crate::browser_url_policy::url_permitted(&spec.url) {
        return (
            "403 Forbidden",
            error_body(
                "this address is not allowed on this server",
                Some("blocked"),
            ),
        );
    }
    de_browser_render::start_idle_reaper();
    let Some(_slot) = RenderSlot::take(SLOT_WAIT) else {
        return (
            "503 Service Unavailable",
            error_body("the server is busy rendering other pages", Some("busy")),
        );
    };
    match BrowserPool::shared().renderer().render(&spec) {
        Ok(png) => (
            "200 OK",
            json!({
                "ok": true,
                "dataBase64": base64::engine::general_purpose::STANDARD.encode(png),
            })
            .to_string(),
        ),
        Err(BrowserRenderError::NoBrowser) => (
            "503 Service Unavailable",
            error_body("no browser is installed on the server", Some("noBrowser")),
        ),
        Err(error) => ("502 Bad Gateway", error_body(&error.to_string(), None)),
    }
}

fn error_body(message: &str, reason: Option<&str>) -> String {
    let mut body = json!({ "ok": false, "error": message });
    if let Some(reason) = reason {
        body["reason"] = json!(reason);
    }
    body.to_string()
}

fn parse_spec(body: &str) -> Result<RenderSpec, SnapshotRequestError> {
    let mut wire: BrowserSnapshotSpec =
        serde_json::from_str(body).map_err(|e| SnapshotRequestError::Body(e.to_string()))?;
    wire.url = de_editor_core::browser_devices::normalize_browser_url(&wire.url)
        .filter(|url| !url.is_empty())
        .ok_or(SnapshotRequestError::Url)?;
    let edge_ok = |edge: u32| (1..=MAX_LAYOUT_EDGE).contains(&edge);
    if !edge_ok(wire.layout_width) || !edge_ok(wire.layout_height) {
        return Err(SnapshotRequestError::Size);
    }
    if !(wire.scale > 0.0 && wire.scale <= MAX_SCALE) {
        return Err(SnapshotRequestError::Scale);
    }
    Ok(render_spec(&wire))
}

/// One of [`MAX_RENDERS`] render slots, held for the render.
struct RenderSlot;

static SLOTS: Mutex<usize> = Mutex::new(0);
static SLOT_FREED: Condvar = Condvar::new();

impl RenderSlot {
    /// A free slot, waiting up to `wait`; `None` when none came free.
    fn take(wait: Duration) -> Option<Self> {
        let deadline = Instant::now() + wait;
        let mut used = SLOTS.lock().unwrap_or_else(|e| e.into_inner());
        while *used >= MAX_RENDERS {
            let left = deadline.checked_duration_since(Instant::now())?;
            used = SLOT_FREED
                .wait_timeout(used, left)
                .unwrap_or_else(|e| e.into_inner())
                .0;
        }
        *used += 1;
        Some(RenderSlot)
    }
}

impl Drop for RenderSlot {
    fn drop(&mut self) {
        let mut used = SLOTS.lock().unwrap_or_else(|e| e.into_inner());
        *used = used.saturating_sub(1);
        SLOT_FREED.notify_one();
    }
}

#[cfg(test)]
#[path = "web_browser_snapshot_tests.rs"]
mod tests;
