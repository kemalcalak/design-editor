//! Website-style browsing over an APP MODE preview (#153).
//!
//! A routed preview already switches screens when an authored `push` /
//! `replace` / `pop` action runs. This module adds what a browser has on
//! top of that:
//!
//! - [`BrowseHistory`]: back / forward over every committed screen
//!   switch, whatever verb caused it. A link wired with `replace` (the
//!   auto-wire default) never grows the router stack, so the router alone
//!   cannot go back; the history can.
//! - the address the chrome shows ([`BrowseAddress`]: page title + route).
//! - which node a hover or press lands on as a *clickable* (the nearest
//!   ancestor-or-self with a tap handler, or an interactive widget), so
//!   hosts show a pointer cursor and paint a hover state.
//! - section links: a `scroll_to` aimed at the page itself is handed to
//!   the host as a [`PageScrollRequest`], because the host owns the page
//!   scroll (device frame) or the viewport (Canvas mode).
//!
//! Everything here reads or drives the preview runtime only; the editor
//! document is never touched.

use de_editor_core::PenNodeExt;
use de_editor_ui::Rect;
use jian_core::action::services::Router;
use jian_ops_schema::node::PenNode;

use crate::{InvalidationKind, PreviewSession};

/// Upper bound on history entries; the oldest entry is dropped beyond it.
const HISTORY_CAP: usize = 64;

/// One visited screen and the router stack depth it was committed at.
#[derive(Debug, Clone, PartialEq, Eq)]
struct HistoryEntry {
    path: String,
    depth: usize,
}

/// Browser-style history over committed screen switches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BrowseHistory {
    entries: Vec<HistoryEntry>,
    index: usize,
    /// The entry index a back / forward step is travelling to, so the
    /// switch it causes moves the cursor instead of adding an entry.
    pending: Option<usize>,
}

impl BrowseHistory {
    pub(crate) fn new(entry_path: &str, depth: usize) -> Self {
        Self {
            entries: vec![HistoryEntry {
                path: entry_path.to_owned(),
                depth,
            }],
            index: 0,
            pending: None,
        }
    }

    pub(crate) fn can_go_back(&self) -> bool {
        self.index > 0
    }

    pub(crate) fn can_go_forward(&self) -> bool {
        self.index + 1 < self.entries.len()
    }

    /// Record a committed switch to `path` at router depth `depth`. A
    /// switch that lands on the entry a pending back / forward step aimed
    /// at moves the cursor there; anything else is a new navigation that
    /// drops the forward entries, like a browser.
    pub(crate) fn record(&mut self, path: &str, depth: usize) {
        if let Some(target) = self.pending.take() {
            if self.entries.get(target).is_some_and(|e| e.path == path) {
                self.index = target;
                self.entries[target].depth = depth;
                return;
            }
        }
        if self.entries[self.index].path == path {
            self.entries[self.index].depth = depth;
            return;
        }
        self.entries.truncate(self.index + 1);
        self.entries.push(HistoryEntry {
            path: path.to_owned(),
            depth,
        });
        if self.entries.len() > HISTORY_CAP {
            self.entries.remove(0);
        }
        self.index = self.entries.len() - 1;
    }

    fn step_target(&self, forward: bool) -> Option<usize> {
        if forward {
            self.can_go_forward().then_some(self.index + 1)
        } else {
            self.index.checked_sub(1)
        }
    }

    fn current_depth(&self) -> usize {
        self.entries[self.index].depth
    }

    /// Start a back / forward step: the entry it travels to, or `None` at
    /// either end and while an earlier step has not committed yet (a
    /// second step from the same, stale cursor would push the same path
    /// twice — held `Cmd+]` autorepeat, a double mouse-forward click).
    fn begin_step(&mut self, forward: bool) -> Option<HistoryEntry> {
        if self.pending.is_some() {
            return None;
        }
        let target = self.step_target(forward)?;
        self.pending = Some(target);
        Some(self.entries[target].clone())
    }
}

/// What the preview chrome's address bar shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowseAddress {
    /// The mounted screen's route, e.g. `/pricing`.
    pub route: String,
    /// The mounted screen's page name, or the route's slug when unnamed.
    pub title: String,
}

/// A section link's request to scroll the page itself.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PageScrollRequest {
    /// The page offset (scene px from the page's top, never negative)
    /// that puts the section's top at the top of the visible page. The
    /// host clamps it to its own scroll range's upper end.
    pub offset: f32,
    /// The page root's scene rect, for hosts that pan a viewport instead
    /// of scrolling a frame (Canvas mode).
    pub page_rect: Rect,
}

/// The pointer shape a preview hover asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewCursor {
    Default,
    /// Over a link, button or interactive widget.
    Pointer,
    /// Over a text field.
    Text,
}

impl PreviewSession {
    /// Whether the chrome's back control has somewhere to go.
    pub fn can_go_back(&self) -> bool {
        self.app
            .as_ref()
            .is_some_and(|app| app.history.can_go_back())
    }

    /// Whether the chrome's forward control has somewhere to go.
    pub fn can_go_forward(&self) -> bool {
        self.app
            .as_ref()
            .is_some_and(|app| app.history.can_go_forward())
    }

    /// Go back one history entry. The switch commits on the next
    /// `reconcile`, like any other navigation. A no-op (returning
    /// `None` invalidation) at the oldest entry or outside APP MODE.
    pub fn go_back(&mut self) -> InvalidationKind {
        self.history_step(false)
    }

    /// Go forward one history entry; see [`Self::go_back`].
    pub fn go_forward(&mut self) -> InvalidationKind {
        self.history_step(true)
    }

    fn history_step(&mut self, forward: bool) -> InvalidationKind {
        let Some(app) = self.app.as_mut() else {
            return InvalidationKind::None;
        };
        let current_depth = app.history.current_depth();
        let Some(entry) = app.history.begin_step(forward) else {
            return InvalidationKind::None;
        };
        let stack = app.router.current().stack;
        // Prefer the verb that keeps the router stack honest, so an
        // authored in-design back button still pops to the right screen
        // after a chrome back / forward.
        let pops_to_target = stack.len() > 1 && stack[stack.len() - 2] == entry.path;
        if !forward && pops_to_target && entry.depth < current_depth {
            app.router.pop();
        } else if forward && entry.depth > current_depth {
            app.router.push(&entry.path);
        } else {
            app.router.replace(&entry.path);
        }
        InvalidationKind::Navigation
    }

    /// The mounted screen's address, or `None` outside APP MODE.
    pub fn browse_address(&self) -> Option<BrowseAddress> {
        let app = self.app.as_ref()?;
        let route = app.current_path.clone();
        let title = app
            .promoted_doc
            .pages
            .as_ref()
            .and_then(|pages| pages.get(app.page_idx))
            .map(|page| page.name.trim().to_owned())
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| route.trim_start_matches('/').to_owned());
        Some(BrowseAddress { route, title })
    }

    /// The schema id of the clickable the mouse is on: the nearest
    /// ancestor-or-self of the hovered node — or, while a button is held
    /// (a press clears hover), of the pressed node — that carries a tap
    /// handler or is an interactive widget. `None` when nothing clickable
    /// is under the pointer.
    pub fn hovered_clickable(&self) -> Option<String> {
        self.pointer_node()
            .and_then(|node| self.clickable_ancestor(&node))
    }

    /// The node under the pointer: the hover sample, else the first
    /// pressed node.
    fn pointer_node(&self) -> Option<String> {
        self.interaction
            .hovered_node()
            .or_else(|| self.interaction.pressed_nodes().into_iter().next())
            .map(str::to_owned)
    }

    /// The pointer shape for the node under the pointer.
    pub fn preview_cursor(&self) -> PreviewCursor {
        let Some(hovered) = self.pointer_node() else {
            return PreviewCursor::Default;
        };
        let hovered = hovered.as_str();
        let mut cursor = PreviewCursor::Default;
        self.walk_ancestors(hovered, |node| {
            if matches!(
                node,
                PenNode::TextInput(_) | PenNode::TextArea(_) | PenNode::NumberInput(_)
            ) {
                cursor = PreviewCursor::Text;
                return true;
            }
            if is_clickable(node) {
                cursor = PreviewCursor::Pointer;
                return true;
            }
            false
        });
        cursor
    }

    /// Resolve queued `scroll_to` actions and return the one aimed at the
    /// page itself, if any. Hosts call this once per frame after
    /// `reconcile` and scroll their page (or pan their viewport) to it.
    pub fn take_page_scroll_request(&self) -> Option<PageScrollRequest> {
        let (root_id, offset) = self
            .binding_overlay
            .take_page_scroll_request(&self.scene, &self.ui_actions)?;
        let page_rect = self.scene.active_page()?.find(&root_id)?.bounds;
        Some(PageScrollRequest { offset, page_rect })
    }

    pub(crate) fn clickable_ancestor(&self, id: &str) -> Option<String> {
        let mut found = None;
        self.walk_ancestors(id, |node| {
            if is_clickable(node) {
                found = Some(jian_core::document::tree::node_schema_id(node).to_owned());
                return true;
            }
            false
        });
        found
    }

    /// Visit `id`'s runtime node and then each ancestor until `visit`
    /// returns `true`.
    fn walk_ancestors(&self, id: &str, mut visit: impl FnMut(&PenNode) -> bool) {
        let Some(document) = self.runtime.document.as_ref() else {
            return;
        };
        let mut key = document.tree.by_id.get(id).copied();
        while let Some(current) = key {
            let Some(data) = document.tree.nodes.get(current) else {
                return;
            };
            if visit(&data.schema) {
                return;
            }
            key = data.parent;
        }
    }
}

/// A node a click does something on: a tap / double-tap / long-press
/// handler, or a widget whose own runtime reacts to taps.
fn is_clickable(node: &PenNode) -> bool {
    if matches!(
        node,
        PenNode::Switch(_)
            | PenNode::Checkbox(_)
            | PenNode::Select(_)
            | PenNode::RadioGroup(_)
            | PenNode::Tabs(_)
            | PenNode::Slider(_)
    ) {
        return true;
    }
    let has = |list: &Option<jian_ops_schema::events::ActionList>| {
        list.as_ref().is_some_and(|actions| !actions.is_empty())
    };
    node.events().is_some_and(|events| {
        has(&events.on_tap) || has(&events.on_double_tap) || has(&events.on_long_press)
    })
}

#[cfg(test)]
#[path = "browse_history_tests.rs"]
mod history_tests;
