//! The web editor's side of the daemon's live browser channel (#324),
//! without the transport: what the client streams, what it last told the
//! daemon, which frames to decode, and when to reconnect.
//!
//! The shared `BrowserStreamQueue` hands its Start / Update / Stop commands
//! to [`LiveClientState::apply`]. Each running node streams under a key of
//! its own (`<node id>#<n>`); a Reload gives the node a new key, so the
//! daemon opens a fresh page instead of keeping the old one. The daemon
//! learns the whole set in one call ([`LiveClientState::next_post`]) once
//! the event stream said hello, and again whenever the set moves on.
//! While the tab is hidden or unfocused the posted set is empty, so the
//! daemon stops the screencasts; the client's own map is kept, so they
//! resume as they were.
//!
//! Frames: each node decodes at most one frame at a time; a frame arriving
//! meanwhile waits, replaced by any newer one, so a slow decode drops
//! frames instead of queueing them.

use std::collections::BTreeMap;

use de_editor_core::browser_live_wire::{BrowserLiveEntry, BrowserLiveSet};
use de_editor_core::browser_stream::BrowserStreamSpec;
use de_editor_ui::widgets::browser_stream_queue::BrowserStreamCommand;

/// First reconnect delay; doubles per failed attempt.
const RECONNECT_BASE_MS: u32 = 1_000;
/// Longest reconnect delay.
const RECONNECT_MAX_MS: u32 = 30_000;

/// How long to wait before reconnect attempt `attempt` (0-based).
pub(crate) fn reconnect_delay_ms(attempt: u32) -> u32 {
    RECONNECT_BASE_MS
        // 2^5 already passes the cap; the bound keeps the shift in range.
        .saturating_mul(1 << attempt.min(5))
        .min(RECONNECT_MAX_MS)
}

/// A frame to decode: the node, its stream key and the JPEG.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FrameJob {
    pub node_id: String,
    pub key: String,
    pub seq: u64,
    pub jpeg: Vec<u8>,
}

struct Stream {
    key: String,
    spec: BrowserStreamSpec,
    last_seq: u64,
    decoding: bool,
    /// The newest frame that arrived while one was decoding.
    waiting: Option<(u64, Vec<u8>)>,
}

impl Stream {
    /// Stream under `key` from now on: a new page on the daemon. A decode
    /// of the old key still running lands as stale.
    fn rekey(&mut self, key: String) {
        self.key = key;
        self.last_seq = 0;
        self.waiting = None;
        self.decoding = false;
    }
}

/// See the module docs.
pub(crate) struct LiveClientState {
    streams: BTreeMap<String, Stream>,
    next_key: u64,
    active: bool,
    /// The id from the event stream's hello; `None` while disconnected.
    client: Option<String>,
    /// The set the daemon holds (as last posted); `None` when unknown.
    posted: Option<Vec<BrowserLiveEntry>>,
    post_in_flight: bool,
    /// Failed connections since the last hello.
    attempts: u32,
}

impl Default for LiveClientState {
    fn default() -> Self {
        Self {
            streams: BTreeMap::new(),
            next_key: 0,
            active: true,
            client: None,
            posted: None,
            post_in_flight: false,
            attempts: 0,
        }
    }
}

impl LiveClientState {
    /// Carry out one of the stream queue's commands.
    pub(crate) fn apply(&mut self, command: BrowserStreamCommand) {
        match command {
            BrowserStreamCommand::Start { node_id, spec } => {
                let key = self.new_key(&node_id);
                self.streams.insert(
                    node_id,
                    Stream {
                        key,
                        spec,
                        last_seq: 0,
                        decoding: false,
                        waiting: None,
                    },
                );
            }
            BrowserStreamCommand::Update {
                node_id,
                spec,
                reload,
            } => {
                let key = reload.then(|| self.new_key(&node_id));
                if let Some(stream) = self.streams.get_mut(&node_id) {
                    stream.spec = spec;
                    if let Some(key) = key {
                        stream.rekey(key);
                    }
                }
            }
            BrowserStreamCommand::Stop { node_id, .. } => {
                self.streams.remove(&node_id);
            }
        }
    }

    fn new_key(&mut self, node_id: &str) -> String {
        self.next_key += 1;
        format!("{node_id}#{}", self.next_key)
    }

    /// The tab became visible and focused (`true`) or not. Coming back,
    /// every stream takes a new key: the daemon stopped the pages while the
    /// set was empty and opens them afresh, counting frames from 1 again.
    pub(crate) fn set_active(&mut self, active: bool) {
        let resumed = active && !self.active;
        self.active = active;
        if resumed {
            let nodes: Vec<String> = self.streams.keys().cloned().collect();
            for node_id in nodes {
                let key = self.new_key(&node_id);
                if let Some(stream) = self.streams.get_mut(&node_id) {
                    stream.rekey(key);
                }
            }
        }
    }

    /// Whether the event stream should be open: something streams.
    pub(crate) fn wants_connection(&self) -> bool {
        self.active && !self.streams.is_empty()
    }

    /// Whether the event stream may close: nothing streams and the daemon
    /// already holds the empty set (so its streams are stopped).
    pub(crate) fn idle(&self) -> bool {
        self.streams.is_empty()
            && !self.post_in_flight
            && self.posted.as_ref().is_some_and(Vec::is_empty)
    }

    /// The client closed an idle event stream itself: no backoff, the next
    /// stream reconnects at once.
    pub(crate) fn on_closed(&mut self) {
        self.client = None;
        self.posted = None;
    }

    #[cfg(test)]
    fn connected(&self) -> bool {
        self.client.is_some()
    }

    /// The set the daemon should hold now.
    fn desired(&self) -> Vec<BrowserLiveEntry> {
        if !self.active {
            return Vec::new();
        }
        self.streams
            .values()
            .map(|stream| BrowserLiveEntry::for_stream(stream.key.clone(), &stream.spec))
            .collect()
    }

    /// The set call to send now, if the daemon's set is out of date and no
    /// call is on its way; it counts as sent.
    pub(crate) fn next_post(&mut self) -> Option<BrowserLiveSet> {
        let client = self.client.clone()?;
        if self.post_in_flight {
            return None;
        }
        let nodes = self.desired();
        if self.posted.as_ref() == Some(&nodes) {
            return None;
        }
        self.post_in_flight = true;
        self.posted = Some(nodes.clone());
        Some(BrowserLiveSet { client, nodes })
    }

    /// The set call came back; `ok` false when the daemon did not take it
    /// (the next call sends the whole set again; the transport retries).
    pub(crate) fn post_done(&mut self, ok: bool) {
        self.post_in_flight = false;
        if !ok {
            self.posted = None;
        }
    }

    /// The event stream said hello as `client`.
    pub(crate) fn on_hello(&mut self, client: String) {
        self.client = Some(client);
        self.posted = None;
        self.attempts = 0;
    }

    /// The event stream broke (or never opened): the daemon forgot this
    /// client and its streams. Returns the delay before reconnecting.
    pub(crate) fn on_disconnect(&mut self) -> u32 {
        self.client = None;
        self.posted = None;
        for stream in self.streams.values_mut() {
            stream.last_seq = 0;
        }
        let delay = reconnect_delay_ms(self.attempts);
        self.attempts = self.attempts.saturating_add(1);
        delay
    }

    /// Where page input for `node_id` goes (interaction mode): this
    /// client's id and the node's key, once the daemon holds the set that
    /// streams it (no set call on its way, so its page is open).
    pub(crate) fn input_target(&self, node_id: &str) -> Option<(String, String)> {
        let client = self.client.as_ref()?;
        let key = &self.streams.get(node_id)?.key;
        let held = !self.post_in_flight
            && self
                .posted
                .as_ref()
                .is_some_and(|set| set.iter().any(|entry| &entry.key == key));
        held.then(|| (client.clone(), key.clone()))
    }

    /// Whether `node_id` streams now.
    pub(crate) fn streams_node(&self, node_id: &str) -> bool {
        self.streams.contains_key(node_id)
    }

    /// The node streaming under `key`.
    fn node_of(&self, key: &str) -> Option<&str> {
        self.streams
            .iter()
            .find(|(_, stream)| stream.key == key)
            .map(|(node_id, _)| node_id.as_str())
    }

    /// Whether `key` is still `node_id`'s stream (a decoded frame of an
    /// older one is dropped).
    pub(crate) fn is_current(&self, node_id: &str, key: &str) -> bool {
        self.streams
            .get(node_id)
            .is_some_and(|stream| stream.key == key)
    }

    /// A frame event arrived. Returns the frame to decode now, if any: a
    /// frame of an unknown or older stream, an older frame, or any frame
    /// while the tab is inactive is dropped before its JPEG is unpacked
    /// (`jpeg`, `None` when it cannot be); one that arrives while the node
    /// decodes waits (replacing an older waiting one).
    pub(crate) fn on_frame(
        &mut self,
        key: &str,
        seq: u64,
        jpeg: impl FnOnce() -> Option<Vec<u8>>,
    ) -> Option<FrameJob> {
        if !self.active {
            return None;
        }
        let node_id = self.node_of(key)?.to_string();
        let stream = self.streams.get_mut(&node_id)?;
        if seq <= stream.last_seq {
            return None;
        }
        let jpeg = jpeg()?;
        stream.last_seq = seq;
        if stream.decoding {
            stream.waiting = Some((seq, jpeg));
            return None;
        }
        stream.decoding = true;
        Some(FrameJob {
            node_id,
            key: key.to_string(),
            seq,
            jpeg,
        })
    }

    /// `job`'s decode finished; returns the waiting frame to decode next.
    pub(crate) fn decode_done(&mut self, job: &FrameJob) -> Option<FrameJob> {
        let stream = self.streams.get_mut(&job.node_id)?;
        if stream.key != job.key {
            // Reloaded meanwhile: the new stream's own decodes run on.
            return None;
        }
        stream.decoding = false;
        let (seq, jpeg) = stream.waiting.take()?;
        stream.decoding = true;
        Some(FrameJob {
            node_id: job.node_id.clone(),
            key: stream.key.clone(),
            seq,
            jpeg,
        })
    }
}

#[cfg(test)]
#[path = "browser_live_client_tests.rs"]
mod tests;
