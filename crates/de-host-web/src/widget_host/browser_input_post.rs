//! Page input of the web editor's interaction mode (#324, phase 4) on its
//! way to the serve-web daemon's `POST /api/browser/input`.
//!
//! The host's `BrowserInteraction` queues events per DOM event; [`flush`]
//! (after each DOM event while a node is interactive, and once per
//! repaint) moves them into the [`InputOutbox`] and sends what waits as one
//! `BrowserInputBatch` for the node's live key
//! (`browser_live_stream::input_target`, `<node>#<n>`). At most one request
//! is in flight and posts are at least [`MIN_POST_GAP_MS`] apart (a 1 kHz
//! mouse must not mean 1000 requests a second); what comes meanwhile folds
//! together (`InputBatch`: moves replace moves, wheel steps add up) and
//! goes with the next one. Until the daemon streams the node (its set call
//! still opening the page) the events wait and the send is retried on a
//! short timer; a node that never streams drops them after
//! [`MAX_WAIT_MS`]. A refused batch is dropped (the page moved on), as on
//! desktop.

use std::cell::{Cell, RefCell};

use de_editor_core::browser_input::{
    BrowserInputBatch, BrowserInputEvent, InputBatch, BROWSER_INPUT_PATH, MAX_INPUT_EVENTS,
};

use super::WidgetHost;

/// Events kept for a page that does not take them yet; more are dropped.
const MAX_PENDING: usize = 4 * MAX_INPUT_EVENTS;
/// How soon a send that had nowhere to go is tried again.
const RETRY_MS: u32 = 100;
/// How long events wait for their node to stream before they are dropped
/// (a cold browser takes over a second to open the page).
pub(crate) const MAX_WAIT_MS: u64 = 5_000;
/// Shortest time between two posts: about one frame.
pub(crate) const MIN_POST_GAP_MS: u64 = 16;

/// What to do next with the waiting events.
#[derive(Debug, PartialEq)]
pub(crate) enum NextSend {
    /// Nothing waits, or a request is in flight (its answer sends more).
    Idle,
    /// Try again after this many ms (too soon after the last post, or the
    /// node does not stream yet).
    Retry(u32),
    /// Post this; the outbox counts it as in flight.
    Post(BrowserInputBatch),
}

/// Events waiting for the page; transport-free.
#[derive(Debug, Default)]
pub(crate) struct InputOutbox {
    node: Option<String>,
    pending: InputBatch,
    in_flight: bool,
    /// When the waiting events first found no stream to go to.
    waiting_since_ms: Option<u64>,
    last_post_ms: Option<u64>,
}

impl InputOutbox {
    /// Queue `events` for `node`; events still waiting for another node
    /// are stale and dropped.
    pub(crate) fn queue(&mut self, node: String, events: Vec<BrowserInputEvent>) {
        if self.node.as_deref() != Some(node.as_str()) {
            self.pending = InputBatch::new();
            self.node = Some(node);
            self.waiting_since_ms = None;
        }
        if self.pending.len() + events.len() > MAX_PENDING {
            return;
        }
        self.pending.extend(events);
    }

    /// The next batch to post at `now_ms`, given where the node's input
    /// goes now.
    pub(crate) fn next(
        &mut self,
        now_ms: u64,
        target: impl FnOnce(&str) -> Option<(String, String)>,
    ) -> NextSend {
        if self.in_flight || self.pending.is_empty() {
            return NextSend::Idle;
        }
        let Some(node) = self.node.as_deref() else {
            return NextSend::Idle;
        };
        if let Some(last) = self.last_post_ms {
            let due = last + MIN_POST_GAP_MS;
            if now_ms < due {
                return NextSend::Retry((due - now_ms) as u32);
            }
        }
        let Some((client, key)) = target(node) else {
            let since = *self.waiting_since_ms.get_or_insert(now_ms);
            if now_ms.saturating_sub(since) > MAX_WAIT_MS {
                self.pending = InputBatch::new();
                self.waiting_since_ms = None;
                return NextSend::Idle;
            }
            return NextSend::Retry(RETRY_MS);
        };
        self.waiting_since_ms = None;
        let mut events = self.pending.take();
        if events.len() > MAX_INPUT_EVENTS {
            let rest = events.split_off(MAX_INPUT_EVENTS);
            self.pending.extend(rest);
        }
        self.in_flight = true;
        self.last_post_ms = Some(now_ms);
        NextSend::Post(BrowserInputBatch {
            client,
            key,
            events,
        })
    }

    /// The request in flight came back (taken or not), or never left.
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
    let now_ms = crate::listener::now_ms_perf();
    let next = OUTBOX.with(|outbox| {
        outbox
            .borrow_mut()
            .next(now_ms, super::browser_live_stream::input_target)
    });
    match next {
        NextSend::Idle => {}
        NextSend::Retry(delay_ms) => arm_retry(delay_ms),
        NextSend::Post(batch) => post(&batch),
    }
}

fn arm_retry(delay_ms: u32) {
    if RETRY_PENDING.with(|pending| pending.replace(true)) {
        return;
    }
    let armed = super::browser_live_stream::after(delay_ms, || {
        RETRY_PENDING.with(|pending| pending.set(false));
        send();
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
        // Never left: what still waits goes on the next try.
        OUTBOX.with(|outbox| outbox.borrow_mut().done());
        arm_retry(RETRY_MS);
    }
}

#[cfg(test)]
#[path = "browser_input_post_tests.rs"]
mod tests;
