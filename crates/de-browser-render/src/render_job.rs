//! One page snapshot: a fresh tab laid out at the requested CSS viewport,
//! navigated, scrolled and captured as a PNG, then closed. A slow page is
//! captured as far as it got when the load wait runs out.

use crate::cdp_client::CdpClient;
use crate::tab::{Tab, Viewport};
use crate::BrowserRenderError;

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

impl RenderSpec {
    pub fn viewport(&self) -> Viewport {
        Viewport {
            width: self.layout_width,
            height: self.layout_height,
            scale: self.scale,
            mobile: self.mobile,
        }
    }
}

pub(crate) fn render_page(
    client: &mut CdpClient,
    spec: &RenderSpec,
) -> Result<Vec<u8>, BrowserRenderError> {
    let mut tab = Tab::open(client)?;
    let result = capture(client, &mut tab, spec);
    // Always close the tab, even when the capture failed.
    tab.close(client);
    result
}

fn capture(
    client: &mut CdpClient,
    tab: &mut Tab,
    spec: &RenderSpec,
) -> Result<Vec<u8>, BrowserRenderError> {
    tab.emulate(client, &spec.viewport())?;
    tab.navigate(client, &spec.url)?;
    tab.settle(client, Some((spec.scroll_x, spec.scroll_y)));
    tab.screenshot(client, None)
}
