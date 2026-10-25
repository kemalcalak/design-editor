//! One page snapshot: a fresh tab laid out at the requested CSS viewport,
//! navigated, scrolled and captured as a PNG, then closed. A slow page is
//! captured as far as it got when the load wait runs out.

use std::time::{Duration, Instant};

use base64::Engine;
use serde_json::{json, Value};

use crate::cdp_client::CdpClient;
use crate::BrowserRenderError;

/// Answer time for a plain command.
const COMMAND_TIMEOUT: Duration = Duration::from_secs(10);
/// How long the page may load before it is captured anyway.
const LOAD_TIMEOUT: Duration = Duration::from_secs(15);
/// Web fonts and two animation frames after load, so text is not caught
/// in its fallback face.
const SETTLE_TIMEOUT: Duration = Duration::from_secs(3);
const MOBILE_USER_AGENT: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 17_5 like Mac OS X) \
     AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.5 Mobile/15E148 Safari/604.1";

/// What to capture.
#[derive(Debug, Clone, PartialEq)]
pub struct RenderSpec {
    /// An http, https or file address.
    pub url: String,
    /// The CSS viewport the page lays out at.
    pub layout_width: u32,
    pub layout_height: u32,
    /// Output pixels per CSS pixel: the PNG is `layout × scale`.
    pub scale: f64,
    /// Emulate a touch phone or tablet (mobile user agent and viewport).
    pub mobile: bool,
    /// Page scroll in CSS px.
    pub scroll_x: f64,
    pub scroll_y: f64,
}

pub(crate) fn render_page(
    client: &mut CdpClient,
    spec: &RenderSpec,
) -> Result<Vec<u8>, BrowserRenderError> {
    let target = client.call(
        None,
        "Target.createTarget",
        json!({ "url": "about:blank" }),
        COMMAND_TIMEOUT,
    )?;
    let target_id = string_field(&target, "targetId", "Target.createTarget")?;
    let result = capture_in_target(client, &target_id, spec);
    // Always close the tab, even when the capture failed.
    let _ = client.call(
        None,
        "Target.closeTarget",
        json!({ "targetId": target_id }),
        COMMAND_TIMEOUT,
    );
    result
}

fn capture_in_target(
    client: &mut CdpClient,
    target_id: &str,
    spec: &RenderSpec,
) -> Result<Vec<u8>, BrowserRenderError> {
    let attached = client.call(
        None,
        "Target.attachToTarget",
        json!({ "targetId": target_id, "flatten": true }),
        COMMAND_TIMEOUT,
    )?;
    let session = string_field(&attached, "sessionId", "Target.attachToTarget")?;
    let result = capture_in_session(client, &session, spec);
    client.drop_session_events(&session);
    result
}

fn capture_in_session(
    client: &mut CdpClient,
    session: &str,
    spec: &RenderSpec,
) -> Result<Vec<u8>, BrowserRenderError> {
    let s = Some(session);
    client.call(s, "Page.enable", json!({}), COMMAND_TIMEOUT)?;
    client.call(
        s,
        "Emulation.setDeviceMetricsOverride",
        json!({
            "width": spec.layout_width.max(1),
            "height": spec.layout_height.max(1),
            "deviceScaleFactor": spec.scale,
            "mobile": spec.mobile,
        }),
        COMMAND_TIMEOUT,
    )?;
    if spec.mobile {
        client.call(
            s,
            "Emulation.setUserAgentOverride",
            json!({ "userAgent": MOBILE_USER_AGENT }),
            COMMAND_TIMEOUT,
        )?;
        client.call(
            s,
            "Emulation.setTouchEmulationEnabled",
            json!({ "enabled": true }),
            COMMAND_TIMEOUT,
        )?;
    }
    let navigated = client.call(
        s,
        "Page.navigate",
        json!({ "url": spec.url }),
        COMMAND_TIMEOUT,
    )?;
    if let Some(error) = navigated.get("errorText").and_then(Value::as_str) {
        return Err(BrowserRenderError::Navigation(error.to_string()));
    }
    // A page still loading at the deadline is captured as it stands.
    client.wait_event(
        session,
        "Page.loadEventFired",
        Instant::now() + LOAD_TIMEOUT,
    )?;
    let settle = format!(
        "window.scrollTo({x}, {y}); \
         (document.fonts ? document.fonts.ready : Promise.resolve()).then(() => \
           new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r))))",
        x = spec.scroll_x.max(0.0),
        y = spec.scroll_y.max(0.0),
    );
    // Settling is best effort: a page that never settles is still captured.
    let _ = client.call(
        s,
        "Runtime.evaluate",
        json!({ "expression": settle, "awaitPromise": true }),
        SETTLE_TIMEOUT,
    );
    let shot = client.call(
        s,
        "Page.captureScreenshot",
        json!({ "format": "png", "fromSurface": true }),
        COMMAND_TIMEOUT,
    )?;
    let data = string_field(&shot, "data", "Page.captureScreenshot")?;
    let png = base64::engine::general_purpose::STANDARD
        .decode(data)
        .map_err(|_| BrowserRenderError::Screenshot)?;
    if !png.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err(BrowserRenderError::Screenshot);
    }
    Ok(png)
}

fn string_field(value: &Value, key: &str, method: &str) -> Result<String, BrowserRenderError> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| BrowserRenderError::Protocol {
            method: method.to_string(),
            message: format!("reply has no {key}"),
        })
}
