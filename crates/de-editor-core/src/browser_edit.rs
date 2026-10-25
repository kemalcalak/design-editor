//! Panel edits of a live browser node (#20): its address, device preset,
//! page zoom and scroll. Each write is one `PatchNodeData`, so it goes
//! through the normal command path (revision, scene rebuild) and the panel
//! commit records one undo step. Picking a known device also resizes the
//! node to the preset's box in the same patch.

use jian_ops_schema::node::{BrowserNode, PenNode};
use serde_json::{json, Value};

use crate::browser_devices::{
    browser_device, clamp_browser_scroll, clamp_browser_zoom, normalize_browser_url,
};
use crate::{EditorCommand, EditorState};

impl EditorState {
    /// The selected node's browser body, `None` for any other node.
    pub fn selected_browser(&self) -> Option<&BrowserNode> {
        match self.selected_node()? {
            PenNode::Browser(node) => Some(node),
            _ => None,
        }
    }

    /// Point the selected browser at `url` (normalized, see
    /// [`normalize_browser_url`]). False when nothing changes or the
    /// address is refused; a blank address clears it.
    pub fn set_selected_browser_url(&mut self, url: &str) -> bool {
        let Some(current) = self.selected_browser().map(|b| b.url.clone()) else {
            return false;
        };
        let Some(next) = normalize_browser_url(url) else {
            return false;
        };
        let next = (!next.is_empty()).then_some(next);
        if current == next {
            return false;
        }
        self.patch_selected_browser(json!({ "url": next }))
    }

    /// Switch the selected browser to the device preset `id`; a known
    /// preset also resizes the node to its box. `None` clears the preset,
    /// leaving the node's size as it is.
    pub fn set_selected_browser_device(&mut self, id: Option<&str>) -> bool {
        let Some(current) = self.selected_browser().map(|b| b.device_id.clone()) else {
            return false;
        };
        let next = id.map(str::trim).filter(|id| !id.is_empty());
        if current.as_deref() == next {
            return false;
        }
        let mut patch = json!({ "deviceId": next });
        if let Some(device) = next.and_then(browser_device) {
            patch["width"] = json!(device.width);
            patch["height"] = json!(device.height);
        }
        self.patch_selected_browser(patch)
    }

    /// Set the selected browser's page zoom (1 = 100 %, clamped).
    pub fn set_selected_browser_zoom(&mut self, zoom: f64) -> bool {
        let Some(current) = self.selected_browser().map(|b| b.zoom.unwrap_or(1.0)) else {
            return false;
        };
        let next = clamp_browser_zoom(zoom);
        if (current - next).abs() < f64::EPSILON {
            return false;
        }
        self.patch_selected_browser(json!({ "zoom": next }))
    }

    /// Set the selected browser's page scroll in CSS px (clamped at 0).
    /// `None` keeps that axis.
    pub fn set_selected_browser_scroll(&mut self, x: Option<f64>, y: Option<f64>) -> bool {
        let Some(browser) = self.selected_browser() else {
            return false;
        };
        let mut patch = serde_json::Map::new();
        if let Some(x) = x.map(clamp_browser_scroll) {
            if browser.scroll_x.unwrap_or(0.0) != x {
                patch.insert("scrollX".into(), json!(x));
            }
        }
        if let Some(y) = y.map(clamp_browser_scroll) {
            if browser.scroll_y.unwrap_or(0.0) != y {
                patch.insert("scrollY".into(), json!(y));
            }
        }
        if patch.is_empty() {
            return false;
        }
        self.patch_selected_browser(Value::Object(patch))
    }

    fn patch_selected_browser(&mut self, patch: Value) -> bool {
        let node_id = self.selection.anchor.clone();
        if !node_id.is_real() || !self.is_editable(&node_id) {
            return false;
        }
        self.apply(EditorCommand::PatchNodeData {
            node_id,
            patch_json: patch.to_string(),
            page_id: None,
        })
    }
}

#[cfg(test)]
#[path = "browser_edit_tests.rs"]
mod tests;
