//! Live page streams for canvas browser nodes (#324): the rules both hosts
//! share, with no transport.
//!
//! Like pen.dev, a browser node plays live only while the user works with
//! it: while it is selected, or interactive (phase 4 drives the page;
//! interactive implies live). Every other browser node shows its still
//! snapshot. These functions decide which nodes stream
//! ([`pick_live_pages`]: at most [`MAX_LIVE_PAGES`] of a multi-selection),
//! the host's rate and size caps ([`stream_tier`]), how large a stream's
//! frames are ([`stream_frame_width`]) and how its page is laid out
//! ([`BrowserStreamSpec`], derived from the node's snapshot request so a
//! stream lays the page out exactly like its still snapshot).

use crate::browser_live::BrowserRenderRequest;

/// At most this many browser nodes of a selection stream at once.
pub const MAX_LIVE_PAGES: usize = 3;
/// Frame-rate cap of a desktop stream.
pub const DESKTOP_STREAM_FPS: f64 = 30.0;
/// Frame-rate cap of a web stream (every frame crosses the network).
pub const WEB_STREAM_FPS: f64 = 4.0;
/// Frame width steps, in device px: the smallest and the largest.
pub const MIN_STREAM_WIDTH: u32 = 128;
pub const MAX_STREAM_WIDTH: u32 = 2048;
/// Tallest frame, in device px.
pub const MAX_STREAM_HEIGHT: u32 = 4096;
/// Largest device scale a stream lays its page out at (output px per CSS
/// px), so a zoomed-in node gets frames sharper than 1×.
pub const MAX_STREAM_PAGE_SCALE: f64 = 2.0;
/// A stream steps down to a smaller frame only once the node needs at most
/// this share of that smaller step, so a zoom hovering at a step boundary
/// does not restart the stream back and forth.
const SHRINK_SLACK: f32 = 0.8;

/// Which host streams: they cap rates differently.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamHost {
    Desktop,
    Web,
}

/// How fast and how large a host's streams may be.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StreamTier {
    pub max_fps: f64,
    /// Frame bounds in device px.
    pub max_width: u32,
    pub max_height: u32,
}

/// The caps of `host`'s streams.
pub fn stream_tier(host: StreamHost) -> StreamTier {
    StreamTier {
        max_fps: match host {
            StreamHost::Desktop => DESKTOP_STREAM_FPS,
            StreamHost::Web => WEB_STREAM_FPS,
        },
        max_width: MAX_STREAM_WIDTH,
        max_height: MAX_STREAM_HEIGHT,
    }
}

/// A browser node the canvas shows, as a candidate for streaming.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LiveCandidate<'a> {
    pub node_id: &'a str,
    /// On-screen area in logical px².
    pub screen_area: f32,
    pub selected: bool,
    /// The user drives the page (phase 4's interaction mode).
    pub interactive: bool,
}

impl LiveCandidate<'_> {
    /// Whether the node wants to stream: selected or interactive.
    pub fn wants_live(&self) -> bool {
        self.selected || self.interactive
    }
}

/// Which candidates stream, best first: only selected or interactive ones,
/// the interactive node first, then the larger ones on screen, then by id
/// (a stable order for equal sizes). At most `cap`.
pub fn pick_live_pages<'a>(candidates: &[LiveCandidate<'a>], cap: usize) -> Vec<&'a str> {
    let mut ranked: Vec<&LiveCandidate<'a>> = candidates
        .iter()
        .filter(|c| c.wants_live() && c.screen_area.is_finite() && c.screen_area > 0.0)
        .collect();
    ranked.sort_by(|a, b| {
        b.interactive
            .cmp(&a.interactive)
            .then(b.screen_area.total_cmp(&a.screen_area))
            .then(a.node_id.cmp(b.node_id))
    });
    ranked.into_iter().take(cap).map(|c| c.node_id).collect()
}

/// The power-of-two step that covers `device_px`, from
/// [`MIN_STREAM_WIDTH`] up to `max` (a power of two).
fn width_step(device_px: f32, max: u32) -> u32 {
    let mut step = MIN_STREAM_WIDTH;
    while step < max && (step as f32) < device_px {
        step *= 2;
    }
    step.min(max.max(MIN_STREAM_WIDTH))
}

/// The frame width for a node `screen_width` logical px wide at `dpi`,
/// at most `max_width`, given the width its stream uses now (`current`).
/// Widths come in power-of-two steps, so zooming restarts a stream only
/// when it crosses a step: growing as soon as the node outgrows its step,
/// shrinking only once it clearly fits the smaller one.
pub fn stream_frame_width(
    current: Option<u32>,
    screen_width: f32,
    dpi: f32,
    max_width: u32,
) -> u32 {
    let dpi = if dpi.is_finite() && dpi > 0.0 {
        dpi
    } else {
        1.0
    };
    let wanted = if screen_width.is_finite() {
        screen_width.max(0.0) * dpi
    } else {
        0.0
    };
    let target = width_step(wanted, max_width);
    match current {
        Some(current) if current > max_width => target,
        Some(current) if target < current => {
            let smaller = (current / 2).max(MIN_STREAM_WIDTH);
            if wanted <= smaller as f32 * SHRINK_SLACK {
                target
            } else {
                current
            }
        }
        _ => target,
    }
}

/// The key of the MCP `browser` tool's kept-open tab for a node: a stream
/// shares that tab when the agent opened one.
pub fn node_page_session_key(node_id: &str) -> String {
    format!("node:{node_id}")
}

/// What one stream needs: the page and its layout (the snapshot request's)
/// and the frame size and rate.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserStreamSpec {
    pub url: String,
    /// The page's layout viewport in CSS px.
    pub layout_width: u32,
    pub layout_height: u32,
    #[serde(default)]
    pub mobile: bool,
    /// Output px per CSS px the page is laid out at: enough for
    /// `frame_width`, in quarter steps from 1 to
    /// [`MAX_STREAM_PAGE_SCALE`].
    #[serde(default = "unit_scale")]
    pub page_scale: f64,
    /// Frames are scaled to fit these, in device px.
    pub frame_width: u32,
    pub frame_height: u32,
    pub max_fps: f64,
}

fn unit_scale() -> f64 {
    1.0
}

impl BrowserStreamSpec {
    /// The stream of `request`'s page with frames `frame_width` wide,
    /// within `tier`.
    pub fn for_request(request: &BrowserRenderRequest, frame_width: u32, tier: StreamTier) -> Self {
        let frame_width = frame_width.clamp(1, tier.max_width.max(1));
        let wanted = frame_width as f64 / f64::from(request.layout_width.max(1));
        let page_scale = ((wanted * 4.0).ceil() / 4.0).clamp(1.0, MAX_STREAM_PAGE_SCALE);
        Self {
            url: request.url.clone(),
            layout_width: request.layout_width,
            layout_height: request.layout_height,
            mobile: request.mobile,
            page_scale,
            frame_width,
            frame_height: tier.max_height,
            max_fps: tier.max_fps,
        }
    }

    /// Whether `other` shows the same page laid out the same way (only the
    /// device scale, frame size or rate differs): the stream keeps its page
    /// and only re-encodes.
    pub fn same_page(&self, other: &Self) -> bool {
        self.url == other.url
            && self.layout_width == other.layout_width
            && self.layout_height == other.layout_height
            && self.mobile == other.mobile
    }
}

#[cfg(test)]
#[path = "browser_stream_tests.rs"]
mod tests;
