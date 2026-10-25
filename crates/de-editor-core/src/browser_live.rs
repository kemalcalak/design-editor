//! Live page state of the browser node (#20): what a page snapshot is
//! rendered from (a [`BrowserRenderRequest`] and its key), whether pages
//! may load at all, per-node reloads and the last load status. Hosts that
//! can render pages (the desktop app) read requests from here and write
//! the status back; the panel shows both.
//!
//! A document opened from disk that already holds browser nodes keeps its
//! pages from loading until the user allows them (the panel's Load pages
//! row), so opening someone else's file never fetches its addresses
//! unasked. Documents made in this session, and any node whose address
//! the user types, load at once.

use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use jian_ops_schema::node::PenNode;

use crate::browser_devices::{browser_device, clamp_browser_scroll, clamp_browser_zoom};
use crate::pen_node_ext::PenNodeExt;

/// Output pixels per CSS pixel of the node box (a 2× display).
const OUTPUT_SCALE: f64 = 2.0;
/// Longest snapshot edge in pixels; larger boxes render coarser.
pub const MAX_SNAPSHOT_EDGE: f64 = 2048.0;

/// The last load result of one browser node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserPageStatus {
    Loading,
    Ready,
    /// No Chromium-based browser is installed.
    NoBrowser,
    Failed,
}

/// Session state of the live pages. Not part of the document.
#[derive(Debug, Clone, Default)]
pub struct BrowserLiveState {
    /// True while an opened document's pages wait for the user's go-ahead.
    pages_held: bool,
    reloads: HashMap<String, u32>,
    /// Bumped by every reload, so a host can tell nothing changed cheaply.
    reload_generation: u64,
    status: HashMap<String, BrowserPageStatus>,
}

impl BrowserLiveState {
    pub fn pages_allowed(&self) -> bool {
        !self.pages_held
    }

    pub fn allow_pages(&mut self) {
        self.pages_held = false;
    }

    pub fn hold_pages(&mut self) {
        self.pages_held = true;
    }

    /// Ask for a fresh snapshot of `node_id` (changes its request key).
    pub fn reload(&mut self, node_id: &str) {
        *self.reloads.entry(node_id.to_string()).or_default() += 1;
        self.reload_generation += 1;
    }

    /// Changes whenever any node is reloaded.
    pub fn reload_generation(&self) -> u64 {
        self.reload_generation
    }

    pub fn reload_count(&self, node_id: &str) -> u32 {
        self.reloads.get(node_id).copied().unwrap_or(0)
    }

    pub fn set_status(&mut self, node_id: &str, status: BrowserPageStatus) {
        self.status.insert(node_id.to_string(), status);
    }

    pub fn status(&self, node_id: &str) -> Option<BrowserPageStatus> {
        self.status.get(node_id).copied()
    }

    /// Forget per-node reloads and statuses (the document was replaced).
    pub fn clear_nodes(&mut self) {
        self.reloads.clear();
        self.status.clear();
    }
}

/// What one browser snapshot is rendered from.
#[derive(Debug, Clone, PartialEq)]
pub struct BrowserRenderRequest {
    pub node_id: String,
    pub url: String,
    /// The CSS viewport the page lays out at (after zoom).
    pub layout_width: u32,
    pub layout_height: u32,
    /// Output pixels per layout pixel.
    pub scale: f64,
    pub mobile: bool,
    pub scroll_x: f64,
    pub scroll_y: f64,
    /// Equal keys render the same picture; a new key needs a new snapshot.
    pub key: u64,
}

/// The snapshot `node` needs, `None` for a non-browser node, a blank
/// address or an empty box. The viewport is the device preset's width (or
/// the node's own) at the node's aspect ratio, divided by the page zoom;
/// the picture is the node box at 2×, capped at [`MAX_SNAPSHOT_EDGE`].
pub fn browser_render_request(node: &PenNode, reload: u32) -> Option<BrowserRenderRequest> {
    let PenNode::Browser(browser) = node else {
        return None;
    };
    let url = browser
        .url
        .as_deref()
        .map(str::trim)
        .filter(|u| !u.is_empty())?;
    let (box_w, box_h) = (node.width_px()?, node.height_px()?);
    if box_w < 1.0 || box_h < 1.0 {
        return None;
    }
    let device = browser.device_id.as_deref().and_then(browser_device);
    let zoom = clamp_browser_zoom(browser.zoom.unwrap_or(1.0));
    let viewport_w = device.map_or(box_w, |d| d.width);
    let layout_w = (viewport_w / zoom).round().max(1.0);
    let layout_h = (layout_w * box_h / box_w).round().max(1.0);
    let out_w = (box_w * OUTPUT_SCALE)
        .min(MAX_SNAPSHOT_EDGE * box_w / box_w.max(box_h))
        .max(1.0);
    let scale = ((out_w / layout_w) * 100.0).round() / 100.0;
    let mut request = BrowserRenderRequest {
        node_id: browser.base.id.clone(),
        url: url.to_string(),
        layout_width: layout_w as u32,
        layout_height: layout_h as u32,
        scale: scale.max(0.01),
        mobile: device.is_some_and(|d| d.mobile),
        scroll_x: clamp_browser_scroll(browser.scroll_x.unwrap_or(0.0)),
        scroll_y: clamp_browser_scroll(browser.scroll_y.unwrap_or(0.0)),
        key: 0,
    };
    let mut hasher = DefaultHasher::new();
    request.url.hash(&mut hasher);
    (request.layout_width, request.layout_height).hash(&mut hasher);
    request.scale.to_bits().hash(&mut hasher);
    request.mobile.hash(&mut hasher);
    request.scroll_x.to_bits().hash(&mut hasher);
    request.scroll_y.to_bits().hash(&mut hasher);
    reload.hash(&mut hasher);
    request.key = hasher.finish();
    Some(request)
}

/// Whether any page of the document holds a browser node.
pub(crate) fn document_has_browser(doc: &jian_ops_schema::PenDocument) -> bool {
    fn walk(nodes: &[PenNode]) -> bool {
        nodes.iter().any(|node| {
            matches!(node, PenNode::Browser(_)) || node.children_ref().is_some_and(walk)
        })
    }
    walk(&doc.children) || doc.pages.iter().flatten().any(|page| walk(&page.children))
}

#[cfg(test)]
#[path = "browser_live_tests.rs"]
mod tests;
