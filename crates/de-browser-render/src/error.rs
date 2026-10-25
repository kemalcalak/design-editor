//! `BrowserRenderError`: every way a page snapshot can fail.

use std::io;

#[derive(Debug, thiserror::Error)]
pub enum BrowserRenderError {
    /// No Chrome, Chromium, Edge or Brave was found (and `DE_BROWSER_PATH`
    /// names none).
    #[error("no Chromium-based browser found (set DE_BROWSER_PATH)")]
    NoBrowser,
    #[error("could not start the browser: {0}")]
    Launch(#[source] io::Error),
    /// The browser started but never printed its DevTools endpoint.
    #[error("the browser did not open its DevTools endpoint")]
    Startup,
    #[error("could not connect to the browser: {0}")]
    Connect(String),
    /// The DevTools connection dropped mid-command.
    #[error("the browser connection closed")]
    Closed,
    #[error("{method} timed out")]
    Timeout { method: String },
    #[error("{method} failed: {message}")]
    Protocol { method: String, message: String },
    /// `Page.navigate` reported an error (DNS, refused connection, …).
    #[error("could not load the page: {0}")]
    Navigation(String),
    #[error("the screenshot was not a PNG")]
    Screenshot,
    /// A CSS selector matched nothing on the page.
    #[error("no element matches `{selector}`")]
    NoMatch { selector: String },
    /// A DevTools method outside the allowed domains.
    #[error("{method} is not available: only the {allowed} domains are")]
    MethodRefused { method: String, allowed: String },
    /// A kept-open tab has no page yet.
    #[error("no page is loaded yet: call action load-page first")]
    NotLoaded,
}

impl BrowserRenderError {
    /// Whether the caller asked for something wrong (a selector that
    /// matches nothing, a refused method, no page yet) rather than the
    /// browser failing.
    pub fn is_caller_error(&self) -> bool {
        matches!(
            self,
            BrowserRenderError::NoMatch { .. }
                | BrowserRenderError::MethodRefused { .. }
                | BrowserRenderError::NotLoaded
        )
    }

    /// Whether the browser process or its connection is gone, so the next
    /// render should start a fresh one instead of reusing it.
    pub fn needs_restart(&self) -> bool {
        matches!(
            self,
            BrowserRenderError::Closed | BrowserRenderError::Connect(_)
        )
    }
}
