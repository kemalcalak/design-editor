//! `POST /api/browser/input` (#324, phase 4): mouse and keyboard input a
//! web client sends to one of its live browser nodes in interaction mode.
//! The body is a `de_editor_core::browser_input::BrowserInputBatch` (the
//! client id from its live channel's `hello`, the node's key in its live
//! set, the events in the page's CSS px); the answer a
//! `BrowserInputReply`. The route is screened like `/api/browser/live`
//! (same-origin `Origin`, JSON content type: `origin_guard`), and only a
//! node that client streams right now takes input, so a page can only be
//! driven by the client that shows it.
//!
//! The batch is checked and coalesced here, then handed to the process's
//! [`InputSink`]: the live channel's hub, which owns the streams and sends
//! each batch on the node's stream tab with [`dispatch_events`]. Until a
//! sink is installed ([`set_input_sink`]) the route answers `503`.
//!
//! Binding the hub (after the web live client lands): give
//! `web_browser_live::source::LiveStream` an `input()` that returns
//! `PageStream::input`, keep one `de_browser_render::PageInput` per running
//! (client, key) in the hub (opened at the first batch, dropped with the
//! stream), implement [`InputSink`] for a small `Copy` handle wrapping
//! `&'static LiveHub` (`NotLive` when the client does not stream the key,
//! `Failed` when the page refused a call), and install it with
//! `set_input_sink(Arc::new(handle))` where the hub starts.
//!
//! The wasm client then needs: a `BrowserInteraction`
//! (`de_editor_ui::widgets::browser_interact_flow`) fed from its DOM
//! pointer / wheel / key events (DOM `key` / `code` / `text` as they come)
//! exactly like the native arm (`de-host-native/src/widget_host/
//! browser_interact.rs`: `enter_at` on a canvas double-click, `sync` once
//! per frame, the press / move / wheel / key outcomes deciding whether the
//! editor sees an event); then, once per frame, `take()` the queue, map the
//! node id to its key in the live set it posted, and post a
//! `BrowserInputBatch { client, key, events }` here with the `hello`
//! client id (one request in flight per node; batches queued meanwhile
//! fold together through `InputBatch`).

use std::sync::{Arc, Mutex, OnceLock};

use de_browser_render::{BrowserRenderError, PageInput};
use de_editor_core::browser_input::{
    BrowserInputBatch, BrowserInputEvent, BrowserInputReply, InputBatch, MAX_INPUT_EVENTS,
};
use de_editor_core::browser_input_cdp::cdp_calls;

/// Why a batch did not reach its page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum InputRefusal {
    /// The body is not a well-formed batch.
    Malformed(String),
    /// The client does not stream that node (or no such client).
    #[allow(dead_code)] // the hub's sink answers it, see the module docs
    NotLive,
    /// Input is not wired up in this daemon.
    Unavailable,
    /// The page did not take the events.
    #[allow(dead_code)] // the hub's sink answers it, see the module docs
    Failed(String),
}

impl std::fmt::Display for InputRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Malformed(detail) => f.write_str(detail),
            Self::NotLive => f.write_str("that node is not live for this client"),
            Self::Unavailable => f.write_str("page input is not available"),
            Self::Failed(detail) => write!(f, "the page did not take the input: {detail}"),
        }
    }
}

impl InputRefusal {
    fn status(&self) -> &'static str {
        match self {
            InputRefusal::Malformed(_) => "400 Bad Request",
            InputRefusal::NotLive => "404 Not Found",
            InputRefusal::Unavailable => "503 Service Unavailable",
            InputRefusal::Failed(_) => "502 Bad Gateway",
        }
    }
}

/// Where checked batches go: the owner of the live streams.
pub(crate) trait InputSink: Send + Sync {
    /// Send `events` to `client`'s live node `key`, in order. Returns how
    /// many went out.
    fn dispatch(
        &self,
        client: &str,
        key: &str,
        events: Vec<BrowserInputEvent>,
    ) -> Result<usize, InputRefusal>;
}

fn sink_slot() -> &'static Mutex<Option<Arc<dyn InputSink>>> {
    static SINK: OnceLock<Mutex<Option<Arc<dyn InputSink>>>> = OnceLock::new();
    SINK.get_or_init(|| Mutex::new(None))
}

/// Install the process's sink (the live channel's hub).
#[allow(dead_code)] // bound once the web live client lands, see the module docs
pub(crate) fn set_input_sink(sink: Arc<dyn InputSink>) {
    *sink_slot().lock().unwrap_or_else(|e| e.into_inner()) = Some(sink);
}

/// Status and JSON body for a `POST` [`BROWSER_INPUT_PATH`] body.
///
/// [`BROWSER_INPUT_PATH`]: de_editor_core::browser_input::BROWSER_INPUT_PATH
pub(crate) fn input_reply(body: &str) -> (&'static str, String) {
    let sink = sink_slot()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    input_reply_with(body, sink.as_deref())
}

/// [`input_reply`] against `sink` (tests pass a fake).
pub(crate) fn input_reply_with(body: &str, sink: Option<&dyn InputSink>) -> (&'static str, String) {
    let result = checked_batch(body).and_then(|batch| {
        let sink = sink.ok_or(InputRefusal::Unavailable)?;
        sink.dispatch(&batch.client, &batch.key, batch.events)
    });
    let (status, reply) = match result {
        Ok(dispatched) => (
            "200 OK",
            BrowserInputReply {
                ok: true,
                dispatched,
                error: None,
            },
        ),
        Err(refusal) => (
            refusal.status(),
            BrowserInputReply {
                ok: false,
                dispatched: 0,
                error: Some(refusal.to_string()),
            },
        ),
    };
    (status, serde_json::to_string(&reply).unwrap_or_default())
}

/// `body` parsed and checked, its events coalesced.
fn checked_batch(body: &str) -> Result<BrowserInputBatch, InputRefusal> {
    let batch: BrowserInputBatch = serde_json::from_str(body)
        .map_err(|e| InputRefusal::Malformed(format!("body must be an input batch: {e}")))?;
    if batch.client.is_empty() || batch.key.is_empty() {
        return Err(InputRefusal::Malformed(
            "client and key are required".into(),
        ));
    }
    if batch.events.len() > MAX_INPUT_EVENTS {
        return Err(InputRefusal::Malformed(format!(
            "at most {MAX_INPUT_EVENTS} events per batch"
        )));
    }
    if let Some(at) = batch.events.iter().position(|e| !e.is_valid()) {
        return Err(InputRefusal::Malformed(format!("event {at} is malformed")));
    }
    let mut events = InputBatch::new();
    events.extend(batch.events);
    Ok(BrowserInputBatch {
        events: events.take(),
        ..batch
    })
}

/// Send `events` on a stream tab's input connection, in order; the
/// browser runs in this process's OS. Returns how many went out.
#[allow(dead_code)] // the hub's sink calls it, see the module docs
pub(crate) fn dispatch_events(
    input: &mut PageInput,
    events: &[BrowserInputEvent],
) -> Result<usize, BrowserRenderError> {
    let calls = cdp_calls(events, cfg!(target_os = "macos"));
    let count = calls.len();
    for call in calls {
        input.dispatch(call.method, call.params)?;
    }
    Ok(count)
}

#[cfg(test)]
#[path = "web_browser_input_tests.rs"]
mod tests;
