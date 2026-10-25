//! Page snapshots for the live browser node (#20), rendered by the user's
//! installed Chromium-based browser in headless mode over the Chrome
//! DevTools Protocol.
//!
//! A [`BrowserPool`] owns at most one browser process, started on the
//! first render and ended once it sat idle ([`BrowserPool::shutdown_if_idle`]).
//! Each worker thread takes its own [`PageRenderer`]: a DevTools connection
//! of its own to the shared process, so two slow pages load side by side.
//! Everything here blocks; callers run it off the UI thread.

mod cdp_client;
mod chrome_discovery;
mod chrome_process;
mod error;
mod render_job;

use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

pub use chrome_discovery::{find_browser, BROWSER_PATH_ENV};
pub use error::BrowserRenderError;
pub use render_job::RenderSpec;

use cdp_client::CdpClient;
use chrome_process::ChromeProcess;

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

    /// A renderer for one worker thread.
    pub fn renderer(&self) -> PageRenderer {
        PageRenderer {
            pool: self.clone(),
            client: None,
        }
    }

    /// End the browser process when no render ran for `idle`. Returns
    /// whether it was ended.
    pub fn shutdown_if_idle(&self, idle: Duration) -> bool {
        let mut state = self.lock();
        let idle_long_enough = state.last_used.is_some_and(|at| at.elapsed() >= idle);
        if state.busy == 0 && state.process.is_some() && idle_long_enough {
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

    fn end(&self, broken_generation: Option<u64>) {
        let mut state = self.lock();
        state.busy = state.busy.saturating_sub(1);
        state.last_used = Some(Instant::now());
        if broken_generation == Some(state.generation) {
            state.process = None;
        }
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
