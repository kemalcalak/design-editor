//! When a live browser page (#20) needs a new snapshot: the scheduling
//! both hosts share. A host calls [`BrowserRenderQueue::pump`] once per
//! frame, sends the returned requests to its renderer (desktop: worker
//! threads with headless Chrome; web: the daemon's snapshot route) and
//! reports each result through [`BrowserRenderQueue::finish`], which puts
//! the PNG in the image cache and marks it ready for the painter.
//!
//! - Only nodes the last painted page frame showed are considered
//!   (`browser_textures`), so off-screen pages never load.
//! - A quiet frame (same document revision, reloads, load gate and visible
//!   set) walks nothing.
//! - A changed request waits [`DEBOUNCE_MS`] so typing an address or
//!   dragging a size does not start a render per step; a node with no
//!   snapshot yet, or a Reload, starts at once. The old snapshot stays up
//!   until the new one lands.
//! - A failed key is not retried until the request changes or the user
//!   presses Reload.

use std::collections::HashMap;

use de_editor_core::browser_live::{
    browser_render_request, BrowserPageStatus, BrowserRenderRequest,
};
use de_editor_core::{EditorState, NodeId};

use crate::widgets::browser_textures;
use crate::widgets::canvas_viewport_image::has_cached_image_bytes;

/// Quiet time after a change before its page renders.
pub const DEBOUNCE_MS: f64 = 300.0;

/// How one render ended.
#[derive(Debug, Clone, PartialEq)]
pub enum BrowserRenderOutcome {
    /// The PNG snapshot.
    Ready(Vec<u8>),
    /// No browser to render with.
    NoBrowser,
    /// The renderer refused the address (web daemon URL screen).
    Blocked,
    Failed,
}

/// Inputs of a scan of the visible nodes. The visible set itself is
/// compared separately.
#[derive(Clone, Copy, PartialEq, Eq)]
struct ScanStamp {
    revision: u64,
    reload_generation: u64,
    pages_allowed: bool,
}

pub struct BrowserRenderQueue {
    max_in_flight: usize,
    /// The browser nodes the last painted page frame showed.
    visible: Vec<String>,
    /// Requests waiting for their debounce, by node, with their due time.
    wanted: HashMap<String, (BrowserRenderRequest, f64)>,
    /// Key being rendered, by node.
    in_flight: HashMap<String, u64>,
    /// Last key that failed, by node.
    failed: HashMap<String, u64>,
    /// Reload count last seen, by node: a new Reload skips the debounce.
    reloads_seen: HashMap<String, u32>,
    last_scan: Option<ScanStamp>,
    document_generation: u64,
}

impl BrowserRenderQueue {
    /// A queue that keeps at most `max_in_flight` renders running.
    pub fn new(max_in_flight: usize) -> Self {
        Self {
            max_in_flight: max_in_flight.max(1),
            visible: Vec::new(),
            wanted: HashMap::new(),
            in_flight: HashMap::new(),
            failed: HashMap::new(),
            reloads_seen: HashMap::new(),
            last_scan: None,
            document_generation: 0,
        }
    }

    /// Renders running or waiting out their debounce.
    pub fn is_pending(&self) -> bool {
        !self.in_flight.is_empty() || !self.wanted.is_empty()
    }

    /// Requests waiting out their debounce: a host must come back without
    /// other input (renders in flight wake the host when they land).
    pub fn is_waiting(&self) -> bool {
        !self.wanted.is_empty()
    }

    /// Renders running now.
    pub fn in_flight(&self) -> usize {
        self.in_flight.len()
    }

    /// Per-frame step at `now_ms` (any monotonic clock). Returns whether
    /// the panel needs a repaint (a status changed) and the requests to
    /// render now; each is marked in flight until [`Self::finish`].
    pub fn pump(
        &mut self,
        state: &mut EditorState,
        now_ms: f64,
    ) -> (bool, Vec<BrowserRenderRequest>) {
        if state.document_generation() != self.document_generation {
            // A new document: its node ids name other nodes.
            self.document_generation = state.document_generation();
            browser_textures::clear_snapshots();
            self.wanted.clear();
            self.failed.clear();
            self.reloads_seen.clear();
            self.visible.clear();
            self.last_scan = None;
        }
        let mut visible_changed = false;
        if let Some(visible) = browser_textures::take_visible() {
            if visible != self.visible {
                self.visible = visible;
                self.wanted
                    .retain(|id, _| self.visible.binary_search(id).is_ok());
                visible_changed = true;
            }
        }
        let live = &state.editor_ui.browser_live;
        let stamp = ScanStamp {
            revision: state.document_revision(),
            reload_generation: live.reload_generation(),
            pages_allowed: live.pages_allowed(),
        };
        let quiet = !visible_changed && self.last_scan == Some(stamp);
        self.last_scan = Some(stamp);
        let mut forgot = false;
        if !quiet && !self.visible.is_empty() && stamp.pages_allowed {
            forgot = self.scan(state, now_ms);
        }
        let (dispatched, requests) = self.dispatch_due(state, now_ms);
        (dispatched || forgot, requests)
    }

    /// Queue the visible nodes whose picture is out of date. Returns
    /// whether a snapshot was dropped (a node whose address was cleared).
    fn scan(&mut self, state: &EditorState, now_ms: f64) -> bool {
        let mut forgot = false;
        for id in &self.visible {
            let Some(node) =
                de_editor_core::walkers::find_node(state.active_children(), &NodeId::new(id))
            else {
                continue;
            };
            let reload = state.editor_ui.browser_live.reload_count(id);
            let reloaded = self.reloads_seen.insert(id.clone(), reload) != Some(reload);
            let Some(request) = browser_render_request(node, reload) else {
                // No address (or no box): the old page must not stay up.
                forgot |= browser_textures::forget(id);
                self.wanted.remove(id);
                continue;
            };
            let shown = browser_textures::ready(id)
                .filter(|(_, texture)| has_cached_image_bytes(*texture))
                .map(|(key, _)| key);
            if shown == Some(request.key)
                || self.in_flight.get(id) == Some(&request.key)
                || self.failed.get(id) == Some(&request.key)
            {
                continue;
            }
            match self.wanted.get(id) {
                Some((queued, _)) if queued.key == request.key => {}
                _ => {
                    // A first picture or a Reload starts at once; any other
                    // change waits out the debounce.
                    let due = if shown.is_none() || reloaded {
                        now_ms
                    } else {
                        now_ms + DEBOUNCE_MS
                    };
                    self.wanted.insert(id.clone(), (request, due));
                }
            }
        }
        forgot
    }

    fn dispatch_due(
        &mut self,
        state: &mut EditorState,
        now_ms: f64,
    ) -> (bool, Vec<BrowserRenderRequest>) {
        if self.wanted.is_empty() {
            return (false, Vec::new());
        }
        let mut due: Vec<String> = self
            .wanted
            .iter()
            .filter(|(id, (_, at))| *at <= now_ms && !self.in_flight.contains_key(*id))
            .map(|(id, _)| id.clone())
            .collect();
        due.sort_unstable();
        let mut out = Vec::new();
        for id in due {
            if self.in_flight.len() >= self.max_in_flight {
                break;
            }
            let Some((request, _)) = self.wanted.remove(&id) else {
                continue;
            };
            self.in_flight.insert(id.clone(), request.key);
            state
                .editor_ui
                .browser_live
                .set_status(&id, BrowserPageStatus::Loading);
            out.push(request);
        }
        (!out.is_empty(), out)
    }

    /// Report how the render of `key` for `node_id` ended.
    pub fn finish(
        &mut self,
        state: &mut EditorState,
        node_id: &str,
        key: u64,
        outcome: BrowserRenderOutcome,
    ) {
        if self.in_flight.get(node_id) == Some(&key) {
            self.in_flight.remove(node_id);
        }
        let status = match outcome {
            BrowserRenderOutcome::Ready(png) => {
                browser_textures::install(node_id, key, png);
                self.failed.remove(node_id);
                BrowserPageStatus::Ready
            }
            failure => {
                self.failed.insert(node_id.to_string(), key);
                match failure {
                    BrowserRenderOutcome::NoBrowser => BrowserPageStatus::NoBrowser,
                    BrowserRenderOutcome::Blocked => BrowserPageStatus::Blocked,
                    _ => BrowserPageStatus::Failed,
                }
            }
        };
        state.editor_ui.browser_live.set_status(node_id, status);
        // Re-check the visible nodes: this one may already want a newer key.
        self.last_scan = None;
    }
}

#[cfg(test)]
#[path = "browser_render_queue_tests.rs"]
mod tests;
