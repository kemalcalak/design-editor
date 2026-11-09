//! The page input a streaming browser node's driver thread sends (#324,
//! phase 4): what waits for the page, and the connection it goes out on
//! (one of its own on the stream's tab, opened at the first input).
//!
//! A release never gets lost on the way: the queue keeps every release
//! past its limit (`InputBatch::extend_within`), and when a call fails
//! (the connection broke, the tab is busy) the rest of the batch goes
//! back as its releases and still-held presses
//! (`InputBatch::restore_held_state`), tried again on a new connection
//! after [`RETRY_AFTER`]. When the stream itself ends the driver keeps
//! the queue for the next stream on the node.

use std::time::{Duration, Instant};

use de_browser_render::{BrowserRenderError, PageInput};
use de_editor_core::browser_input::{BrowserInputEvent, InputBatch, MAX_QUEUED_INPUT_EVENTS};
use de_editor_core::browser_input_cdp::cdp_call;

/// Pause after a failed send before the next try.
pub(crate) const RETRY_AFTER: Duration = Duration::from_millis(250);

/// Where calls go: a stream tab's input connection (a fake in tests).
pub(crate) trait InputConnection {
    fn dispatch(
        &mut self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<(), BrowserRenderError>;
}

impl InputConnection for PageInput {
    fn dispatch(
        &mut self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<(), BrowserRenderError> {
        PageInput::dispatch(self, method, params)
    }
}

/// Page input waiting to go out, and the connection it goes out on.
pub(crate) struct InputSender<C = PageInput> {
    pending: InputBatch,
    connection: Option<C>,
    /// No send before this, after one failed.
    retry_at: Option<Instant>,
}

impl<C> Default for InputSender<C> {
    fn default() -> Self {
        Self {
            pending: InputBatch::new(),
            connection: None,
            retry_at: None,
        }
    }
}

impl<C: InputConnection> InputSender<C> {
    /// Queue `events` behind what waits (coalesced).
    pub(crate) fn queue(&mut self, events: Vec<BrowserInputEvent>) {
        self.pending.extend_within(events, MAX_QUEUED_INPUT_EVENTS);
    }

    /// Forget the connection (its stream ended); what waits stays.
    pub(crate) fn disconnect(&mut self) {
        self.connection = None;
        self.retry_at = None;
    }

    #[cfg(test)]
    pub(crate) fn pending(&self) -> &[BrowserInputEvent] {
        self.pending.events()
    }

    /// Send what is pending, in order, opening the connection with `open`
    /// when there is none. A failure drops the connection and puts the
    /// unsent releases back (see the module docs).
    pub(crate) fn send(
        &mut self,
        node_id: &str,
        open: impl FnOnce() -> Result<C, BrowserRenderError>,
    ) {
        if self.pending.is_empty() || self.retry_at.is_some_and(|at| Instant::now() < at) {
            return;
        }
        let events = self.pending.take();
        let connection = match self.connection.as_mut() {
            Some(connection) => connection,
            None => match open() {
                Ok(connection) => self.connection.insert(connection),
                Err(error) => {
                    eprintln!("[browser-live] {node_id} input: {error}");
                    self.failed(events);
                    return;
                }
            },
        };
        let mac_editing = cfg!(target_os = "macos");
        for (at, event) in events.iter().enumerate() {
            let call = cdp_call(event, mac_editing);
            if let Err(error) = connection.dispatch(call.method, call.params) {
                eprintln!("[browser-live] {node_id} input: {error}");
                // The failed call may not have reached the page either.
                let unsent = events.into_iter().skip(at).collect();
                self.connection = None;
                self.failed(unsent);
                return;
            }
        }
        self.retry_at = None;
    }

    fn failed(&mut self, unsent: Vec<BrowserInputEvent>) {
        self.pending.restore_held_state(unsent);
        self.retry_at = Some(Instant::now() + RETRY_AFTER);
    }
}

#[cfg(test)]
#[path = "browser_live_input_tests.rs"]
mod tests;
