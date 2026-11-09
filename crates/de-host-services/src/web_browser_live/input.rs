//! Page input for the live streams (#324, phase 4): the hub is the
//! process's `web_browser_input::InputSink`.
//!
//! Each running stream that took input has an [`InputLane`]: a worker
//! thread with the stream's input connection (`LiveStream::input`, a
//! DevTools connection of its own on the stream's tab), opened at the
//! first batch. A batch is only queued by the request (coalesced with what
//! still waits); the worker sends it, so neither the request thread nor
//! anyone holding the hub's locks waits on the page. The lane lives in the
//! stream's `Running` entry: when the stream stops (a set change, an end,
//! or the client leaving) the lane goes with it, its worker drops the
//! connection and exits, and what still waited is dropped with the page.
//! A held button is released by the client when interaction ends
//! (`BrowserInteraction::end` queues the release), as on desktop.

use std::sync::{Arc, Condvar, Mutex, Weak};

use de_browser_render::BrowserRenderError;
use de_editor_core::browser_input::{
    BrowserInputEvent, InputBatch, INPUT_RETRY_AFTER_MS, MAX_QUEUED_INPUT_EVENTS,
};

use super::source::{LiveInput, LiveStream};
use super::{lock, LiveHub};
use crate::web_browser_input::{InputRefusal, InputSink};

#[derive(Default)]
struct LaneState {
    pending: InputBatch,
    /// The stream stopped: the worker exits.
    closed: bool,
}

#[derive(Default)]
struct LaneShared {
    state: Mutex<LaneState>,
    changed: Condvar,
}

/// One stream's input queue and its worker; dropping it ends the worker.
pub(super) struct InputLane {
    shared: Arc<LaneShared>,
}

impl InputLane {
    /// Start the worker for `stream` (held weakly, so the lane never keeps
    /// a stopped stream's tab open). `key` names it in the log.
    fn start(stream: &Arc<dyn LiveStream>, key: &str) -> std::io::Result<Self> {
        let shared = Arc::new(LaneShared::default());
        let worker = Arc::clone(&shared);
        let stream = Arc::downgrade(stream);
        let key = key.to_string();
        std::thread::Builder::new()
            .name("browser-live-input".into())
            .spawn(move || run(&worker, &stream, &key))?;
        Ok(Self { shared })
    }

    /// Queue `events` for the worker; false when the lane already holds
    /// [`MAX_QUEUED_INPUT_EVENTS`] (the page is not keeping up): then
    /// only the releases and still-held presses are queued
    /// (`InputBatch::extend_within`), so the page never keeps a button or
    /// key held whatever the client does with the refusal.
    fn queue(&self, events: Vec<BrowserInputEvent>) -> bool {
        let mut state = lock(&self.shared.state);
        let all = state.pending.extend_within(events, MAX_QUEUED_INPUT_EVENTS);
        self.shared.changed.notify_one();
        all
    }
}

impl Drop for InputLane {
    fn drop(&mut self) {
        lock(&self.shared.state).closed = true;
        self.shared.changed.notify_one();
    }
}

/// The worker: send what is queued, in order, until the lane closes. A
/// call that did not get through drops the connection, which reopens for
/// the next try, and the batch's releases and still-held presses are
/// tried again; a call Chrome refused is not (it would fail again). Once
/// the stream is gone nothing is held on its page any more, so the worker
/// just ends.
fn run(shared: &LaneShared, stream: &Weak<dyn LiveStream>, key: &str) {
    let mut connection: Option<Box<dyn LiveInput>> = None;
    loop {
        let events = {
            let state = lock(&shared.state);
            let mut state = shared
                .changed
                .wait_while(state, |s| s.pending.is_empty() && !s.closed)
                .unwrap_or_else(|e| e.into_inner());
            if state.closed {
                return;
            }
            state.pending.take()
        };
        if connection.is_none() {
            let Some(live) = stream.upgrade() else {
                return;
            };
            match live.input() {
                Ok(opened) => connection = Some(opened),
                Err(error) => {
                    eprintln!("[browser-live] {key} input: {error}");
                    retry_later(shared, events);
                    continue;
                }
            }
        }
        if let Some(open) = connection.as_mut() {
            if let Err(error) = open.dispatch(&events) {
                eprintln!("[browser-live] {key} input: {error}");
                if !matches!(error, BrowserRenderError::Protocol { .. }) {
                    connection = None;
                    retry_later(shared, events);
                }
            }
        }
    }
}

/// Put a batch that failed back in front of the queue as its releases and
/// still-held presses, and pause before the next try on a new connection.
/// `LiveInput::dispatch` does not say which call failed, so the whole
/// batch goes back (unlike the desktop driver, which resends from the
/// failed call): an event that already got through may go out again,
/// which `keep_held_state` keeps harmless (a press followed by its
/// release leaves only the release).
fn retry_later(shared: &LaneShared, events: Vec<BrowserInputEvent>) {
    lock(&shared.state).pending.restore_held_state(events);
    std::thread::sleep(std::time::Duration::from_millis(INPUT_RETRY_AFTER_MS));
}

impl InputSink for LiveHub {
    fn dispatch(
        &self,
        client: &str,
        key: &str,
        events: Vec<BrowserInputEvent>,
    ) -> Result<usize, InputRefusal> {
        let Some(client) = lock(&self.clients).get(client).cloned() else {
            return Err(InputRefusal::NotLive);
        };
        let count = events.len();
        let lane = {
            let state = lock(&client.state);
            let running = state.streams.get(key).ok_or(InputRefusal::NotLive)?;
            match &running.input {
                Some(lane) => Arc::clone(lane),
                None => {
                    let (id, stream) = (running.id, Arc::clone(&running.stream));
                    drop(state);
                    // Started outside the lock; a racing batch may have
                    // installed one meanwhile, which then wins.
                    let started = InputLane::start(&stream, key)
                        .map_err(|e| InputRefusal::Failed(format!("no input worker: {e}")))?;
                    let mut state = lock(&client.state);
                    if !state.is_current(key, id) {
                        return Err(InputRefusal::NotLive);
                    }
                    let running = state.streams.get_mut(key).ok_or(InputRefusal::NotLive)?;
                    Arc::clone(running.input.get_or_insert_with(|| Arc::new(started)))
                }
            }
        };
        if !lane.queue(events) {
            return Err(InputRefusal::Busy);
        }
        Ok(count)
    }
}
