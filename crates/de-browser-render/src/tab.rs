//! One browser tab driven over a [`CdpClient`]: open, emulate a viewport,
//! navigate (waiting for the load, capped), evaluate script, capture and
//! close. Both the one-shot canvas snapshot and the kept-open MCP sessions
//! are built from these steps.

use std::time::{Duration, Instant};

use base64::Engine;
use serde_json::{json, Value};

use crate::cdp_client::CdpClient;
use crate::BrowserRenderError;

/// Answer time for a plain command.
pub(crate) const COMMAND_TIMEOUT: Duration = Duration::from_secs(10);
/// How long a page may load before it is used as it stands.
pub(crate) const LOAD_TIMEOUT: Duration = Duration::from_secs(15);
/// Web fonts and two animation frames, so text is not caught in its
/// fallback face.
const SETTLE_TIMEOUT: Duration = Duration::from_secs(3);
/// Tallest full-page capture, in CSS px.
pub(crate) const MAX_FULL_PAGE_HEIGHT: f64 = 8000.0;
const MOBILE_USER_AGENT: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 17_5 like Mac OS X) \
     AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.5 Mobile/15E148 Safari/604.1";

/// The CSS viewport a tab lays pages out at.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Viewport {
    pub width: u32,
    pub height: u32,
    /// Output pixels per CSS pixel.
    pub scale: f64,
    /// Emulate a touch phone or tablet.
    pub mobile: bool,
}

/// A region of the page in CSS px (document coordinates).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Clip {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    /// Output pixels per CSS pixel of the capture.
    pub scale: f64,
}

pub(crate) struct Tab {
    target_id: String,
    session: String,
    /// The browser's own user agent, read before the first mobile override
    /// so a later desktop viewport can put it back.
    default_user_agent: Option<String>,
}

impl Tab {
    pub(crate) fn open(client: &mut CdpClient) -> Result<Self, BrowserRenderError> {
        let target = client.call(
            None,
            "Target.createTarget",
            json!({ "url": "about:blank" }),
            COMMAND_TIMEOUT,
        )?;
        let target_id = string_field(&target, "targetId", "Target.createTarget")?;
        let attached = client.call(
            None,
            "Target.attachToTarget",
            json!({ "targetId": target_id, "flatten": true }),
            COMMAND_TIMEOUT,
        );
        let session = match attached
            .and_then(|reply| string_field(&reply, "sessionId", "Target.attachToTarget"))
        {
            Ok(session) => session,
            Err(error) => {
                let _ = close_target(client, &target_id);
                return Err(error);
            }
        };
        let tab = Self {
            target_id,
            session,
            default_user_agent: None,
        };
        tab.command(client, "Page.enable", json!({}))?;
        Ok(tab)
    }

    /// Send `method` to this tab.
    pub(crate) fn command(
        &self,
        client: &mut CdpClient,
        method: &str,
        params: Value,
    ) -> Result<Value, BrowserRenderError> {
        client.call(Some(&self.session), method, params, COMMAND_TIMEOUT)
    }

    pub(crate) fn emulate(
        &mut self,
        client: &mut CdpClient,
        viewport: &Viewport,
    ) -> Result<(), BrowserRenderError> {
        self.command(
            client,
            "Emulation.setDeviceMetricsOverride",
            json!({
                "width": viewport.width.max(1),
                "height": viewport.height.max(1),
                "deviceScaleFactor": viewport.scale,
                "mobile": viewport.mobile,
            }),
        )?;
        if self.default_user_agent.is_none() {
            let agent = self.evaluate(client, "navigator.userAgent", COMMAND_TIMEOUT)?;
            self.default_user_agent = agent.as_str().map(str::to_string);
        }
        let agent = if viewport.mobile {
            Some(MOBILE_USER_AGENT)
        } else {
            self.default_user_agent.as_deref()
        };
        if let Some(agent) = agent {
            self.command(
                client,
                "Emulation.setUserAgentOverride",
                json!({ "userAgent": agent }),
            )?;
        }
        self.command(
            client,
            "Emulation.setTouchEmulationEnabled",
            json!({ "enabled": viewport.mobile }),
        )?;
        Ok(())
    }

    /// Load `url` and wait for it (a page still loading at the deadline is
    /// used as it stands), then let fonts settle.
    pub(crate) fn navigate(
        &self,
        client: &mut CdpClient,
        url: &str,
    ) -> Result<(), BrowserRenderError> {
        // Events from an earlier load would end the wait below at once.
        client.drop_session_events(&self.session);
        let navigated = self.command(client, "Page.navigate", json!({ "url": url }))?;
        if let Some(error) = navigated.get("errorText").and_then(Value::as_str) {
            return Err(BrowserRenderError::Navigation(error.to_string()));
        }
        self.wait_load(client)?;
        self.settle(client, None);
        Ok(())
    }

    /// Wait for the tab's next load event, capped at [`LOAD_TIMEOUT`].
    pub(crate) fn wait_load(&self, client: &mut CdpClient) -> Result<bool, BrowserRenderError> {
        client.wait_event(
            &self.session,
            "Page.loadEventFired",
            Instant::now() + LOAD_TIMEOUT,
        )
    }

    /// Whether a navigation started within `window` (an event in flight or
    /// buffered), so a command that clicked a link can wait for its load.
    pub(crate) fn navigation_started(
        &self,
        client: &mut CdpClient,
        window: Duration,
    ) -> Result<bool, BrowserRenderError> {
        client.wait_event(
            &self.session,
            "Page.frameStartedLoading",
            Instant::now() + window,
        )
    }

    /// Best-effort settle: optional scroll, web fonts, two frames.
    pub(crate) fn settle(&self, client: &mut CdpClient, scroll: Option<(f64, f64)>) {
        let scroll = scroll.map_or(String::new(), |(x, y)| {
            format!("window.scrollTo({}, {});", x.max(0.0), y.max(0.0))
        });
        let expression = format!(
            "{scroll} (document.fonts ? document.fonts.ready : Promise.resolve()).then(() => \
               new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r))))"
        );
        let _ = client.call(
            Some(&self.session),
            "Runtime.evaluate",
            json!({ "expression": expression, "awaitPromise": true }),
            SETTLE_TIMEOUT,
        );
    }

    /// Run `expression` (awaiting a promise) and return its JSON value. A
    /// thrown exception is a protocol error carrying its message.
    pub(crate) fn evaluate(
        &self,
        client: &mut CdpClient,
        expression: &str,
        timeout: Duration,
    ) -> Result<Value, BrowserRenderError> {
        let reply = client.call(
            Some(&self.session),
            "Runtime.evaluate",
            json!({ "expression": expression, "awaitPromise": true, "returnByValue": true }),
            timeout,
        )?;
        if let Some(details) = reply.get("exceptionDetails") {
            let message = details
                .pointer("/exception/description")
                .or_else(|| details.get("text"))
                .and_then(Value::as_str)
                .unwrap_or("script error");
            return Err(BrowserRenderError::Protocol {
                method: "Runtime.evaluate".into(),
                message: message.to_string(),
            });
        }
        Ok(reply
            .pointer("/result/value")
            .cloned()
            .unwrap_or(Value::Null))
    }

    /// The whole page's size in CSS px.
    pub(crate) fn page_size(
        &self,
        client: &mut CdpClient,
    ) -> Result<(f64, f64), BrowserRenderError> {
        let metrics = self.command(client, "Page.getLayoutMetrics", json!({}))?;
        let size = metrics
            .get("cssContentSize")
            .or_else(|| metrics.get("contentSize"));
        let field = |key: &str| size.and_then(|s| s.get(key)).and_then(Value::as_f64);
        Ok((
            field("width").unwrap_or(1.0),
            field("height").unwrap_or(1.0),
        ))
    }

    /// Capture the viewport, or `clip` of the page, as PNG bytes.
    pub(crate) fn screenshot(
        &self,
        client: &mut CdpClient,
        clip: Option<Clip>,
    ) -> Result<Vec<u8>, BrowserRenderError> {
        let mut params = json!({ "format": "png", "fromSurface": true });
        if let Some(clip) = clip {
            params["clip"] = json!({
                "x": clip.x, "y": clip.y,
                "width": clip.width.max(1.0), "height": clip.height.max(1.0),
                "scale": clip.scale,
            });
            params["captureBeyondViewport"] = json!(true);
        }
        let shot = self.command(client, "Page.captureScreenshot", params)?;
        decode_png(&shot)
    }

    /// Forget events this tab buffered (before a command whose own events
    /// are waited for).
    pub(crate) fn drop_events(&self, client: &mut CdpClient) {
        client.drop_session_events(&self.session);
    }

    pub(crate) fn close(self, client: &mut CdpClient) {
        let _ = close_target(client, &self.target_id);
        client.drop_session_events(&self.session);
    }
}

/// The PNG in a `Page.captureScreenshot` reply.
pub(crate) fn decode_png(reply: &Value) -> Result<Vec<u8>, BrowserRenderError> {
    let data = string_field(reply, "data", "Page.captureScreenshot")?;
    let png = base64::engine::general_purpose::STANDARD
        .decode(data)
        .map_err(|_| BrowserRenderError::Screenshot)?;
    if !png.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err(BrowserRenderError::Screenshot);
    }
    Ok(png)
}

fn close_target(client: &mut CdpClient, target_id: &str) -> Result<Value, BrowserRenderError> {
    client.call(
        None,
        "Target.closeTarget",
        json!({ "targetId": target_id }),
        COMMAND_TIMEOUT,
    )
}

pub(crate) fn string_field(
    value: &Value,
    key: &str,
    method: &str,
) -> Result<String, BrowserRenderError> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| BrowserRenderError::Protocol {
            method: method.to_string(),
            message: format!("reply has no {key}"),
        })
}
