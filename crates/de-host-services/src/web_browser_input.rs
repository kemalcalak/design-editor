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
//! The batch is checked and coalesced here, then handed to the live
//! channel's hub (`web_browser_live::LiveHub`, the process's
//! [`InputSink`]), which owns the streams: it queues the batch for the
//! node's stream and a worker sends it on that stream's tab with
//! [`dispatch_events`] (`web_browser_live/input.rs`).
//!
//! The web client (`de-host-web/src/canvaskit/mount_browser_interaction.rs`
//! and `widget_host/browser_interact.rs`) feeds the shared
//! `BrowserInteraction` from its DOM events; `widget_host/
//! browser_input_post.rs` posts what it queued, one request in flight.

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
    NotLive,
    /// The page is not keeping up: too many events wait for it. Only the
    /// batch's releases and still-held presses were queued.
    Busy,
    /// The input could not be sent (no worker could start).
    Failed(String),
}

impl std::fmt::Display for InputRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Malformed(detail) => f.write_str(detail),
            Self::NotLive => f.write_str("that node is not live for this client"),
            Self::Busy => f.write_str("the page is not keeping up with its input"),
            Self::Failed(detail) => write!(f, "page input failed: {detail}"),
        }
    }
}

impl InputRefusal {
    fn status(&self) -> &'static str {
        match self {
            InputRefusal::Malformed(_) => "400 Bad Request",
            InputRefusal::NotLive => "404 Not Found",
            InputRefusal::Busy => "429 Too Many Requests",
            InputRefusal::Failed(_) => "500 Internal Server Error",
        }
    }
}

/// Where checked batches go: the owner of the live streams.
pub(crate) trait InputSink: Send + Sync {
    /// Queue `events` for `client`'s live node `key`, in order, without
    /// waiting for the page. Returns how many were taken.
    fn dispatch(
        &self,
        client: &str,
        key: &str,
        events: Vec<BrowserInputEvent>,
    ) -> Result<usize, InputRefusal>;
}

/// Status and JSON body for a `POST` [`BROWSER_INPUT_PATH`] body.
///
/// [`BROWSER_INPUT_PATH`]: de_editor_core::browser_input::BROWSER_INPUT_PATH
pub(crate) fn input_reply(body: &str) -> (&'static str, String) {
    input_reply_with(body, crate::web_browser_live::LiveHub::shared())
}

/// [`input_reply`] against `sink` (tests pass a fake).
pub(crate) fn input_reply_with(body: &str, sink: &dyn InputSink) -> (&'static str, String) {
    let result = checked_batch(body)
        .and_then(|batch| sink.dispatch(&batch.client, &batch.key, batch.events));
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
