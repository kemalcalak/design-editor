//! Page snapshots for the live browser node (#20), rendered by the user's
//! installed Chromium-based browser in headless mode over the Chrome
//! DevTools Protocol.
//!
//! A [`BrowserPool`] owns at most one browser process, started on the
//! first render and ended once it sat idle ([`BrowserPool::shutdown_if_idle`]).
//! Each worker thread takes its own [`PageRenderer`]: a DevTools connection
//! of its own to the shared process, so two slow pages load side by side.
//! [`PageSessions`] keep tabs open between calls for the MCP `browser`
//! tool (load a page, click in it, look at it). A [`PageStream`] streams a
//! tab's repaints as JPEG frames (CDP screencast) for a live canvas node,
//! on a fresh tab or on a kept-open one. Every caller in the app
//! shares one process through [`BrowserPool::shared`]. Everything here
//! blocks; callers run it off the UI thread.

mod cdp_client;
mod chrome_discovery;
mod chrome_process;
mod error;
mod page_session;
mod render_job;
mod screencast;
mod screencast_worker;
mod tab;

use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

pub use chrome_discovery::{find_browser, BROWSER_PATH_ENV};
pub use error::BrowserRenderError;
pub use page_session::{check_cdp_method, PageSession, PageSessions, ShotTarget, CDP_DOMAINS};
pub use render_job::RenderSpec;
pub use screencast::{FrameMetadata, PageStream, StreamFrame, StreamSettings};
pub use tab::Viewport;

use cdp_client::CdpClient;
use chrome_process::ChromeProcess;

/// The shared browser ends after this long without renders, open tabs or
/// streams.
pub const IDLE_SHUTDOWN: Duration = Duration::from_secs(60);

/// Start (once per process) the thread that closes kept-open tabs unused
/// for [`page_session::SESSION_IDLE`] and ends the shared browser after
/// [`IDLE_SHUTDOWN`]. Every user of the shared pool calls this.
pub fn start_idle_reaper() {
    static STARTED: std::sync::Once = std::sync::Once::new();
    STARTED.call_once(|| {
        let _ = std::thread::Builder::new()
            .name("browser-idle".into())
            .spawn(|| loop {
                std::thread::sleep(Duration::from_secs(5));
                PageSessions::shared().close_idle();
                BrowserPool::shared().shutdown_if_idle(IDLE_SHUTDOWN);
            });
    });
}

/// The shared browser process and its bookkeeping.
#[derive(Clone, Default)]
pub struct BrowserPool {
    inner: Arc<Mutex<PoolState>>,
}

#[derive(Default)]
struct PoolState {
    /// `None` until discovery ran; `Some(None)` when nothing is installed.
    binary: Option<Option<PathBuf>>,
    process: Option<ChromeProcess>,
    /// Bumped per started process, so renderers drop stale connections.
    generation: u64,
    busy: usize,
    /// Open [`PageSession`]s and running [`PageStream`]s: the browser
    /// stays up while any exists.
    holders: usize,
    last_used: Option<Instant>,
}

impl BrowserPool {
    /// A pool that finds the browser on first use ([`find_browser`]).
    pub fn new() -> Self {
        Self::default()
    }

    /// A pool that launches `binary`.
    pub fn with_binary(binary: PathBuf) -> Self {
        let pool = Self::default();
        pool.lock().binary = Some(Some(binary));
        pool
    }

    /// The pool every caller in the process shares (canvas snapshots, the
    /// MCP `browser` tool, the web daemon), so one browser serves them all.
    pub fn shared() -> BrowserPool {
        static SHARED: std::sync::OnceLock<BrowserPool> = std::sync::OnceLock::new();
        SHARED.get_or_init(BrowserPool::new).clone()
    }

    /// A renderer for one worker thread.
    pub fn renderer(&self) -> PageRenderer {
        PageRenderer {
            pool: self.clone(),
            client: None,
        }
    }

    /// End the browser process when no render is running, no
    /// [`PageSession`] is open, no [`PageStream`] runs and none ran for
    /// `idle`. Returns whether it
    /// was ended.
    pub fn shutdown_if_idle(&self, idle: Duration) -> bool {
        let mut state = self.lock();
        let idle_long_enough = state.last_used.is_some_and(|at| at.elapsed() >= idle);
        if state.busy == 0 && state.holders == 0 && state.process.is_some() && idle_long_enough {
            state.process = None;
            return true;
        }
        false
    }

    /// End the browser process now, even mid-render (the app is quitting;
    /// a render in progress fails with a closed connection).
    pub fn shutdown(&self) {
        self.lock().process = None;
    }

    /// The running browser process's generation, if one runs.
    pub(crate) fn running_generation(&self) -> Option<u64> {
        let state = self.lock();
        state.process.as_ref().map(|_| state.generation)
    }

    /// Whether a browser process is running.
    pub fn is_running(&self) -> bool {
        self.lock().process.is_some()
    }

    fn lock(&self) -> MutexGuard<'_, PoolState> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Mark a render started and hand out the live process endpoint,
    /// starting the browser when none runs.
    fn begin(&self) -> Result<(u64, String), BrowserRenderError> {
        let mut state = self.lock();
        state.busy += 1;
        let result = Self::endpoint(&mut state);
        if result.is_err() {
            state.busy -= 1;
        }
        result
    }

    fn endpoint(state: &mut PoolState) -> Result<(u64, String), BrowserRenderError> {
        if let Some(process) = state.process.as_mut() {
            if process.is_alive() {
                return Ok((state.generation, process.ws_url().to_string()));
            }
            state.process = None;
        }
        let binary = state
            .binary
            .get_or_insert_with(find_browser)
            .clone()
            .ok_or(BrowserRenderError::NoBrowser)?;
        let process = ChromeProcess::spawn(&binary)?;
        let ws_url = process.ws_url().to_string();
        state.process = Some(process);
        state.generation += 1;
        Ok((state.generation, ws_url))
    }

    /// Keep the browser up for a session and hand out its endpoint.
    fn hold(&self) -> Result<(u64, String), BrowserRenderError> {
        let mut state = self.lock();
        let endpoint = Self::endpoint(&mut state)?;
        state.holders += 1;
        Ok(endpoint)
    }

    /// A session closed. When `broken_generation` names the running
    /// process, its connection broke: the process is ended so the next
    /// caller starts a fresh one.
    fn release(&self, broken_generation: Option<u64>) {
        let mut state = self.lock();
        state.holders = state.holders.saturating_sub(1);
        state.last_used = Some(Instant::now());
        if broken_generation == Some(state.generation) {
            state.process = None;
        }
    }

    /// A render finished; `broken_generation` as in [`Self::release`].
    fn end(&self, broken_generation: Option<u64>) {
        let mut state = self.lock();
        state.busy = state.busy.saturating_sub(1);
        state.last_used = Some(Instant::now());
        if broken_generation == Some(state.generation) {
            state.process = None;
        }
    }
}

/// A hold on the pool's browser for a [`PageStream`]: while one exists,
/// [`BrowserPool::shutdown_if_idle`] leaves the browser running. Dropping
/// it releases the hold; a lease marked broken also ends the browser it
/// was taken on (not a newer one).
pub(crate) struct PoolLease {
    pool: BrowserPool,
    generation: u64,
    broken: bool,
}

impl PoolLease {
    /// Hold the browser (starting it when none runs) and hand out its
    /// endpoint.
    pub(crate) fn acquire(pool: &BrowserPool) -> Result<(Self, String), BrowserRenderError> {
        let (generation, ws_url) = pool.hold()?;
        let lease = Self {
            pool: pool.clone(),
            generation,
            broken: false,
        };
        Ok((lease, ws_url))
    }

    /// The browser process this lease holds.
    pub(crate) fn generation(&self) -> u64 {
        self.generation
    }

    /// The connection to this browser broke: end it on release.
    pub(crate) fn mark_broken(&mut self) {
        self.broken = true;
    }

    /// Note a failure on this browser: one that lost the connection
    /// marks the lease broken.
    pub(crate) fn fail(&mut self, error: &BrowserRenderError) {
        if error.needs_restart() {
            self.mark_broken();
        }
    }
}

impl Drop for PoolLease {
    fn drop(&mut self) {
        self.pool.release(self.broken.then_some(self.generation));
    }
}

/// One worker's connection to the pool's browser.
pub struct PageRenderer {
    pool: BrowserPool,
    client: Option<(u64, CdpClient)>,
}

impl PageRenderer {
    /// Render `spec` to PNG bytes. A dropped connection or a dead browser
    /// is restarted once before the error is returned.
    pub fn render(&mut self, spec: &RenderSpec) -> Result<Vec<u8>, BrowserRenderError> {
        match self.render_once(spec) {
            Err(error) if error.needs_restart() => self.render_once(spec),
            result => result,
        }
    }

    fn render_once(&mut self, spec: &RenderSpec) -> Result<Vec<u8>, BrowserRenderError> {
        let (generation, ws_url) = self.pool.begin()?;
        let result = self.render_with(generation, &ws_url, spec);
        let broken = match &result {
            Err(error) if error.needs_restart() => {
                self.client = None;
                Some(generation)
            }
            _ => None,
        };
        self.pool.end(broken);
        result
    }

    fn render_with(
        &mut self,
        generation: u64,
        ws_url: &str,
        spec: &RenderSpec,
    ) -> Result<Vec<u8>, BrowserRenderError> {
        if self.client.as_ref().map(|(g, _)| *g) != Some(generation) {
            self.client = Some((generation, CdpClient::connect(ws_url)?));
        }
        let (_, client) = self.client.as_mut().expect("connected above");
        render_job::render_page(client, spec)
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
