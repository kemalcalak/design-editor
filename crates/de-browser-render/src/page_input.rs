//! Mouse and keyboard input for a streamed tab (#324, phase 4): the user
//! drives a live canvas browser node's page. A [`PageInput`] is a DevTools
//! connection of its own, attached to the stream's tab
//! ([`crate::PageStream::input`]), so the page that gets the input is
//! exactly the one the canvas shows and the stream's own connection keeps
//! reading frames meanwhile. Callers may send only `Input.*` methods (it
//! reads `Page.getLayoutMetrics` itself, see below); each call blocks
//! until Chrome handled the event (about a millisecond locally, a press
//! two round trips), so callers send from a worker thread. A JavaScript dialog the input opens
//! (`alert`, `confirm`, `prompt`, `beforeunload`) would hold that call
//! until it closes, and a headless tab shows no dialog to close, so it is
//! answered at once: an `alert` or `beforeunload` is accepted, a `confirm`
//! or `prompt` is cancelled, so a click never says yes to a question the
//! user did not see. Dropping it detaches from the tab (the tab stays
//! open).
//!
//! Mouse positions come in the px of the frame the user sees, which is
//! the page's visual viewport, while Chrome reads them as layout-viewport
//! CSS px. The two differ when the page is shown scaled: a mobile
//! (Tablet / Phone) tab lays a page without `width=device-width` out
//! 980 px wide and shows it scaled down to the device width, so a click
//! aimed at a field would land beside it and never focus it. Each mouse
//! event is mapped through the tab's visual viewport
//! ([`VisualViewport`], read with `Page.getLayoutMetrics`, at most
//! [`VIEWPORT_TTL`] old, fresh for every press and after a wheel step).
//! On an unscaled page
//! the mapping is the identity.

use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::cdp_client::{Awaited, CdpClient};
use crate::tab::{Tab, COMMAND_TIMEOUT};
use crate::BrowserRenderError;

/// The only domain a [`PageInput`] may call.
const INPUT_DOMAIN: &str = "Input";
/// The event a page dialog opens with.
const DIALOG_OPENING: &str = "Page.javascriptDialogOpening";
/// The input method whose positions are mapped.
const MOUSE_EVENT: &str = "Input.dispatchMouseEvent";
/// How long a read of the visual viewport is used for moves and wheel
/// steps; a press always reads it again.
const VIEWPORT_TTL: Duration = Duration::from_millis(100);

pub struct PageInput {
    client: CdpClient,
    /// `None` only while dropping.
    tab: Option<Tab>,
    /// The last read of the visual viewport and when it was read.
    viewport: Option<(VisualViewport, Instant)>,
}

/// Where the shown frame sits in the page: the visual viewport's offset in
/// the layout viewport (CSS px) and its scale (frame px per CSS px).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct VisualViewport {
    pub offset_x: f64,
    pub offset_y: f64,
    pub scale: f64,
}

impl VisualViewport {
    /// An unscaled page: frame px are CSS px.
    pub const IDENTITY: Self = Self {
        offset_x: 0.0,
        offset_y: 0.0,
        scale: 1.0,
    };

    /// The `cssVisualViewport` of a `Page.getLayoutMetrics` result;
    /// the identity when it is missing or unusable.
    pub fn from_metrics(metrics: &Value) -> Self {
        let viewport = metrics.get("cssVisualViewport");
        let field = |key: &str| viewport.and_then(|v| v.get(key)).and_then(Value::as_f64);
        let finite = |v: Option<f64>| v.filter(|v| v.is_finite());
        Self {
            offset_x: finite(field("offsetX")).unwrap_or(0.0),
            offset_y: finite(field("offsetY")).unwrap_or(0.0),
            scale: finite(field("scale")).filter(|s| *s > 0.0).unwrap_or(1.0),
        }
    }

    /// Map an `Input.dispatchMouseEvent`'s params from frame px to
    /// layout-viewport CSS px. Only the position: Chrome already reads a
    /// wheel step's deltas in frame px (it scrolls the page by them over
    /// the scale), so the page scrolls one frame px per px.
    pub fn map_mouse_params(&self, params: &mut Value) {
        if *self == Self::IDENTITY {
            return;
        }
        let map = |params: &mut Value, key: &str, offset: f64| {
            if let Some(v) = params.get(key).and_then(Value::as_f64) {
                params[key] = json!(offset + v / self.scale);
            }
        };
        map(params, "x", self.offset_x);
        map(params, "y", self.offset_y);
    }
}

impl PageInput {
    /// Attach to the tab `target_id` of the browser at `ws_url`.
    pub(crate) fn connect(ws_url: &str, target_id: &str) -> Result<Self, BrowserRenderError> {
        let mut client = CdpClient::connect(ws_url)?;
        let tab = Tab::attach(&mut client, target_id)?;
        Ok(Self {
            client,
            tab: Some(tab),
            viewport: None,
        })
    }

    /// Send one `Input.*` call and wait for Chrome to handle it. Any other
    /// domain is refused ([`BrowserRenderError::MethodRefused`]).
    pub fn dispatch(&mut self, method: &str, mut params: Value) -> Result<(), BrowserRenderError> {
        match method.split_once('.') {
            Some((INPUT_DOMAIN, name)) if !name.is_empty() => {}
            _ => {
                return Err(BrowserRenderError::MethodRefused {
                    method: method.to_string(),
                    allowed: INPUT_DOMAIN.to_string(),
                })
            }
        }
        let kind = params.get("type").and_then(Value::as_str).unwrap_or("");
        let (pressed, wheel) = (kind == "mousePressed", kind == "mouseWheel");
        if method == MOUSE_EVENT {
            self.visual_viewport(pressed)?.map_mouse_params(&mut params);
        }
        self.call_answering_dialogs(method, params)?;
        if method == MOUSE_EVENT && wheel {
            // A wheel step can pan a zoomed-in visual viewport.
            self.viewport = None;
        }
        Ok(())
    }

    /// Send `method` on the tab and wait for its result, answering any page
    /// dialog that opens meanwhile (see the module docs): the reply to a
    /// call that opened one only comes once it is closed.
    fn call_answering_dialogs(
        &mut self,
        method: &str,
        params: Value,
    ) -> Result<Value, BrowserRenderError> {
        let tab = self.tab.as_ref().ok_or(BrowserRenderError::TabClosed)?;
        let session = tab.session().to_string();
        let id = self.client.send(Some(&session), method, params)?;
        let deadline = Instant::now() + COMMAND_TIMEOUT;
        loop {
            let watch = Some((session.as_str(), DIALOG_OPENING));
            match self.client.await_reply(id, method, deadline, watch)? {
                Awaited::Reply(result) => return Ok(result),
                Awaited::Event(params) => {
                    // The reply to `id` arrives once the dialog closed and
                    // is kept for the next wait.
                    self.client.call(
                        Some(&session),
                        "Page.handleJavaScriptDialog",
                        json!({ "accept": accepts_dialog(&params) }),
                        COMMAND_TIMEOUT,
                    )?;
                }
            }
        }
    }

    /// The tab's visual viewport, read again when `fresh` or older than
    /// [`VIEWPORT_TTL`]. A page that cannot say is taken as unscaled; a
    /// lost connection is an error.
    fn visual_viewport(&mut self, fresh: bool) -> Result<VisualViewport, BrowserRenderError> {
        if let Some((viewport, at)) = self.viewport {
            if !fresh && at.elapsed() < VIEWPORT_TTL {
                return Ok(viewport);
            }
        }
        let viewport = match self.call_answering_dialogs("Page.getLayoutMetrics", json!({})) {
            Ok(metrics) => VisualViewport::from_metrics(&metrics),
            Err(BrowserRenderError::Protocol { .. }) => VisualViewport::IDENTITY,
            Err(error) => return Err(error),
        };
        self.viewport = Some((viewport, Instant::now()));
        Ok(viewport)
    }

    /// Any call on the tab, for tests that check what the input did.
    #[cfg(test)]
    pub(crate) fn tab_command(
        &mut self,
        method: &str,
        params: Value,
    ) -> Result<Value, BrowserRenderError> {
        let tab = self.tab.as_ref().ok_or(BrowserRenderError::TabClosed)?;
        tab.command(&mut self.client, method, params)
    }
}

/// Whether a page dialog (`Page.javascriptDialogOpening` params) is
/// accepted: only one that asks nothing (`alert`, `beforeunload`).
fn accepts_dialog(params: &Value) -> bool {
    matches!(
        params.get("type").and_then(Value::as_str),
        Some("alert" | "beforeunload")
    )
}

impl Drop for PageInput {
    fn drop(&mut self) {
        if let Some(tab) = self.tab.take() {
            tab.detach(&mut self.client);
        }
    }
}

#[cfg(test)]
#[path = "page_input_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "page_input_chrome_tests.rs"]
mod chrome_tests;
