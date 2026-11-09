//! Interaction mode of a browser node (#324, phase 4): the user drives the
//! node's live page with the mouse and keyboard, like pen.dev. A single
//! click selects a browser node (selected means live); a double-click
//! enters interaction mode, Escape, a click outside the node or any
//! selection change leaves it. The interactive node is
//! `BrowserLiveState::interactive_node`, which also keeps it streaming.
//!
//! These are the shared rules, no transport and no widgets: which node may
//! be driven, the mode's transitions, and how a canvas point maps onto the
//! page ([`PageMapping`]). The hosts' event plumbing lives in
//! `de_editor_ui::widgets::browser_interact_flow`.

use jian_ops_schema::node::PenNode;

use crate::browser_live::{browser_layout, browser_url, BrowserLayout};
use crate::walkers::find_node;
use crate::{EditorState, NodeId};

impl EditorState {
    /// Whether `id` is a browser node the user can drive now: on the
    /// active page with an address, its pages allowed and live pages on
    /// (the page must be streaming to show what the input does).
    pub fn can_interact_with_browser(&self, id: &NodeId) -> bool {
        let live = &self.editor_ui.browser_live;
        live.pages_allowed()
            && live.live_pages()
            && find_node(self.active_children(), id).is_some_and(|node| {
                matches!(node, PenNode::Browser(_)) && browser_url(node).is_some()
            })
    }

    /// The node in interaction mode.
    pub fn interactive_browser(&self) -> Option<NodeId> {
        self.editor_ui
            .browser_live
            .interactive_node()
            .map(NodeId::new)
    }

    /// Enter interaction mode on `id`: it becomes the single selection and
    /// the interactive node. False (nothing changes) when it cannot be
    /// driven ([`Self::can_interact_with_browser`]) or already is.
    pub fn enter_browser_interaction(&mut self, id: NodeId) -> bool {
        if !self.can_interact_with_browser(&id) {
            return false;
        }
        if self.interactive_browser().as_ref() == Some(&id) && self.selection_count() == 1 {
            return false;
        }
        self.set_single_selection(id.clone());
        self.editor_ui
            .browser_live
            .set_interactive_node(Some(id.as_str()));
        true
    }

    /// Leave interaction mode. The node stays selected (and so live).
    /// False when no node was interactive.
    pub fn leave_browser_interaction(&mut self) -> bool {
        if self.editor_ui.browser_live.interactive_node().is_none() {
            return false;
        }
        self.editor_ui.browser_live.set_interactive_node(None);
        true
    }

    /// Leave interaction mode when its node is no longer the single
    /// selection or can no longer be driven (deleted, another page, the
    /// address cleared, pages held, live pages off). Hosts call this once
    /// per frame; it is one comparison while nothing is interactive.
    /// Returns whether the mode ended.
    pub fn sync_browser_interaction(&mut self) -> bool {
        let Some(id) = self.interactive_browser() else {
            return false;
        };
        let still = self.selection_count() == 1
            && self.selection.anchor == id
            && self.can_interact_with_browser(&id);
        !still && self.leave_browser_interaction()
    }

    /// The layout of the interactive node's page, with its id.
    pub fn interactive_browser_layout(&self) -> Option<(NodeId, BrowserLayout)> {
        let id = self.interactive_browser()?;
        let layout = browser_layout(find_node(self.active_children(), &id)?)?;
        Some((id, layout))
    }
}

/// The browser node a canvas press over `hit_path` (root first) lands on:
/// the deepest browser in the path.
pub fn browser_in_hit_path(state: &EditorState, hit_path: &[NodeId]) -> Option<NodeId> {
    hit_path
        .iter()
        .rev()
        .find(|id| {
            find_node(state.active_children(), id).is_some_and(|n| matches!(n, PenNode::Browser(_)))
        })
        .cloned()
}

/// How a browser node's box on the canvas maps onto its page: the page's
/// layout viewport (CSS px, `browser_layout`) fills the node box, so a
/// document point maps by the box's offset and the CSS px per document
/// px. The canvas zoom only enters through the screen → document step
/// the host already does, and the device scale of the stream's frames
/// does not enter at all (input is in CSS px).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PageMapping {
    /// The node box in document px.
    pub box_x: f32,
    pub box_y: f32,
    pub box_w: f32,
    pub box_h: f32,
    /// The page's layout viewport in CSS px.
    pub layout_w: u32,
    pub layout_h: u32,
}

impl PageMapping {
    /// The mapping of a node box `[x, y, w, h]` (document px) laid out
    /// per `layout`. `None` for an empty box.
    pub fn new(doc_box: [f32; 4], layout: &BrowserLayout) -> Option<Self> {
        let [box_x, box_y, box_w, box_h] = doc_box;
        let finite = doc_box.iter().all(|v| v.is_finite());
        (finite && box_w > 0.0 && box_h > 0.0).then_some(Self {
            box_x,
            box_y,
            box_w,
            box_h,
            layout_w: layout.width.max(1),
            layout_h: layout.height.max(1),
        })
    }

    /// CSS px per document px, per axis.
    fn scale(&self) -> (f64, f64) {
        (
            f64::from(self.layout_w) / f64::from(self.box_w),
            f64::from(self.layout_h) / f64::from(self.box_h),
        )
    }

    /// Whether the document point lies on the node box.
    pub fn contains(&self, doc_x: f32, doc_y: f32) -> bool {
        doc_x >= self.box_x
            && doc_y >= self.box_y
            && doc_x <= self.box_x + self.box_w
            && doc_y <= self.box_y + self.box_h
    }

    /// The page point (CSS px from the viewport's top left) under a
    /// document point. Not clamped: a drag that leaves the box keeps
    /// reporting where the pointer is, as a page sees a captured drag.
    pub fn page_point(&self, doc_x: f32, doc_y: f32) -> (f64, f64) {
        let (sx, sy) = self.scale();
        (
            f64::from(doc_x - self.box_x) * sx,
            f64::from(doc_y - self.box_y) * sy,
        )
    }

    /// A scroll of the page by a screen delta at canvas `zoom`, in CSS px:
    /// the page content follows the pointer one screen px per screen px.
    /// `screen_dx` / `screen_dy` move content like a canvas pan (positive
    /// moves it right / down), so the page scrolls the other way.
    pub fn page_scroll(&self, screen_dx: f32, screen_dy: f32, zoom: f32) -> (f64, f64) {
        let zoom = if zoom.is_finite() && zoom > 0.0 {
            f64::from(zoom)
        } else {
            1.0
        };
        let (sx, sy) = self.scale();
        (
            -f64::from(screen_dx) / zoom * sx,
            -f64::from(screen_dy) / zoom * sy,
        )
    }
}

#[cfg(test)]
#[path = "browser_interact_tests.rs"]
mod tests;
