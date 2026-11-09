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

use de_editor_core::browser_input::{BrowserInputEvent, InputBatch, MAX_INPUT_EVENTS};

use super::source::{LiveInput, LiveStream};
use super::{lock, LiveHub};
use crate::web_browser_input::{InputRefusal, InputSink};

/// Events a lane holds for a slow page before it refuses more.
const LANE_MAX_EVENTS: usize = 4 * MAX_INPUT_EVENTS;

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
    /// too many (the page is not keeping up).
    fn queue(&self, events: Vec<BrowserInputEvent>) -> bool {
        let mut state = lock(&self.shared.state);
        if state.pending.len() + events.len() > LANE_MAX_EVENTS {
            return false;
        }
        state.pending.extend(events);
        self.shared.changed.notify_one();
        true
    }
}

impl Drop for InputLane {
    fn drop(&mut self) {
        lock(&self.shared.state).closed = true;
        self.shared.changed.notify_one();
    }
}

/// The worker: send what is queued, in order, until the lane closes. A
/// failed call drops the rest of its batch and the connection, which
/// reopens with the next batch (as the desktop driver does).
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
                    continue;
                }
            }
        }
        if let Some(open) = connection.as_mut() {
            if let Err(error) = open.dispatch(&events) {
                eprintln!("[browser-live] {key} input: {error}");
                connection = None;
            }
        }
    }
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
