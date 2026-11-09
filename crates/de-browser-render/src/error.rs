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
    /// A streamed tab was closed, crashed or detached, or the browser it
    /// lived in was restarted. Also: a kept-open tab that is not there for
    /// a stream (none kept for the key, or a call holds it right now).
    #[error("the streamed tab closed")]
    TabClosed,
    /// A stream on a kept-open tab cannot change its viewport: the tab's
    /// page session owns it (`PageSessions::resize_stream`).
    #[error("a stream on the MCP browser tab cannot resize it: resize the tab's page session")]
    SharedTabViewport,
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
                | BrowserRenderError::SharedTabViewport
        )
    }

    /// The same error again (the enum is not `Clone`: `Launch` holds an
    /// `io::Error`), for a reason told to two callers.
    pub(crate) fn duplicate(&self) -> Self {
        use BrowserRenderError as E;
        match self {
            E::NoBrowser => E::NoBrowser,
            E::Launch(error) => E::Launch(io::Error::new(error.kind(), error.to_string())),
            E::Startup => E::Startup,
            E::Connect(message) => E::Connect(message.clone()),
            E::Closed => E::Closed,
            E::Timeout { method } => E::Timeout {
                method: method.clone(),
            },
            E::Protocol { method, message } => E::Protocol {
                method: method.clone(),
                message: message.clone(),
            },
            E::Navigation(message) => E::Navigation(message.clone()),
            E::Screenshot => E::Screenshot,
            E::NoMatch { selector } => E::NoMatch {
                selector: selector.clone(),
            },
            E::MethodRefused { method, allowed } => E::MethodRefused {
                method: method.clone(),
                allowed: allowed.clone(),
            },
            E::NotLoaded => E::NotLoaded,
            E::TabClosed => E::TabClosed,
            E::SharedTabViewport => E::SharedTabViewport,
        }
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
