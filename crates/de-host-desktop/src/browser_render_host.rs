//! Live browser pages on the desktop canvas (#20).
//!
//! The shared [`BrowserRenderQueue`] decides once per redraw which visible
//! browser nodes need a new snapshot; this host renders them on worker
//! threads with the user's headless Chrome (`de_browser_render`, the
//! process-wide browser) and wakes the UI thread with
//! `DesktopEvent::BrowserFrameReady` when one lands. Nothing here runs per
//! paint, and a slow page only ever blocks a worker. The browser process
//! ends after `de_browser_render::IDLE_SHUTDOWN` without renders or open
//! MCP tabs (`start_idle_reaper`), and with the app.

use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use de_browser_render::{BrowserPool, BrowserRenderError, RenderSpec};
use de_editor_core::browser_live::BrowserRenderRequest;
use de_editor_core::EditorState;
use de_editor_ui::widgets::browser_render_queue::{BrowserRenderOutcome, BrowserRenderQueue};
use winit::event_loop::EventLoopProxy;

use crate::DesktopEvent;

/// Pages rendered at the same time.
const WORKERS: usize = 2;

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

type WakeProxy = Arc<Mutex<Option<EventLoopProxy<DesktopEvent>>>>;
/// Test seam: renders a spec without a browser.
type ScriptedRender = Arc<dyn Fn(&RenderSpec) -> Result<Vec<u8>, BrowserRenderError> + Send + Sync>;

pub struct BrowserRenderHost {
    queue: BrowserRenderQueue,
    pool: BrowserPool,
    scripted: Option<ScriptedRender>,
    jobs: Option<Sender<Job>>,
    done_tx: Sender<Done>,
    done_rx: Receiver<Done>,
    wake: WakeProxy,
    clock: Instant,
}

impl BrowserRenderHost {
    pub fn new() -> Self {
        let (done_tx, done_rx) = mpsc::channel();
        Self {
            queue: BrowserRenderQueue::new(WORKERS),
            pool: BrowserPool::shared(),
            scripted: None,
            jobs: None,
            done_tx,
            done_rx,
            wake: Arc::new(Mutex::new(None)),
            clock: Instant::now(),
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

    /// A render waits out its debounce: the loop must wake to send it.
    /// Renders in flight wake it themselves (`BrowserFrameReady`).
    pub fn is_waiting(&self) -> bool {
        self.queue.is_waiting()
    }

    #[cfg(test)]
    fn is_pending(&self) -> bool {
        self.queue.is_pending()
    }

    /// Per-redraw step. Returns whether a repaint is needed (a snapshot or
    /// a status changed).
    pub fn pump(&mut self, state: &mut EditorState) -> bool {
        let mut changed = false;
        while let Ok(done) = self.done_rx.try_recv() {
            let outcome = match done.result {
                Ok(png) => BrowserRenderOutcome::Ready(png),
                Err(error) => {
                    eprintln!("[browser] {}: {error}", done.node_id);
                    match error {
                        BrowserRenderError::NoBrowser => BrowserRenderOutcome::NoBrowser,
                        _ => BrowserRenderOutcome::Failed,
                    }
                }
            };
            self.queue.finish(state, &done.node_id, done.key, outcome);
            changed = true;
        }
        let now_ms = self.clock.elapsed().as_secs_f64() * 1000.0;
        let (status_changed, requests) = self.queue.pump(state, now_ms);
        for request in requests {
            let (node_id, key) = (request.node_id.clone(), request.key);
            if self.sender().send(job_for(request)).is_err() {
                self.queue
                    .finish(state, &node_id, key, BrowserRenderOutcome::Failed);
            }
        }
        changed || status_changed
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
                        Some(script) => worker(move |spec| script(spec), rx, done, wake),
                        None => {
                            de_browser_render::start_idle_reaper();
                            let mut renderer = pool.renderer();
                            worker(move |spec| renderer.render(spec), rx, done, wake)
                        }
                    })
                    .expect("spawn browser render worker");
            }
            tx
        })
    }

    /// Stop taking jobs and end the browser process.
    pub fn shutdown(&mut self) {
        self.jobs = None;
        self.pool.shutdown();
    }
}

impl Drop for BrowserRenderHost {
    /// Stop the workers and end the browser with the app, so no headless
    /// browser outlives it.
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn job_for(request: BrowserRenderRequest) -> Job {
    Job {
        spec: de_host_services::web_browser_snapshot::render_spec(&request.snapshot_spec()),
        node_id: request.node_id,
        key: request.key,
    }
}

/// Render jobs until the host is dropped.
fn worker(
    mut render: impl FnMut(&RenderSpec) -> Result<Vec<u8>, BrowserRenderError>,
    jobs: Arc<Mutex<Receiver<Job>>>,
    done: Sender<Done>,
    wake: WakeProxy,
) {
    loop {
        let next = match jobs.lock() {
            Ok(rx) => rx.recv(),
            Err(_) => return,
        };
        let Ok(job) = next else {
            return;
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
