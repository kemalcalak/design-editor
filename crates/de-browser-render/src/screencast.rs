//! Live page frames for a canvas browser node (#324): a CDP
//! `Page.startScreencast` on a tab, read on a connection and thread of its
//! own. Chrome sends a JPEG only when the page repaints and the next one
//! only after the last was acked, so a still page costs nothing.
//!
//! The newest frame waits in a latest-wins slot. A frame is acked once the
//! consumer took the last one handed out **and** `1 / max_fps` passed
//! since the previous ack: one rule gives both the rate cap and
//! backpressure (a slow consumer slows the page's stream instead of
//! queueing frames). How that maps onto Chrome's in-flight window is in
//! `screencast_worker`. Nothing is decoded here; the consumer gets the
//! JPEG bytes.
//!
//! A stream runs on a fresh tab for an address ([`PageStream::open`]) or
//! on the kept-open tab of an MCP page session
//! ([`crate::PageSessions::stream`]), whose idle close waits while the
//! stream lives. Either way it holds the shared browser up. Dropping the
//! handle stops the stream without waiting; [`PageStream::stop`] waits.

use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::cdp_client::CdpClient;
use crate::page_session::TabStreamClaim;
use crate::screencast_worker::{Control, Holds, TabMode, Worker};
use crate::tab::{Tab, Viewport};
use crate::{BrowserPool, BrowserRenderError, PoolLease};

/// Widest frame by default, in device px.
const DEFAULT_MAX_WIDTH: u32 = 1024;
const DEFAULT_MAX_HEIGHT: u32 = 2048;
const DEFAULT_QUALITY: u8 = 70;
/// The desktop cap; the web daemon passes its own.
const DEFAULT_MAX_FPS: f64 = 10.0;
/// The stream connection wakes this often to notice a taken frame or a
/// control message.
pub(crate) const STREAM_READ_SLICE: Duration = Duration::from_millis(10);
/// Longest wait for a restart: stop, a viewport emulation and a start.
const RESTART_TIMEOUT: Duration = Duration::from_secs(60);

/// How a stream encodes and paces frames.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StreamSettings {
    /// Frames are scaled down to fit these, in device px.
    pub max_width: u32,
    pub max_height: u32,
    /// JPEG quality, 0–100.
    pub quality: u8,
    /// Send every n-th repaint only (1 = all).
    pub every_nth_frame: u32,
    /// Frame-rate cap; 0 (or less) means none.
    pub max_fps: f64,
}

impl Default for StreamSettings {
    fn default() -> Self {
        Self {
            max_width: DEFAULT_MAX_WIDTH,
            max_height: DEFAULT_MAX_HEIGHT,
            quality: DEFAULT_QUALITY,
            every_nth_frame: 1,
            max_fps: DEFAULT_MAX_FPS,
        }
    }
}

impl StreamSettings {
    /// The shortest time between two acks (`1 / max_fps`).
    pub fn min_interval(&self) -> Duration {
        if self.max_fps.is_finite() && self.max_fps > 0.0 {
            Duration::from_secs_f64(1.0 / self.max_fps)
        } else {
            Duration::ZERO
        }
    }

    /// `Page.startScreencast` parameters.
    pub(crate) fn start_params(&self) -> Value {
        json!({
            "format": "jpeg",
            "quality": self.quality.min(100),
            "maxWidth": self.max_width.max(1),
            "maxHeight": self.max_height.max(1),
            "everyNthFrame": self.every_nth_frame.max(1),
        })
    }
}

/// What Chrome reports with a frame (`Page.ScreencastFrameMetadata`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct FrameMetadata {
    /// The page's device size in CSS px.
    pub device_width: f64,
    pub device_height: f64,
    pub page_scale_factor: f64,
    /// Top offset of the page content in CSS px.
    pub offset_top: f64,
    /// Page scroll in CSS px.
    pub scroll_offset_x: f64,
    pub scroll_offset_y: f64,
    /// When the frame was swapped, in seconds since the epoch.
    pub timestamp: Option<f64>,
}

impl FrameMetadata {
    pub(crate) fn from_json(value: &Value) -> Self {
        let field = |key: &str| value.get(key).and_then(Value::as_f64);
        Self {
            device_width: field("deviceWidth").unwrap_or(0.0),
            device_height: field("deviceHeight").unwrap_or(0.0),
            page_scale_factor: field("pageScaleFactor").unwrap_or(1.0),
            offset_top: field("offsetTop").unwrap_or(0.0),
            scroll_offset_x: field("scrollOffsetX").unwrap_or(0.0),
            scroll_offset_y: field("scrollOffsetY").unwrap_or(0.0),
            timestamp: field("timestamp"),
        }
    }
}

/// One frame of a stream.
#[derive(Debug, Clone, PartialEq)]
pub struct StreamFrame {
    /// Counts up from 1 per stream, restarts included; a frame that was
    /// replaced before it was taken leaves a gap.
    pub seq: u64,
    /// The encoded JPEG.
    pub jpeg: Vec<u8>,
    pub metadata: FrameMetadata,
}

type Waker = Arc<dyn Fn() + Send + Sync>;

/// Run the consumer's waker; a panic in it must not take the stream
/// thread down before it closed its tab.
fn wake(waker: Option<Waker>) {
    if let Some(waker) = waker {
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| waker()));
    }
}

/// The slot the worker fills and the handle empties.
#[derive(Default)]
pub(crate) struct SlotState {
    latest: Option<StreamFrame>,
    next_seq: u64,
    ended: bool,
    error: Option<BrowserRenderError>,
    waker: Option<Waker>,
}

#[derive(Default)]
pub(crate) struct StreamShared {
    state: Mutex<SlotState>,
    ready: Condvar,
}

impl StreamShared {
    fn lock(&self) -> MutexGuard<'_, SlotState> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Put a frame in the slot, replacing one not taken yet.
    pub(crate) fn publish(&self, jpeg: Vec<u8>, metadata: FrameMetadata) {
        let waker = {
            let mut state = self.lock();
            state.next_seq += 1;
            state.latest = Some(StreamFrame {
                seq: state.next_seq,
                jpeg,
                metadata,
            });
            state.waker.clone()
        };
        self.ready.notify_all();
        wake(waker);
    }

    /// Whether the consumer took the last frame (or none came yet).
    pub(crate) fn is_taken(&self) -> bool {
        self.lock().latest.is_none()
    }

    /// The worker ended, after a failure when `error` is set.
    pub(crate) fn end(&self, error: Option<BrowserRenderError>) {
        let waker = {
            let mut state = self.lock();
            state.ended = true;
            state.error = error;
            state.waker.clone()
        };
        self.ready.notify_all();
        wake(waker);
    }
}

/// A running screencast. See the module docs.
pub struct PageStream {
    shared: Arc<StreamShared>,
    control: Sender<Control>,
    worker: JoinHandle<()>,
    shared_tab: bool,
}

impl PageStream {
    /// Stream `url` in a fresh tab laid out at `viewport`, waiting for the
    /// page to load first (capped; a slow page streams as it stands). The
    /// tab closes when the stream ends.
    pub fn open(
        pool: &BrowserPool,
        url: &str,
        viewport: Viewport,
        settings: StreamSettings,
    ) -> Result<Self, BrowserRenderError> {
        let (mut lease, ws_url) = PoolLease::acquire(pool)?;
        let opened = CdpClient::connect(&ws_url).and_then(|mut client| {
            let mut tab = Tab::open(&mut client)?;
            let loaded = tab
                .emulate(&mut client, &viewport)
                .and_then(|()| tab.navigate(&mut client, url));
            match loaded {
                Ok(()) => Ok((client, tab)),
                Err(error) => {
                    tab.close(&mut client);
                    Err(error)
                }
            }
        });
        match opened {
            Ok((client, tab)) => {
                Self::from_tab(client, tab, TabMode::Owned, settings, Some(lease), None)
            }
            Err(error) => {
                lease.fail(&error);
                Err(error)
            }
        }
    }

    /// Stream the existing tab `target_id` of the browser `generation`
    /// over a second connection; the tab stays open when the stream ends.
    pub(crate) fn attach(
        pool: &BrowserPool,
        target_id: &str,
        generation: u64,
        settings: StreamSettings,
        session: TabStreamClaim,
    ) -> Result<Self, BrowserRenderError> {
        // The tab's browser ended or was replaced meanwhile: the tab is
        // gone. Checked before the hold too, which would start a browser.
        if pool.running_generation() != Some(generation) {
            return Err(BrowserRenderError::TabClosed);
        }
        let (mut lease, ws_url) = PoolLease::acquire(pool)?;
        if lease.generation() != generation {
            return Err(BrowserRenderError::TabClosed);
        }
        let attached = CdpClient::connect(&ws_url).and_then(|mut client| {
            let tab = Tab::attach(&mut client, target_id)?;
            Ok((client, tab))
        });
        match attached {
            Ok((client, tab)) => Self::from_tab(
                client,
                tab,
                TabMode::Shared,
                settings,
                Some(lease),
                Some(session),
            ),
            Err(error) => {
                lease.fail(&error);
                Err(error)
            }
        }
    }

    /// Start the screencast on `tab` and hand `client` to the worker.
    pub(crate) fn from_tab(
        client: CdpClient,
        tab: Tab,
        mode: TabMode,
        settings: StreamSettings,
        lease: Option<PoolLease>,
        session: Option<TabStreamClaim>,
    ) -> Result<Self, BrowserRenderError> {
        let shared = Arc::new(StreamShared::default());
        let (control, inbox) = mpsc::channel();
        let shared_tab = mode == TabMode::Shared;
        let worker = Worker::start(
            client,
            tab,
            mode,
            settings,
            Arc::clone(&shared),
            inbox,
            Holds { lease, session },
        )?;
        let worker = std::thread::Builder::new()
            .name("browser-stream".into())
            .spawn(move || worker.run())
            .map_err(BrowserRenderError::Launch)?;
        Ok(Self {
            shared,
            control,
            worker,
            shared_tab,
        })
    }

    /// The newest frame not taken yet, if one came. Once the stream ended
    /// this returns why (the first time; [`BrowserRenderError::Closed`]
    /// after that). Taking a frame lets the next one be acked.
    pub fn take_frame(&self) -> Result<Option<StreamFrame>, BrowserRenderError> {
        let mut state = self.shared.lock();
        if let Some(frame) = state.latest.take() {
            return Ok(Some(frame));
        }
        if state.ended {
            return Err(state.error.take().unwrap_or(BrowserRenderError::Closed));
        }
        Ok(None)
    }

    /// Like [`Self::take_frame`], waiting up to `timeout` for a frame.
    pub fn wait_frame(&self, timeout: Duration) -> Result<Option<StreamFrame>, BrowserRenderError> {
        let deadline = Instant::now() + timeout;
        let mut state = self.shared.lock();
        loop {
            if let Some(frame) = state.latest.take() {
                return Ok(Some(frame));
            }
            if state.ended {
                return Err(state.error.take().unwrap_or(BrowserRenderError::Closed));
            }
            let now = Instant::now();
            if now >= deadline {
                return Ok(None);
            }
            state = self
                .shared
                .ready
                .wait_timeout(state, deadline - now)
                .map(|(guard, _)| guard)
                .unwrap_or_else(|e| e.into_inner().0);
        }
    }

    /// Call `waker` (on the stream's thread) whenever a frame arrives or
    /// the stream ends, so a consumer can wake its loop instead of polling.
    /// The stream runs before the waker can be set, so when a frame is
    /// already waiting (or the stream already ended) it is called once at
    /// once, on the caller's thread; otherwise that wake-up would be lost.
    pub fn set_frame_waker(&self, waker: impl Fn() + Send + Sync + 'static) {
        let waker: Waker = Arc::new(waker);
        let missed = {
            let mut state = self.shared.lock();
            state.waker = Some(Arc::clone(&waker));
            state.latest.is_some() || state.ended
        };
        if missed {
            wake(Some(waker));
        }
    }

    /// Lay the tab out at `viewport` and restart the screencast. Refused on
    /// a shared tab, whose page session owns the viewport
    /// ([`crate::PageSessions::resize_stream`]).
    pub fn set_viewport(&self, viewport: Viewport) -> Result<(), BrowserRenderError> {
        if self.shared_tab {
            return Err(BrowserRenderError::SharedTabViewport);
        }
        self.request_restart(Some(viewport), None)
    }

    /// Restart the screencast with new `settings` (frame size, quality,
    /// rate). The last frame stays in the slot until a new one comes.
    pub fn set_settings(&self, settings: StreamSettings) -> Result<(), BrowserRenderError> {
        self.request_restart(None, Some(settings))
    }

    /// Restart the screencast as it is (after the tab's viewport changed).
    pub fn restart(&self) -> Result<(), BrowserRenderError> {
        self.request_restart(None, None)
    }

    /// Whether this streams a kept-open MCP tab rather than its own.
    pub fn is_shared_tab(&self) -> bool {
        self.shared_tab
    }

    /// Whether the stream ended (its tab or browser went away).
    pub fn is_ended(&self) -> bool {
        self.shared.lock().ended
    }

    /// Stop the stream and wait until its tab is closed (or detached) and
    /// its hold on the browser released. Dropping the handle does the same
    /// without waiting.
    pub fn stop(self) {
        let Self {
            control, worker, ..
        } = self;
        drop(control);
        let _ = worker.join();
    }

    fn request_restart(
        &self,
        viewport: Option<Viewport>,
        settings: Option<StreamSettings>,
    ) -> Result<(), BrowserRenderError> {
        let (reply, answer) = mpsc::channel();
        let message = Control::Restart {
            viewport,
            settings,
            reply,
        };
        if self.control.send(message).is_err() {
            return Err(self.end_error());
        }
        match answer.recv_timeout(RESTART_TIMEOUT) {
            Ok(result) => result,
            Err(RecvTimeoutError::Timeout) => Err(BrowserRenderError::Timeout {
                method: "screencast restart".into(),
            }),
            Err(RecvTimeoutError::Disconnected) => Err(self.end_error()),
        }
    }

    /// Why the worker ended, or `Closed`; left in place for
    /// [`Self::take_frame`].
    fn end_error(&self) -> BrowserRenderError {
        self.shared
            .lock()
            .error
            .as_ref()
            .map_or(BrowserRenderError::Closed, BrowserRenderError::duplicate)
    }
}

#[cfg(test)]
#[path = "screencast_tests.rs"]
mod tests;
