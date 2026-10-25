//! Live browser pages on the desktop canvas (#20).
//!
//! The canvas painter records which browser nodes its last page frame
//! showed (`browser_textures`); [`BrowserRenderHost::pump`] runs once per
//! redraw, turns each visible node into a render request
//! (`de_editor_core::browser_live`) and, when the request's key differs from
//! the snapshot on screen, hands it to a worker thread. Workers render with
//! the user's headless Chrome (`de_browser_render`), the PNG lands in the
//! shared image byte cache like a fetched image, and the UI thread is woken
//! with `DesktopEvent::BrowserFrameReady`. Nothing here runs per paint, and
//! a slow page only ever blocks a worker.
//!
//! - Off-screen nodes are never requested (the painter never notes them).
//! - A changed request waits [`DEBOUNCE`] so typing an address or dragging
//!   a size does not start a render per step; a node with no snapshot yet,
//!   or a Reload, starts at once. The old snapshot stays up until the new one lands.
//! - A failed key is not retried until the request changes or the user
//!   presses Reload.
//! - The browser process ends after [`IDLE_SHUTDOWN`] without renders.

use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use de_browser_render::{BrowserPool, BrowserRenderError, RenderSpec};
use de_editor_core::browser_live::{
    browser_render_request, BrowserPageStatus, BrowserRenderRequest,
};
use de_editor_core::{EditorState, NodeId};
use de_editor_ui::widgets::browser_textures;
use de_editor_ui::widgets::canvas_viewport_image::{
    has_cached_image_bytes, store_remote_image_bytes,
};
use winit::event_loop::EventLoopProxy;

use crate::DesktopEvent;

/// Pages rendered at the same time.
const WORKERS: usize = 2;
/// Quiet time after a change before its page renders.
const DEBOUNCE: Duration = Duration::from_millis(300);
/// The browser process ends after this long without a render.
const IDLE_SHUTDOWN: Duration = Duration::from_secs(60);
/// How often an idle worker checks the idle shutdown.
const IDLE_POLL: Duration = Duration::from_secs(5);

struct Job {
    node_id: String,
    key: u64,
    spec: RenderSpec,
}

struct Done {
    node_id: String,
    key: u64,
    result: Result<Vec<u8>, BrowserRenderError>,
}

/// Inputs of a scan of the visible nodes: document revision, reloads and
/// the load gate. The visible set itself is compared separately.
#[derive(Clone, Copy, PartialEq, Eq)]
struct ScanStamp {
    revision: u64,
    reload_generation: u64,
    pages_allowed: bool,
}

type WakeProxy = Arc<Mutex<Option<EventLoopProxy<DesktopEvent>>>>;
/// Test seam: renders a spec without a browser.
type ScriptedRender = Arc<dyn Fn(&RenderSpec) -> Result<Vec<u8>, BrowserRenderError> + Send + Sync>;

pub struct BrowserRenderHost {
    pool: BrowserPool,
    scripted: Option<ScriptedRender>,
    jobs: Option<Sender<Job>>,
    done_tx: Sender<Done>,
    done_rx: Receiver<Done>,
    wake: WakeProxy,
    /// The browser nodes the last painted page frame showed.
    visible: Vec<String>,
    /// Requests waiting for their debounce, by node.
    wanted: HashMap<String, (BrowserRenderRequest, Instant)>,
    /// Key being rendered, by node.
    in_flight: HashMap<String, u64>,
    /// Last key that failed, by node (not retried).
    failed: HashMap<String, u64>,
    /// Reload count last seen, by node: a new Reload skips the debounce.
    reloads_seen: HashMap<String, u32>,
    /// What the last scan of the visible nodes saw; an unchanged stamp
    /// skips the scan (no document walk on a quiet frame).
    last_scan: Option<ScanStamp>,
    document_generation: u64,
}

impl BrowserRenderHost {
    pub fn new() -> Self {
        let (done_tx, done_rx) = mpsc::channel();
        Self {
            pool: BrowserPool::new(),
            scripted: None,
            jobs: None,
            done_tx,
            done_rx,
            wake: Arc::new(Mutex::new(None)),
            visible: Vec::new(),
            wanted: HashMap::new(),
            in_flight: HashMap::new(),
            failed: HashMap::new(),
            reloads_seen: HashMap::new(),
            last_scan: None,
            document_generation: 0,
        }
    }

    /// Test seam: render with `script` instead of a browser.
    #[cfg(test)]
    fn with_script(script: ScriptedRender) -> Self {
        let mut host = Self::new();
        host.scripted = Some(script);
        host
    }

    pub fn set_wake_proxy(&mut self, proxy: EventLoopProxy<DesktopEvent>) {
        if let Ok(mut wake) = self.wake.lock() {
            *wake = Some(proxy);
        }
    }

    /// Renders in flight or waiting out their debounce: keeps the event
    /// loop waking until they land.
    pub fn is_pending(&self) -> bool {
        !self.in_flight.is_empty() || !self.wanted.is_empty()
    }

    /// Per-redraw step. Returns whether a repaint is needed (a snapshot or
    /// a status changed).
    pub fn pump(&mut self, state: &mut EditorState) -> bool {
        if state.document_generation() != self.document_generation {
            // A new document: its node ids name other nodes.
            self.document_generation = state.document_generation();
            browser_textures::clear_snapshots();
            self.wanted.clear();
            self.failed.clear();
            self.reloads_seen.clear();
            self.visible.clear();
            self.last_scan = None;
        }
        let landed = self.drain_results(state);
        let mut changed = landed;
        let mut visible_changed = false;
        if let Some(visible) = browser_textures::take_visible() {
            if visible != self.visible {
                self.visible = visible;
                self.wanted
                    .retain(|id, _| self.visible.binary_search(id).is_ok());
                visible_changed = true;
            }
        }
        let now = Instant::now();
        let live = &state.editor_ui.browser_live;
        let stamp = ScanStamp {
            revision: state.document_revision(),
            reload_generation: live.reload_generation(),
            pages_allowed: live.pages_allowed(),
        };
        let quiet = !landed && !visible_changed && self.last_scan == Some(stamp);
        self.last_scan = Some(stamp);
        if quiet || self.visible.is_empty() || !stamp.pages_allowed {
            changed |= self.dispatch_due(state, now);
            return changed;
        }
        for id in &self.visible {
            let Some(node) =
                de_editor_core::walkers::find_node(state.active_children(), &NodeId::new(id))
            else {
                continue;
            };
            let reload = state.editor_ui.browser_live.reload_count(id);
            let reloaded = self.reloads_seen.insert(id.clone(), reload) != Some(reload);
            let Some(request) = browser_render_request(node, reload) else {
                continue;
            };
            let shown = browser_textures::ready(id)
                .filter(|(_, texture)| has_cached_image_bytes(*texture))
                .map(|(key, _)| key);
            if shown == Some(request.key)
                || self.in_flight.get(id) == Some(&request.key)
                || self.failed.get(id) == Some(&request.key)
            {
                continue;
            }
            match self.wanted.get(id) {
                Some((queued, _)) if queued.key == request.key => {}
                _ => {
                    // A first picture or a Reload starts at once; any other
                    // change waits out the debounce.
                    let due = if shown.is_none() || reloaded {
                        now
                    } else {
                        now + DEBOUNCE
                    };
                    self.wanted.insert(id.clone(), (request, due));
                }
            }
        }
        changed |= self.dispatch_due(state, now);
        changed
    }

    fn drain_results(&mut self, state: &mut EditorState) -> bool {
        let mut changed = false;
        while let Ok(done) = self.done_rx.try_recv() {
            if self.in_flight.get(&done.node_id) == Some(&done.key) {
                self.in_flight.remove(&done.node_id);
            }
            let status = match done.result {
                Ok(png) => {
                    let texture = browser_textures::texture_id(&done.node_id, done.key);
                    store_remote_image_bytes(texture, png);
                    browser_textures::set_ready(&done.node_id, done.key);
                    self.failed.remove(&done.node_id);
                    BrowserPageStatus::Ready
                }
                Err(error) => {
                    eprintln!("[browser] {}: {error}", done.node_id);
                    self.failed.insert(done.node_id.clone(), done.key);
                    match error {
                        BrowserRenderError::NoBrowser => BrowserPageStatus::NoBrowser,
                        _ => BrowserPageStatus::Failed,
                    }
                }
            };
            state
                .editor_ui
                .browser_live
                .set_status(&done.node_id, status);
            changed = true;
        }
        changed
    }

    fn dispatch_due(&mut self, state: &mut EditorState, now: Instant) -> bool {
        if self.wanted.is_empty() {
            return false;
        }
        let mut due: Vec<String> = self
            .wanted
            .iter()
            .filter(|(id, (_, at))| *at <= now && !self.in_flight.contains_key(*id))
            .map(|(id, _)| id.clone())
            .collect();
        due.sort_unstable();
        let mut changed = false;
        for id in due {
            if self.in_flight.len() >= WORKERS {
                break;
            }
            let Some((request, _)) = self.wanted.remove(&id) else {
                continue;
            };
            let job = Job {
                node_id: id.clone(),
                key: request.key,
                spec: RenderSpec {
                    url: request.url,
                    layout_width: request.layout_width,
                    layout_height: request.layout_height,
                    scale: request.scale,
                    mobile: request.mobile,
                    scroll_x: request.scroll_x,
                    scroll_y: request.scroll_y,
                },
            };
            if self.sender().send(job).is_err() {
                continue;
            }
            self.in_flight.insert(id.clone(), request.key);
            state
                .editor_ui
                .browser_live
                .set_status(&id, BrowserPageStatus::Loading);
            changed = true;
        }
        changed
    }

    /// The job queue, starting the workers on first use.
    fn sender(&mut self) -> &Sender<Job> {
        self.jobs.get_or_insert_with(|| {
            let (tx, rx) = mpsc::channel::<Job>();
            let rx = Arc::new(Mutex::new(rx));
            for _ in 0..WORKERS {
                let rx = rx.clone();
                let done = self.done_tx.clone();
                let wake = self.wake.clone();
                let pool = self.pool.clone();
                let scripted = self.scripted.clone();
                std::thread::Builder::new()
                    .name("browser-render".into())
                    .spawn(move || match scripted {
                        Some(script) => worker(move |spec| script(spec), || {}, rx, done, wake),
                        None => {
                            let mut renderer = pool.renderer();
                            let idle = move || {
                                pool.shutdown_if_idle(IDLE_SHUTDOWN);
                            };
                            worker(move |spec| renderer.render(spec), idle, rx, done, wake)
                        }
                    })
                    .expect("spawn browser render worker");
            }
            tx
        })
    }
}

impl Drop for BrowserRenderHost {
    /// Stop the workers and end the browser with the app, so no headless
    /// browser outlives it.
    fn drop(&mut self) {
        self.shutdown();
    }
}

impl BrowserRenderHost {
    /// Stop taking jobs and end the browser process.
    pub fn shutdown(&mut self) {
        self.jobs = None;
        self.pool.shutdown();
    }
}

/// Render jobs until the host is dropped; end the browser when idle.
fn worker(
    mut render: impl FnMut(&RenderSpec) -> Result<Vec<u8>, BrowserRenderError>,
    on_idle: impl Fn(),
    jobs: Arc<Mutex<Receiver<Job>>>,
    done: Sender<Done>,
    wake: WakeProxy,
) {
    loop {
        let next = match jobs.lock() {
            Ok(rx) => rx.recv_timeout(IDLE_POLL),
            Err(_) => return,
        };
        let job = match next {
            Ok(job) => job,
            Err(RecvTimeoutError::Timeout) => {
                on_idle();
                continue;
            }
            Err(RecvTimeoutError::Disconnected) => return,
        };
        let result = render(&job.spec);
        let finished = Done {
            node_id: job.node_id,
            key: job.key,
            result,
        };
        if done.send(finished).is_err() {
            return;
        }
        if let Ok(wake) = wake.lock() {
            if let Some(proxy) = wake.as_ref() {
                let _ = proxy.send_event(DesktopEvent::BrowserFrameReady);
            }
        }
    }
}

#[cfg(test)]
#[path = "browser_render_host_tests.rs"]
mod tests;
