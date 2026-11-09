//! Live browser pages on the desktop canvas (#324).
//!
//! The shared [`BrowserStreamQueue`] decides once per redraw which browser
//! nodes stream: the selected (or interactive) ones on screen (at most
//! `browser_stream::MAX_LIVE_PAGES`, at `DESKTOP_STREAM_FPS`); the others
//! keep their still snapshot. This host
//! runs one driver thread per streaming node (`browser_live_driver`). A
//! driver owns the node's `de_browser_render::PageStream` (on the MCP tab
//! kept for the node when the agent opened one, else on a tab of its
//! own), decodes each JPEG off the UI thread and wakes the loop with
//! `DesktopEvent::BrowserFrameReady`. The UI thread then installs the
//! newest frame under the node's stable texture id
//! (`browser_textures::live_texture`), which repaints only the node's
//! region of the retained canvas layer. A driver hands over its next frame
//! only after the UI took the last one, so a busy UI slows the stream
//! instead of queueing frames.
//!
//! A deselected or scrolled-away node's stream stops and its last frame
//! stays up until the next still snapshot replaces it
//! (`browser_textures::end_live`); so do the streams while the window is
//! unfocused or hidden, and focus starts them again. With the "Live pages" setting off the stills show at once.

#[path = "browser_live_driver.rs"]
mod driver;
#[path = "browser_live_input.rs"]
mod input;

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};

use de_browser_render::BrowserPool;
use de_editor_core::browser_input::BrowserInputEvent;
use de_editor_core::browser_stream::{BrowserStreamSpec, StreamHost};
use de_editor_core::EditorState;
use de_editor_ui::widgets::browser_render_queue::BrowserRenderQueue;
use de_editor_ui::widgets::browser_stream_queue::{BrowserStreamCommand, BrowserStreamQueue};
use de_editor_ui::widgets::browser_textures;
use de_host_native::NativeBackend;
use skia_safe::Sendable;
use winit::event_loop::EventLoopProxy;

use crate::DesktopEvent;
pub(crate) use driver::DriverControl;

type WakeProxy = Arc<Mutex<Option<EventLoopProxy<DesktopEvent>>>>;

/// One decoded frame on its way to the UI thread.
pub(crate) struct LiveFrame {
    pub node_id: String,
    /// The driver it came from; a frame of a stopped stream is dropped.
    pub stream: u64,
    pub seq: u64,
    pub image: Sendable<skia_safe::Image>,
}

/// What a driver thread is started with.
pub(crate) struct DriverArgs {
    pub node_id: String,
    pub stream: u64,
    pub spec: BrowserStreamSpec,
    pub control: Receiver<DriverControl>,
    pub frames: Sender<LiveFrame>,
    /// Set by the UI thread once it installed the last frame.
    pub taken: Arc<AtomicBool>,
    pub wake: WakeProxy,
    pub pool: BrowserPool,
    /// Wakes the driver's frame wait when the UI sends page input.
    pub interrupt: InterruptSlot,
}

/// The running stream's wake handle, set by its driver.
pub(crate) type InterruptSlot = Arc<Mutex<Option<de_browser_render::StreamInterrupt>>>;

/// Starts a driver; a test seam.
type Spawn = Arc<dyn Fn(DriverArgs) + Send + Sync>;

struct Running {
    stream: u64,
    control: Sender<DriverControl>,
    taken: Arc<AtomicBool>,
    interrupt: InterruptSlot,
}

pub struct BrowserLiveHost {
    queue: BrowserStreamQueue,
    running: HashMap<String, Running>,
    next_stream: u64,
    frames_tx: Sender<LiveFrame>,
    frames_rx: Receiver<LiveFrame>,
    wake: WakeProxy,
    pool: BrowserPool,
    spawn: Spawn,
}

impl BrowserLiveHost {
    pub fn new() -> Self {
        Self::with_spawn(Arc::new(driver::spawn))
    }

    fn with_spawn(spawn: Spawn) -> Self {
        let (frames_tx, frames_rx) = mpsc::channel();
        Self {
            queue: BrowserStreamQueue::new(StreamHost::Desktop),
            running: HashMap::new(),
            next_stream: 0,
            frames_tx,
            frames_rx,
            wake: Arc::new(Mutex::new(None)),
            pool: BrowserPool::shared(),
            spawn,
        }
    }

    pub fn set_wake_proxy(&mut self, proxy: EventLoopProxy<DesktopEvent>) {
        if let Ok(mut wake) = self.wake.lock() {
            *wake = Some(proxy);
        }
    }

    /// Before paint: install the frames the drivers decoded. Returns
    /// whether one landed (the canvas must repaint).
    pub fn install_frames(&mut self, backend: &mut NativeBackend) -> bool {
        let mut landed = false;
        while let Ok(frame) = self.frames_rx.try_recv() {
            let Some(running) = self.running.get(&frame.node_id) else {
                continue;
            };
            if running.stream != frame.stream {
                continue;
            }
            let texture = browser_textures::live_texture(&frame.node_id);
            let restarting = browser_textures::live_for_paint(&frame.node_id)
                .is_none_or(|(_, streaming)| !streaming);
            if restarting {
                // A new or kept slot: its box is in no cached canvas layer's
                // live regions yet, so this frame must reach the layer
                // through a raster-generation bump.
                backend.drop_live_raster(texture);
            }
            backend.install_live_raster(texture, frame.image.into_inner());
            browser_textures::set_live_frame(&frame.node_id, frame.seq);
            running.taken.store(true, Ordering::Release);
            let _ = running.control.send(DriverControl::Taken);
            landed = true;
        }
        landed
    }

    /// After paint (and the snapshot queue's pump): start, change and stop
    /// streams for what the frame showed. `active` is false while the
    /// window is unfocused or hidden. Returns whether a slot was dropped
    /// (the canvas must repaint with the snapshot).
    pub fn pump(
        &mut self,
        state: &EditorState,
        snapshots: &BrowserRenderQueue,
        dpi: f32,
        active: bool,
        backend: &mut NativeBackend,
    ) -> bool {
        let mut changed = false;
        let live = &state.editor_ui.browser_live;
        if !live.live_pages() || !live.pages_allowed() {
            // Kept last frames give way to the stills too.
            for texture in browser_textures::forget_kept_live() {
                backend.drop_live_raster(texture);
                changed = true;
            }
        }
        for command in self.queue.pump(state, snapshots, dpi, active) {
            match command {
                BrowserStreamCommand::Start { node_id, spec } => self.start(node_id, spec),
                BrowserStreamCommand::Update {
                    node_id,
                    spec,
                    reload,
                } => {
                    if let Some(running) = self.running.get(&node_id) {
                        let _ = running.control.send(DriverControl::Update { spec, reload });
                    }
                }
                BrowserStreamCommand::Stop {
                    node_id,
                    keep_frame,
                } => {
                    // Dropping the control channel ends the driver, which
                    // stops its stream on its own thread.
                    self.running.remove(&node_id);
                    if keep_frame {
                        browser_textures::end_live(&node_id);
                    } else {
                        changed |= forget(&node_id, backend);
                    }
                }
            }
        }
        changed
    }

    /// Nodes streaming now.
    #[cfg(test)]
    fn streaming(&self) -> usize {
        self.running.len()
    }

    fn start(&mut self, node_id: String, spec: BrowserStreamSpec) {
        self.next_stream += 1;
        let (control, inbox) = mpsc::channel();
        let taken = Arc::new(AtomicBool::new(true));
        let interrupt = InterruptSlot::default();
        self.running.insert(
            node_id.clone(),
            Running {
                stream: self.next_stream,
                control,
                taken: taken.clone(),
                interrupt: interrupt.clone(),
            },
        );
        (self.spawn)(DriverArgs {
            node_id,
            stream: self.next_stream,
            spec,
            control: inbox,
            frames: self.frames_tx.clone(),
            taken,
            wake: self.wake.clone(),
            pool: self.pool.clone(),
            interrupt,
        });
    }

    /// Hand page input (interaction mode) to `node_id`'s driver, which
    /// sends it on the node's stream tab. False when the node does not
    /// stream (the input is dropped: the page is not on screen).
    pub fn send_input(&self, node_id: &str, events: Vec<BrowserInputEvent>) -> bool {
        let Some(running) = self.running.get(node_id) else {
            return false;
        };
        if running.control.send(DriverControl::Input(events)).is_err() {
            return false;
        }
        if let Some(handle) = running.interrupt.lock().ok().and_then(|slot| slot.clone()) {
            handle.interrupt();
        }
        true
    }

    /// End every stream (app exit).
    pub fn shutdown(&mut self) {
        self.running.clear();
    }
}

/// Drop `node_id`'s live frame so its snapshot shows again. Returns
/// whether there was one.
fn forget(node_id: &str, backend: &mut NativeBackend) -> bool {
    match browser_textures::forget_live(node_id) {
        Some(texture) => {
            backend.drop_live_raster(texture);
            true
        }
        None => false,
    }
}

/// Serializes the desktop tests that paint or drain the process-wide
/// browser registry (`browser_textures`).
#[cfg(test)]
pub(crate) fn browser_registry_test_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

#[cfg(test)]
#[path = "browser_live_host_tests.rs"]
mod tests;
