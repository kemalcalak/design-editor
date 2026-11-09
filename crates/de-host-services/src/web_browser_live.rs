//! The serve-web daemon's live browser channel (#324): each web client
//! streams its live browser nodes over one event stream, the wire shapes in
//! `de_editor_core::browser_live_wire`.
//!
//! `GET /api/browser/live` registers a client and holds its connection
//! thread as the client's writer; `POST /api/browser/live` replaces that
//! client's set. The hub opens, resizes and stops screencasts
//! (`de_browser_render::PageStream`) to match, at most
//! `WEB_LIVE_MAX_STREAMS` per client at `WEB_LIVE_MAX_FPS`, behind the same
//! URL policy as snapshots.
//!
//! A stream keeps only its newest frame and is acked once that frame is
//! taken. The writer takes a node's frame only when it is about to write
//! it, so a slow client slows its streams instead of queueing frames: what
//! it misses is replaced, never sent late. When the client goes away (a
//! write fails), its streams stop and the browser pool's idle reaper takes
//! it from there. Nothing here takes the daemon's state lock.

mod plan;
mod source;

use std::collections::{BTreeMap, HashMap};
use std::io::{self, Write};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock};
use std::time::Duration;

use de_browser_render::{BrowserPool, BrowserRenderError, StreamFrame};
use de_editor_core::browser_live_wire::{
    BrowserLiveEndReason, BrowserLiveEvent, BrowserLiveSet, BrowserLiveSetReply,
};

use plan::{plan, select, LiveSpec, Selection};
use source::{BrowserSource, LiveSource, LiveStream};

/// Event streams open at once; a stream more is refused as busy.
const MAX_CLIENTS: usize = 8;
/// The event writer's buffer: about one mid-size frame.
const WRITE_BUFFER: usize = 64 * 1024;
/// Keep-alive cadence of an idle event stream, also how soon a client that
/// left without a frame to write is noticed.
const LIVE_HEARTBEAT: Duration = Duration::from_secs(5);

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

/// Raised by a stream or a set change; the writer waits on it.
#[derive(Default)]
struct WakeFlag {
    raised: Mutex<bool>,
    changed: Condvar,
}

impl WakeFlag {
    fn raise(&self) {
        *lock(&self.raised) = true;
        self.changed.notify_all();
    }

    /// Wait up to `timeout` for a raise; whether one came. Clears it.
    fn wait(&self, timeout: Duration) -> bool {
        let raised = lock(&self.raised);
        let (mut raised, _) = self
            .changed
            .wait_timeout_while(raised, timeout, |raised| !*raised)
            .unwrap_or_else(|e| e.into_inner());
        std::mem::take(&mut *raised)
    }
}

/// A stream the client runs; `id` tells a reopened stream from the one a
/// slow call still holds.
struct Running {
    id: u64,
    spec: LiveSpec,
    stream: Arc<dyn LiveStream>,
}

#[derive(Default)]
struct ClientState {
    /// The event stream ended; nothing may be added.
    closed: bool,
    streams: BTreeMap<String, Running>,
    /// Specs whose stream failed to open or ended; not reopened until the
    /// spec changes.
    failed: BTreeMap<String, LiveSpec>,
    /// Events for the writer besides frames.
    notices: Vec<BrowserLiveEvent>,
    next_id: u64,
}

struct LiveClient {
    id: String,
    /// One set change at a time.
    turn: Mutex<()>,
    state: Mutex<ClientState>,
    wake: Arc<WakeFlag>,
}

/// The live channel's clients. See the module docs.
pub(crate) struct LiveHub {
    source: Arc<dyn LiveSource>,
    clients: Mutex<HashMap<String, Arc<LiveClient>>>,
    heartbeat: Duration,
}

impl LiveHub {
    fn new(source: Arc<dyn LiveSource>, heartbeat: Duration) -> Self {
        Self {
            source,
            clients: Mutex::new(HashMap::new()),
            heartbeat,
        }
    }

    /// The daemon's hub, streaming from the shared browser.
    pub(crate) fn shared() -> &'static LiveHub {
        static HUB: OnceLock<LiveHub> = OnceLock::new();
        HUB.get_or_init(|| {
            let source = BrowserSource {
                pool: BrowserPool::shared(),
            };
            LiveHub::new(Arc::new(source), LIVE_HEARTBEAT)
        })
    }

    /// Register a client, or `None` when [`MAX_CLIENTS`] are connected.
    pub(crate) fn connect(&self) -> Option<LiveLease<'_>> {
        let mut clients = lock(&self.clients);
        if clients.len() >= MAX_CLIENTS {
            return None;
        }
        let id = new_client_id();
        let client = Arc::new(LiveClient {
            id: id.clone(),
            turn: Mutex::new(()),
            state: Mutex::new(ClientState::default()),
            wake: Arc::new(WakeFlag::default()),
        });
        clients.insert(id, Arc::clone(&client));
        Some(LiveLease { hub: self, client })
    }

    /// Forget a client and stop its streams.
    fn disconnect(&self, id: &str) {
        let Some(client) = lock(&self.clients).remove(id) else {
            return;
        };
        let streams = {
            let mut state = lock(&client.state);
            state.closed = true;
            std::mem::take(&mut state.streams)
        };
        drop(streams);
    }

    /// Answer `POST /api/browser/live`: `(HTTP status, JSON body)`.
    pub(crate) fn set_reply(
        &self,
        body: &str,
        document_folder: Option<&str>,
    ) -> (&'static str, String) {
        let (status, reply) = match serde_json::from_str::<BrowserLiveSet>(body) {
            Ok(set) => self.apply(
                set,
                document_folder,
                crate::browser_url_policy::url_permitted,
            ),
            Err(error) => (
                "400 Bad Request",
                refusal(format!("body must be the live set JSON: {error}")),
            ),
        };
        let body = serde_json::to_string(&reply).unwrap_or_default();
        (status, body)
    }

    /// Make `set` its client's live set, screening addresses with
    /// `permitted`. Opens run in parallel without any lock held.
    fn apply(
        &self,
        set: BrowserLiveSet,
        document_folder: Option<&str>,
        permitted: impl Fn(&str) -> bool,
    ) -> (&'static str, BrowserLiveSetReply) {
        let Some(client) = lock(&self.clients).get(&set.client).cloned() else {
            return ("404 Not Found", refusal("unknown live client".into()));
        };
        let _turn = lock(&client.turn);
        let selection = select(set.nodes, document_folder, permitted);
        let Some(changes) = client.begin(&selection) else {
            return ("404 Not Found", refusal("unknown live client".into()));
        };
        for update in changes.update {
            client.update(update);
        }
        let opens: Vec<_> = std::thread::scope(|scope| {
            let handles: Vec<_> = changes
                .open
                .iter()
                .map(|spec| {
                    let wake = Arc::clone(&client.wake);
                    let waker: source::Waker = Arc::new(move || wake.raise());
                    scope.spawn(move || self.source.open(spec, waker))
                })
                .collect();
            handles
                .into_iter()
                .map(|handle| handle.join().unwrap_or(Err(BrowserRenderError::Closed)))
                .collect()
        });
        client.finish_opens(changes.open.into_iter().zip(opens).collect());
        client.wake.raise();
        ("200 OK", client.reply(&selection))
    }
}

/// A connected client; dropping it disconnects the client.
pub(crate) struct LiveLease<'a> {
    hub: &'a LiveHub,
    client: Arc<LiveClient>,
}

impl Drop for LiveLease<'_> {
    fn drop(&mut self) {
        self.hub.disconnect(&self.client.id);
    }
}

impl LiveLease<'_> {
    /// Write the client's event stream to `out` until a write fails (the
    /// client left).
    pub(crate) fn serve_events<S: Write>(
        &self,
        out: &mut S,
        cors_origin: Option<&str>,
    ) -> io::Result<()> {
        // Frames are base64-encoded straight into this buffer in small
        // pieces; it turns them into a few large socket writes.
        let mut out = io::BufWriter::with_capacity(WRITE_BUFFER, out);
        crate::ai_proxy::write_sse_headers(&mut out, cors_origin)?;
        let hello = BrowserLiveEvent::Hello {
            client: self.client.id.clone(),
        };
        write_event(&mut out, &hello)?;
        out.flush()?;
        loop {
            let outgoing = self.client.drain();
            if outgoing.is_empty() {
                if !self.client.wake.wait(self.hub.heartbeat) {
                    out.write_all(b": ping\n\n")?;
                    out.flush()?;
                }
                continue;
            }
            for item in &outgoing {
                item.write_to(&mut out)?;
            }
            out.flush()?;
        }
    }
}

/// One item for the writer: a notice, or a node's newest frame, which is
/// written straight from its JPEG bytes ([`write_frame_event`]).
#[derive(Debug)]
enum Outgoing {
    Event(BrowserLiveEvent),
    Frame { key: String, frame: StreamFrame },
}

impl Outgoing {
    fn write_to<S: Write>(&self, out: &mut S) -> io::Result<()> {
        match self {
            Outgoing::Event(event) => write_event(out, event),
            Outgoing::Frame { key, frame } => write_frame_event(out, key, frame),
        }
    }
}

/// How an open came out.
type Opened = Result<Arc<dyn LiveStream>, BrowserRenderError>;

/// What a set change does once its stops ran.
struct Changes {
    update: Vec<plan::Update>,
    open: Vec<LiveSpec>,
}

impl LiveClient {
    /// Note the set's refusals, stop what it no longer holds and return
    /// the rest of the plan; `None` when the client is gone.
    fn begin(&self, selection: &Selection) -> Option<Changes> {
        let (changes, stopped) = {
            let mut state = lock(&self.state);
            if state.closed {
                return None;
            }
            for key in &selection.blocked {
                let key = key.clone();
                state.notices.push(BrowserLiveEvent::Blocked { key });
            }
            for (key, message) in &selection.invalid {
                state.notices.push(BrowserLiveEvent::Ended {
                    key: key.clone(),
                    reason: BrowserLiveEndReason::Invalid,
                    message: message.clone(),
                });
            }
            let wanted = &selection.wanted;
            state
                .failed
                .retain(|key, spec| wanted.iter().any(|w| &w.key == key && w == spec));
            let current = state
                .streams
                .iter()
                .map(|(key, running)| (key.clone(), running.spec.clone()))
                .collect();
            let plan = plan(&current, wanted);
            let stopped: Vec<_> = plan
                .stop
                .iter()
                .filter_map(|key| state.streams.remove(key))
                .collect();
            let open = plan
                .open
                .into_iter()
                .filter(|spec| state.failed.get(&spec.key) != Some(spec))
                .collect();
            let changes = Changes {
                update: plan.update,
                open,
            };
            (changes, stopped)
        };
        // Streams stop outside the lock.
        drop(stopped);
        Some(changes)
    }

    /// Resize a running stream in place; one that fails is dropped.
    fn update(&self, update: plan::Update) {
        let key = &update.spec.key;
        let Some((id, stream)) = lock(&self.state)
            .streams
            .get(key)
            .map(|running| (running.id, Arc::clone(&running.stream)))
        else {
            return;
        };
        let spec = &update.spec;
        let mut result = Ok(());
        if update.viewport {
            result = stream.set_viewport(spec.viewport);
        }
        if update.settings {
            result = result.and_then(|()| stream.set_settings(spec.settings));
        }
        let mut state = lock(&self.state);
        if !state.is_current(key, id) {
            return;
        }
        match result {
            Ok(()) => {
                if let Some(running) = state.streams.get_mut(key) {
                    running.spec = update.spec;
                }
            }
            Err(error) => {
                let removed = state.streams.remove(key);
                state.end(key, spec, &error);
                drop(state);
                drop(removed);
            }
        }
    }

    /// Install the streams that opened; note the ones that did not.
    fn finish_opens(&self, opens: Vec<(LiveSpec, Opened)>) {
        // Opened after the client left, or replaced: stopped outside the lock.
        let mut to_stop = Vec::new();
        {
            let mut state = lock(&self.state);
            for (spec, opened) in opens {
                match opened {
                    Ok(stream) if state.closed => to_stop.push(stream),
                    Ok(stream) => {
                        state.next_id += 1;
                        let running = Running {
                            id: state.next_id,
                            spec: spec.clone(),
                            stream,
                        };
                        if let Some(old) = state.streams.insert(spec.key, running) {
                            to_stop.push(old.stream);
                        }
                    }
                    Err(error) => state.end(&spec.key, &spec, &error),
                }
            }
        }
        drop(to_stop);
    }

    /// The pending notices and the newest frame of each stream. Taking a
    /// frame here, right before it is written, is what paces the streams.
    fn drain(&self) -> Vec<Outgoing> {
        let streams: Vec<_> = lock(&self.state)
            .streams
            .iter()
            .map(|(key, running)| (key.clone(), running.id, Arc::clone(&running.stream)))
            .collect();
        let taken: Vec<_> = streams
            .into_iter()
            .map(|(key, id, stream)| (key, id, stream.take_frame()))
            .collect();
        let (notices, frames, removed) = {
            let mut state = lock(&self.state);
            let mut frames = Vec::new();
            let mut removed = Vec::new();
            for (key, id, taken) in taken {
                // A set call may have stopped or reopened the key since:
                // the old stream's frame must not go out under the key.
                if !state.is_current(&key, id) {
                    continue;
                }
                match taken {
                    Ok(Some(frame)) => frames.push((key, frame)),
                    Ok(None) => {}
                    Err(error) => {
                        if let Some(running) = state.streams.remove(&key) {
                            state.end(&key, &running.spec, &error);
                            removed.push(running);
                        }
                    }
                }
            }
            (std::mem::take(&mut state.notices), frames, removed)
        };
        drop(removed);
        let mut outgoing: Vec<Outgoing> = notices.into_iter().map(Outgoing::Event).collect();
        outgoing.extend(
            frames
                .into_iter()
                .map(|(key, frame)| Outgoing::Frame { key, frame }),
        );
        outgoing
    }

    /// The reply to the set call that just ran.
    fn reply(&self, selection: &Selection) -> BrowserLiveSetReply {
        let state = lock(&self.state);
        let mut failed: Vec<String> = selection.invalid.iter().map(|(k, _)| k.clone()).collect();
        failed.extend(
            selection
                .wanted
                .iter()
                .filter(|spec| state.failed.contains_key(&spec.key))
                .map(|spec| spec.key.clone()),
        );
        BrowserLiveSetReply {
            ok: true,
            live: state.streams.keys().cloned().collect(),
            blocked: selection.blocked.clone(),
            failed,
            dropped: selection.dropped.clone(),
            error: None,
        }
    }
}

impl ClientState {
    /// Whether `key` still runs the stream `id`.
    fn is_current(&self, key: &str, id: u64) -> bool {
        self.streams
            .get(key)
            .is_some_and(|running| running.id == id)
    }

    /// `key`'s stream ended or failed to start with `error`.
    fn end(&mut self, key: &str, spec: &LiveSpec, error: &BrowserRenderError) {
        self.failed.insert(key.to_string(), spec.clone());
        self.notices.push(ended_event(key.to_string(), error));
    }
}

fn refusal(error: String) -> BrowserLiveSetReply {
    BrowserLiveSetReply {
        error: Some(error),
        ..Default::default()
    }
}

/// An id no other client can guess (it is what a set call names).
fn new_client_id() -> String {
    let mut bytes = [0u8; 16];
    if getrandom::fill(&mut bytes).is_err() {
        // No OS randomness: still unique, just guessable.
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        bytes = nanos.to_le_bytes();
    }
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Write `frame` as a [`BrowserLiveEvent::Frame`] event, the same JSON
/// serde makes, with the JPEG base64-encoded straight into `out`: a frame
/// is never held as a base64 string, nor copied into a JSON one.
fn write_frame_event<S: Write>(out: &mut S, key: &str, frame: &StreamFrame) -> io::Result<()> {
    let (width, height) = jpeg_size(&frame.jpeg).unwrap_or((0, 0));
    out.write_all(br#"data: {"type":"frame","key":"#)?;
    serde_json::to_writer(&mut *out, key).map_err(io::Error::other)?;
    write!(
        out,
        r#","seq":{},"width":{width},"height":{height},"jpegBase64":""#,
        frame.seq
    )?;
    {
        let engine = &base64::engine::general_purpose::STANDARD;
        let mut encoder = base64::write::EncoderWriter::new(&mut *out, engine);
        encoder.write_all(&frame.jpeg)?;
        encoder.finish()?;
    }
    out.write_all(b"\"}\n\n")
}

/// The JPEG's size from its header (nothing is decoded).
fn jpeg_size(jpeg: &[u8]) -> Option<(u32, u32)> {
    image::ImageReader::with_format(io::Cursor::new(jpeg), image::ImageFormat::Jpeg)
        .into_dimensions()
        .ok()
}

fn ended_event(key: String, error: &BrowserRenderError) -> BrowserLiveEvent {
    let reason = match error {
        BrowserRenderError::NoBrowser => BrowserLiveEndReason::NoBrowser,
        _ => BrowserLiveEndReason::Failed,
    };
    BrowserLiveEvent::Ended {
        key,
        reason,
        message: error.to_string(),
    }
}

fn write_event<S: Write>(out: &mut S, event: &BrowserLiveEvent) -> io::Result<()> {
    let json = serde_json::to_string(event).map_err(io::Error::other)?;
    out.write_all(b"data: ")?;
    out.write_all(json.as_bytes())?;
    out.write_all(b"\n\n")
}

#[cfg(test)]
#[path = "web_browser_live_tests.rs"]
mod tests;
