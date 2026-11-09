//! Page input of the web editor's interaction mode (#324, phase 4) on its
//! way to the serve-web daemon's `POST /api/browser/input`.
//!
//! The host's `BrowserInteraction` queues events per DOM event; [`flush`]
//! (after each event the page took, and once per repaint) moves them into
//! the [`InputOutbox`] and sends what waits as one `BrowserInputBatch` for
//! the node's live key (`browser_live_stream::input_target`, `<node>#<n>`).
//! At most one request is in flight; what comes meanwhile folds together
//! (`InputBatch`: moves replace moves, wheel steps add up) and goes with
//! the next one. Until the daemon streams the node (its set call still on
//! its way) the events wait and the send is retried on a short timer; a
//! node that never streams drops them after [`MAX_WAITS`] tries. A refused
//! batch is dropped (the page moved on), as on desktop.

use std::cell::{Cell, RefCell};

use de_editor_core::browser_input::{
    BrowserInputBatch, BrowserInputEvent, InputBatch, BROWSER_INPUT_PATH, MAX_INPUT_EVENTS,
};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use super::WidgetHost;

/// Events kept for a page that does not take them yet; more are dropped.
const MAX_PENDING: usize = 4 * MAX_INPUT_EVENTS;
/// How often a send that had nowhere to go is tried again.
const RETRY_MS: i32 = 100;
/// Tries before waiting events are dropped (about 5 s).
pub(crate) const MAX_WAITS: u32 = 50;

/// What to do next with the waiting events.
#[derive(Debug, PartialEq)]
pub(crate) enum NextSend {
    /// Nothing waits, or a request is in flight (its answer sends more).
    Idle,
    /// The node is not streamed yet: try again shortly.
    Retry,
    /// Post this; the outbox counts it as in flight.
    Post(BrowserInputBatch),
}

/// Events waiting for the page; transport-free.
#[derive(Debug, Default)]
pub(crate) struct InputOutbox {
    node: Option<String>,
    pending: InputBatch,
    in_flight: bool,
    waits: u32,
}

impl InputOutbox {
    /// Queue `events` for `node`; events still waiting for another node
    /// are stale and dropped.
    pub(crate) fn queue(&mut self, node: String, events: Vec<BrowserInputEvent>) {
        if self.node.as_deref() != Some(node.as_str()) {
            self.pending = InputBatch::new();
            self.node = Some(node);
            self.waits = 0;
        }
        if self.pending.len() + events.len() > MAX_PENDING {
            return;
        }
        self.pending.extend(events);
    }

    /// The next batch to post, given where the node's input goes now.
    pub(crate) fn next(
        &mut self,
        target: impl FnOnce(&str) -> Option<(String, String)>,
    ) -> NextSend {
        if self.in_flight || self.pending.is_empty() {
            return NextSend::Idle;
        }
        let Some(node) = self.node.as_deref() else {
            return NextSend::Idle;
        };
        let Some((client, key)) = target(node) else {
            self.waits += 1;
            if self.waits > MAX_WAITS {
                self.pending = InputBatch::new();
                self.waits = 0;
                return NextSend::Idle;
            }
            return NextSend::Retry;
        };
        self.waits = 0;
        let mut events = self.pending.take();
        if events.len() > MAX_INPUT_EVENTS {
            let rest = events.split_off(MAX_INPUT_EVENTS);
            self.pending.extend(rest);
        }
        self.in_flight = true;
        NextSend::Post(BrowserInputBatch {
            client,
            key,
            events,
        })
    }

    /// The request in flight came back (taken or not).
    pub(crate) fn done(&mut self) {
        self.in_flight = false;
    }
}

thread_local! {
    static OUTBOX: RefCell<InputOutbox> = RefCell::new(InputOutbox::default());
    /// A retry timer is armed.
    static RETRY_PENDING: Cell<bool> = const { Cell::new(false) };
}

/// Move the host's queued page input into the outbox and send what
/// waits.
pub(crate) fn flush(host: &mut WidgetHost) {
    if let Some((node, events)) = host.take_browser_input() {
        OUTBOX.with(|outbox| outbox.borrow_mut().queue(node, events));
    }
    send();
}

fn send() {
    let next = OUTBOX.with(|outbox| {
        outbox
            .borrow_mut()
            .next(super::browser_live_stream::input_target)
    });
    match next {
        NextSend::Idle => {}
        NextSend::Retry => arm_retry(),
        NextSend::Post(batch) => post(&batch),
    }
}

fn arm_retry() {
    if RETRY_PENDING.with(|pending| pending.replace(true)) {
        return;
    }
    let retry = Closure::once_into_js(|| {
        RETRY_PENDING.with(|pending| pending.set(false));
        send();
    });
    let armed = web_sys::window().is_some_and(|window| {
        window
            .set_timeout_with_callback_and_timeout_and_arguments_0(retry.unchecked_ref(), RETRY_MS)
            .is_ok()
    });
    if !armed {
        RETRY_PENDING.with(|pending| pending.set(false));
    }
}

fn post(batch: &BrowserInputBatch) {
    let url = crate::daemon_base::daemon_url(BROWSER_INPUT_PATH);
    let on_reply: std::rc::Rc<dyn Fn(u16, String)> = std::rc::Rc::new(|_status, _body| {
        OUTBOX.with(|outbox| outbox.borrow_mut().done());
        send();
    });
    let sent = serde_json::to_string(batch)
        .is_ok_and(|body| crate::live_sync::post_json_with_status(&url, &body, on_reply));
    if !sent {
        OUTBOX.with(|outbox| outbox.borrow_mut().done());
    }
}

#[cfg(test)]
#[path = "browser_input_post_tests.rs"]
mod tests;
