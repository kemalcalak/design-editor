//! Tabs kept open between calls, for the MCP `browser` tool: a page loaded
//! once is clicked, read and captured by later calls. One tab per key (a
//! browser node's id, or the tool's own page); a tab unused for
//! [`SESSION_IDLE`] is closed, unless a [`PageStream`] streams it (the
//! idle time then counts from the stream's end). While any tab is open the
//! shared browser stays up.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::cdp_client::CdpClient;
use crate::tab::{decode_png, Clip, Tab, Viewport, MAX_FULL_PAGE_HEIGHT};
use crate::{BrowserPool, BrowserRenderError, PageStream, StreamSettings};

/// A tab unused this long is closed.
pub const SESSION_IDLE: Duration = Duration::from_secs(5 * 60);
/// The DevTools domains `cdp` may call (pen.dev's set).
pub const CDP_DOMAINS: [&str; 7] = [
    "DOM",
    "Input",
    "Page",
    "Runtime",
    "Emulation",
    "CSS",
    "Accessibility",
];
/// After a `cdp` command, how long to look for a navigation it started.
const NAVIGATION_WINDOW: Duration = Duration::from_millis(300);
/// Longest `return-element` HTML, in bytes.
const MAX_ELEMENT_HTML: usize = 100 * 1024;
/// Widest full-page capture, in CSS px.
const MAX_FULL_PAGE_WIDTH: f64 = 4096.0;
/// Script time for reads and the snapshot extractor.
const SCRIPT_TIMEOUT: Duration = Duration::from_secs(30);

/// What a screenshot covers.
#[derive(Debug, Clone, PartialEq)]
pub enum ShotTarget {
    /// The whole page, up to 4096 × 8000 CSS px.
    FullPage,
    /// The first element matching a CSS selector.
    Query(String),
}

/// One kept-open tab.
pub struct PageSession {
    client: CdpClient,
    tab: Tab,
    /// The browser process the tab lives in (see `BrowserPool::release`).
    generation: u64,
    viewport: Viewport,
    url: Option<String>,
    last_used: Instant,
    /// Streams of this tab; the tab is not idle while one runs.
    streams: TabStreams,
}

/// The streams running on one kept-open tab.
#[derive(Clone, Default)]
pub(crate) struct TabStreams(Arc<Mutex<LeaseState>>);

#[derive(Default)]
struct LeaseState {
    live: usize,
    /// When the last stream ended.
    released_at: Option<Instant>,
}

impl TabStreams {
    fn lock(&self) -> std::sync::MutexGuard<'_, LeaseState> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub(crate) fn acquire(&self) -> TabStreamClaim {
        self.lock().live += 1;
        TabStreamClaim(self.clone())
    }
}

/// One stream's claim on a kept-open tab; dropping it restarts the tab's
/// idle clock.
pub(crate) struct TabStreamClaim(TabStreams);

impl Drop for TabStreamClaim {
    fn drop(&mut self) {
        let mut state = self.0.lock();
        state.live = state.live.saturating_sub(1);
        state.released_at = Some(Instant::now());
    }
}

/// Whether a tab last used at `last_used` is idle: no stream runs on it
/// and `idle` passed since its last use or its last stream's end.
pub(crate) fn session_is_idle(last_used: Instant, streams: &TabStreams, idle: Duration) -> bool {
    let state = streams.lock();
    if state.live > 0 {
        return false;
    }
    let since = state.released_at.map_or(last_used, |at| at.max(last_used));
    since.elapsed() >= idle
}

/// How a session leaves [`PageSessions`].
#[derive(Clone, Copy, PartialEq, Eq)]
enum Leave {
    /// Close its tab; the browser stays.
    Close,
    /// Its connection broke: end the browser it was on.
    Broken,
}

impl PageSession {
    fn open(pool: &BrowserPool, viewport: Viewport) -> Result<Self, BrowserRenderError> {
        let (generation, ws_url) = pool.hold()?;
        let opened = CdpClient::connect(&ws_url).and_then(|mut client| {
            let mut tab = Tab::open(&mut client)?;
            tab.emulate(&mut client, &viewport)?;
            Ok((client, tab))
        });
        match opened {
            Ok((client, tab)) => Ok(Self {
                client,
                tab,
                generation,
                viewport,
                url: None,
                last_used: Instant::now(),
                streams: TabStreams::default(),
            }),
            Err(error) => {
                pool.release(error.needs_restart().then_some(generation));
                Err(error)
            }
        }
    }

    /// The address last loaded with [`Self::load`], `None` before any. A
    /// `cdp` navigation does not change it.
    pub fn url(&self) -> Option<&str> {
        self.url.as_deref()
    }

    /// Load `url` and wait for it (capped; a slow page is used as it stands).
    pub fn load(&mut self, url: &str) -> Result<(), BrowserRenderError> {
        self.tab.navigate(&mut self.client, url)?;
        self.url = Some(url.to_string());
        Ok(())
    }

    fn set_viewport(&mut self, viewport: Viewport) -> Result<(), BrowserRenderError> {
        if self.viewport != viewport {
            self.tab.emulate(&mut self.client, &viewport)?;
            self.viewport = viewport;
        }
        Ok(())
    }

    /// Capture the whole page or one element at 1×.
    pub fn screenshot(&mut self, target: &ShotTarget) -> Result<Vec<u8>, BrowserRenderError> {
        let clip = match target {
            ShotTarget::FullPage => {
                let (width, height) = self.tab.page_size(&mut self.client)?;
                Clip {
                    x: 0.0,
                    y: 0.0,
                    width: width.min(MAX_FULL_PAGE_WIDTH),
                    height: height.min(MAX_FULL_PAGE_HEIGHT),
                    scale: 1.0,
                }
            }
            ShotTarget::Query(selector) => self.element_box(selector)?,
        };
        self.tab.screenshot(&mut self.client, Some(clip))
    }

    /// Capture what the viewport shows now at `scale` output pixels per
    /// CSS pixel: the picture a canvas node shows of this tab.
    pub fn view_snapshot(&mut self, scale: f64) -> Result<Vec<u8>, BrowserRenderError> {
        let view = self.tab.evaluate(
            &mut self.client,
            "[scrollX, scrollY, innerWidth, innerHeight]",
            SCRIPT_TIMEOUT,
        )?;
        let numbers: Vec<f64> = view
            .as_array()
            .map(|items| items.iter().filter_map(Value::as_f64).collect())
            .unwrap_or_default();
        let [x, y, width, height] = numbers[..] else {
            return self.tab.screenshot(&mut self.client, None);
        };
        let clip = Clip {
            x,
            y,
            width,
            height,
            scale,
        };
        self.tab.screenshot(&mut self.client, Some(clip))
    }

    /// The element's HTML (capped) and the computed style values that
    /// differ from a bare element of the same tag, as JSON.
    pub fn element(&mut self, selector: &str) -> Result<Value, BrowserRenderError> {
        let script = format!(
            "(() => {{ const e = document.querySelector({sel}); if (!e) return null; \
               const bare = document.createElement(e.tagName); \
               bare.style.cssText = 'position:absolute;visibility:hidden;'; \
               (e.parentElement || document.documentElement).appendChild(bare); \
               const cs = getComputedStyle(e), base = getComputedStyle(bare), style = {{}}; \
               for (const name of cs) {{ const v = cs.getPropertyValue(name); \
                 if (v !== base.getPropertyValue(name)) style[name] = v; }} \
               bare.remove(); const html = e.outerHTML; \
               return {{ html: html.slice(0, {max}), truncated: html.length > {max}, style }}; }})()",
            sel = js_string(selector),
            max = MAX_ELEMENT_HTML,
        );
        let value = self
            .tab
            .evaluate(&mut self.client, &script, SCRIPT_TIMEOUT)?;
        if value.is_null() {
            return Err(no_match(selector));
        }
        Ok(value)
    }

    /// Run `expression` in the page and return its JSON value.
    pub fn evaluate(&mut self, expression: &str) -> Result<Value, BrowserRenderError> {
        self.tab
            .evaluate(&mut self.client, expression, SCRIPT_TIMEOUT)
    }

    /// Send one DevTools command from [`CDP_DOMAINS`]. A command that
    /// started a navigation (a clicked link) returns once the load ends.
    pub fn cdp(&mut self, method: &str, params: Value) -> Result<Value, BrowserRenderError> {
        check_cdp_method(method)?;
        // Only events this command causes may end the waits below.
        self.tab.drop_events(&mut self.client);
        let reply = self.tab.command(&mut self.client, method, params)?;
        if self
            .tab
            .navigation_started(&mut self.client, NAVIGATION_WINDOW)?
        {
            self.tab.wait_load(&mut self.client)?;
        }
        Ok(reply)
    }

    /// The PNG in a `Page.captureScreenshot` reply returned by [`Self::cdp`].
    pub fn screenshot_reply_png(reply: &Value) -> Result<Vec<u8>, BrowserRenderError> {
        decode_png(reply)
    }

    fn element_box(&mut self, selector: &str) -> Result<Clip, BrowserRenderError> {
        let script = format!(
            "(() => {{ const e = document.querySelector({}); if (!e) return null; \
               const r = e.getBoundingClientRect(); \
               return [r.left + scrollX, r.top + scrollY, r.width, r.height]; }})()",
            js_string(selector)
        );
        let value = self
            .tab
            .evaluate(&mut self.client, &script, SCRIPT_TIMEOUT)?;
        let numbers: Vec<f64> = value
            .as_array()
            .map(|items| items.iter().filter_map(Value::as_f64).collect())
            .unwrap_or_default();
        match numbers[..] {
            [x, y, width, height] => Ok(Clip {
                x,
                y,
                width: width.min(MAX_FULL_PAGE_WIDTH),
                height: height.min(MAX_FULL_PAGE_HEIGHT),
                scale: 1.0,
            }),
            _ => Err(no_match(selector)),
        }
    }

    fn close(self) {
        let mut client = self.client;
        self.tab.close(&mut client);
    }
}

/// Refuse a `cdp` method outside [`CDP_DOMAINS`] (Browser, Target,
/// Network, Storage … stay out of reach).
pub fn check_cdp_method(method: &str) -> Result<(), BrowserRenderError> {
    match method.split_once('.') {
        Some((domain, name)) if CDP_DOMAINS.contains(&domain) && !name.is_empty() => Ok(()),
        _ => Err(BrowserRenderError::MethodRefused {
            method: method.to_string(),
            allowed: CDP_DOMAINS.join(", "),
        }),
    }
}

fn no_match(selector: &str) -> BrowserRenderError {
    BrowserRenderError::NoMatch {
        selector: selector.to_string(),
    }
}

/// `text` as a JavaScript string literal.
fn js_string(text: &str) -> String {
    serde_json::to_string(text).unwrap_or_else(|_| "\"\"".into())
}

/// The kept-open tabs, by key.
pub struct PageSessions {
    pool: BrowserPool,
    sessions: Mutex<HashMap<String, PageSession>>,
}

impl PageSessions {
    pub fn new(pool: BrowserPool) -> Self {
        Self {
            pool,
            sessions: Mutex::new(HashMap::new()),
        }
    }

    /// The process-wide tabs, on [`BrowserPool::shared`].
    pub fn shared() -> &'static PageSessions {
        static SHARED: OnceLock<PageSessions> = OnceLock::new();
        SHARED.get_or_init(|| PageSessions::new(BrowserPool::shared()))
    }

    /// Run `f` on the tab kept for `key`, opening it at `viewport` first
    /// (an open tab is switched to `viewport`). A tab whose browser went
    /// away is reopened once, at its last address.
    pub fn with<R>(
        &self,
        key: &str,
        viewport: Viewport,
        f: impl FnMut(&mut PageSession) -> Result<R, BrowserRenderError>,
    ) -> Result<R, BrowserRenderError> {
        self.run_on(key, viewport, true, f)
    }

    /// Stream the tab kept for `key` over a second connection (the canvas
    /// shows what the MCP `browser` tool drives). `None` when no tab is
    /// kept for `key`, or a call holds it right now (try again later). The
    /// tab is not closed for idleness while the stream runs. Its viewport
    /// stays the session's: after a [`Self::with`] that may change it, use
    /// [`Self::resize_stream`] so the stream restarts at the new size.
    pub fn stream(
        &self,
        key: &str,
        settings: StreamSettings,
    ) -> Result<Option<PageStream>, BrowserRenderError> {
        let (target_id, generation, lease) = {
            let sessions = self.lock();
            let Some(session) = sessions.get(key) else {
                return Ok(None);
            };
            (
                session.tab.target_id().to_string(),
                session.generation,
                session.streams.acquire(),
            )
        };
        PageStream::attach(&self.pool, &target_id, generation, settings, lease).map(Some)
    }

    /// Lay the tab kept for `key` out at `viewport` and restart `stream`
    /// on it. No tab kept for `key` (or one busy with a call right now) is
    /// [`BrowserRenderError::TabClosed`] and nothing is opened; a tab whose
    /// browser broke is reopened at its address like [`Self::with`] does,
    /// and the stream, left on the old browser, ends.
    pub fn resize_stream(
        &self,
        key: &str,
        viewport: Viewport,
        stream: &PageStream,
    ) -> Result<(), BrowserRenderError> {
        self.run_on(key, viewport, false, |_| Ok(()))?;
        stream.restart()
    }

    fn run_on<R>(
        &self,
        key: &str,
        viewport: Viewport,
        open_missing: bool,
        mut f: impl FnMut(&mut PageSession) -> Result<R, BrowserRenderError>,
    ) -> Result<R, BrowserRenderError> {
        self.close_idle();
        let existing = self.take(key);
        let reopen_at = existing.as_ref().and_then(|s| s.url.clone());
        let mut session = match existing {
            Some(session) => session,
            None if open_missing => PageSession::open(&self.pool, viewport)?,
            None => return Err(BrowserRenderError::TabClosed),
        };
        let mut result = session
            .set_viewport(viewport)
            .and_then(|()| f(&mut session));
        if matches!(&result, Err(error) if error.needs_restart()) {
            self.leave(session, Leave::Broken);
            session = PageSession::open(&self.pool, viewport)?;
            if let Some(url) = reopen_at.as_deref() {
                if let Err(error) = session.load(url) {
                    let leave = if error.needs_restart() {
                        Leave::Broken
                    } else {
                        Leave::Close
                    };
                    self.leave(session, leave);
                    return Err(error);
                }
            }
            result = f(&mut session);
        }
        match &result {
            Err(error) if error.needs_restart() => self.leave(session, Leave::Broken),
            _ => {
                session.last_used = Instant::now();
                // A second caller on the same key opened its own tab while
                // this one ran: keep the newest, close the other.
                let replaced = self.lock().insert(key.to_string(), session);
                if let Some(older) = replaced {
                    self.leave(older, Leave::Close);
                }
            }
        }
        result
    }

    /// Close the tab kept for `key`, if any.
    pub fn close(&self, key: &str) {
        if let Some(session) = self.take(key) {
            self.leave(session, Leave::Close);
        }
    }

    /// How many tabs are open.
    pub fn len(&self) -> usize {
        self.lock().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Close tabs unused for [`SESSION_IDLE`] and not streamed.
    pub fn close_idle(&self) {
        let idle: Vec<PageSession> = {
            let mut sessions = self.lock();
            let keys: Vec<String> = sessions
                .iter()
                .filter(|(_, s)| session_is_idle(s.last_used, &s.streams, SESSION_IDLE))
                .map(|(key, _)| key.clone())
                .collect();
            keys.iter().filter_map(|key| sessions.remove(key)).collect()
        };
        for session in idle {
            self.leave(session, Leave::Close);
        }
    }

    fn take(&self, key: &str) -> Option<PageSession> {
        self.lock().remove(key)
    }

    /// Drop `session`'s hold on the browser; a broken one also ends the
    /// browser process it was opened on (not a newer one).
    fn leave(&self, session: PageSession, how: Leave) {
        let generation = session.generation;
        if how == Leave::Close {
            session.close();
        }
        self.pool
            .release((how == Leave::Broken).then_some(generation));
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, PageSession>> {
        self.sessions.lock().unwrap_or_else(|e| e.into_inner())
    }
}

#[cfg(test)]
#[path = "page_session_tests.rs"]
mod tests;
