//! Track C-1 of the interactive-preview plan — auto-wire App Mode on
//! preview entry.
//!
//! `PreviewSession::enter` already turns a document with authored
//! `screen` markers into a routed multi-screen App Mode session (see the
//! module doc on `super::PreviewSession::enter`). Most generated documents
//! now pick up those markers for free — Track A's
//! `de_orchestrator::wire_screen_navigation::wire_screen_navigation` runs at
//! the end of every generation turn (`de_orchestrator::cleanup::
//! run_cleanup_passes`) — but a hand-drawn document, an import, or anything
//! authored before Track A shipped still has none, so preview falls back to
//! the classic single-page workbench view even though the doc plainly has
//! several screen-shaped top-level frames.
//!
//! [`auto_wire_for_preview`] closes that gap AT PREVIEW-ENTRY TIME: it
//! wires an `EditorState` built from a clone of the document and hands the
//! result back. `enter` then builds the runtime from that wired clone
//! instead of the original — the caller-visible (saved) document is never
//! touched, matching `enter`'s existing "never mutates the saved doc"
//! invariant (see the module doc on `preview/mod.rs`).
//!
//! The editor hosts name the board Preview opens on; then the entry's site
//! is routed and its links bound ("Entering on a board" below). Without a
//! board (MCP capture without `entryNodeId`) the older rule holds: when the
//! page carries no authored `screen` marker, the generation pass and the
//! website link pass (`wire_web_nav_links`, #153) run on the clone.
//!
//! ## Entering on a board (#223)
//!
//! The editor hosts always name the board Preview opens on. Then the
//! routed boards are that board's site (`de_editor_core::
//! preview_screen_set`), component instances are expanded before wiring
//! so a shared header's links bind on every page, and the pass fills gaps
//! around authored markers instead of stepping aside: the panel's "Mark
//! as screen" / "Set as start screen" controls write single markers, and
//! one marker must not switch every other page off. The rule below is
//! kept for entries without a board (MCP capture without `entryNodeId`).
//!
//! ## Why "any authored marker → skip entirely", not Track A's own
//! "fill only the gaps" rule
//!
//! `wire_screen_navigation` itself is additive-only per node (contract
//! point 4 in `de_orchestrator::wire_screen_navigation`): it fills in
//! whatever is missing even when SOME screens already carry a marker. At
//! generation time that is exactly right — a multi-turn document
//! accumulates screens incrementally and each turn should link the new
//! ones in. At PREVIEW-ENTRY time it is deliberately stricter: an author
//! who has started marking screens by hand is mid-way through an
//! intentional App Mode setup, and silently binding additional nav taps
//! behind their back on every preview open would be a confusing,
//! preview-only side effect they never asked for and can't see in the
//! editor. So this module's gate is document-wide ("does ANY authored
//! marker exist"), not per-node — user/model intent wins outright rather
//! than being merged with.
//! Generation-only back/card completion is deliberately not part of this
//! cloned-state fallback; those interactions are persisted by cleanup.
//!
//! ## Why de-host-native can depend on de-orchestrator
//!
//! `de-host-native` is not a wasm32 crate (`de-host-web` does not depend on
//! it), and `de-orchestrator`'s own dependency graph never reaches back to
//! `de-host-native` (verified: `de-editor-core` / `de-design-lint` /
//! `de-mcp` / `de-ai-skills` / `de-pen-loader` / jian schema-only crates —
//! no cycle). The dependency is gated in the same
//! `[target.'cfg(not(target_arch = "wasm32"))']` block as `jian-core` /
//! `de-pen-loader` in `Cargo.toml`.

use de_editor_core::{EditorCommand, EditorState, NodeId};
use de_orchestrator::wire_screen_navigation::{
    wire_preview_screens, wire_screen_navigation, wire_web_nav_links,
};
use de_orchestrator::DocSink;
use jian_ops_schema::node::PenNode;
use jian_ops_schema::PenDocument;

/// Minimal [`DocSink`] wrapping a borrowed [`EditorState`] — mirrors
/// `de_orchestrator::loop_finalize::StateDocSink`, duplicated here because
/// that type is `pub(crate)` to `de-orchestrator` and this preview-entry
/// hook is the only caller outside that crate.
struct PreviewDocSink<'a> {
    state: &'a mut EditorState,
}

impl DocSink for PreviewDocSink<'_> {
    fn state(&self) -> &EditorState {
        self.state
    }
    fn apply(&mut self, cmd: EditorCommand) -> bool {
        self.state.apply(cmd)
    }
    fn insert_subtree_returning_root_ids(
        &mut self,
        nodes: Vec<PenNode>,
        parent_id: &NodeId,
    ) -> Option<Vec<String>> {
        self.state
            .insert_subtree_returning_root_ids(nodes, parent_id)
    }
    fn begin_undo_batch(&mut self) {}
    fn end_undo_batch(&mut self) {}
}

/// True if any top-level frame on the active page already carries an
/// authored `screen` marker — the escape hatch that makes this whole
/// module a no-op once an author has started marking screens by hand.
fn has_authored_screen_marker(state: &EditorState) -> bool {
    state
        .active_children()
        .iter()
        .any(|node| matches!(node, PenNode::Frame(f) if f.screen.is_some()))
}

/// The preview-only wired copy of `doc`, or `None` to preview `doc` as is.
/// Never mutates `doc` itself.
///
/// With an entry board (#223) the boards Preview browses are that board
/// and the boards of its kind (`de_editor_core::preview_screen_set`): the
/// pass expands component instances first, so links inside a shared
/// header instance can be bound per page, then routes those boards and
/// binds their navigation (`wire_preview_screens`). Authored markers and
/// events are kept and gaps around them are filled.
///
/// Without one, it keeps the older rule: when no top-level frame on
/// `active_page_index` carries an authored `screen` marker, Track A's
/// shape-based pass and the website link pass run on the clone;
/// otherwise `None`.
pub(super) fn auto_wire_for_preview(
    doc: &PenDocument,
    active_page_index: usize,
    entry_root_id: Option<&str>,
) -> Option<PenDocument> {
    if let Some(entry) = entry_root_id {
        let roots = de_editor_core::preview_screen_set::page_roots(doc, active_page_index);
        let screens = de_editor_core::preview_screen_set::preview_screen_set(roots, entry);
        if screens.is_empty() {
            return None;
        }
        let expanded = if de_editor_core::ref_resolve::document_has_refs(doc) {
            de_editor_core::ref_resolve::resolve_refs_for_canvas(doc)
        } else {
            doc.clone()
        };
        let mut state = EditorState::from_document(expanded);
        state.ui.active_page_index = active_page_index;
        wire_preview_screens(&mut PreviewDocSink { state: &mut state }, &screens);
        return Some(state.doc);
    }
    let mut state = EditorState::from_document(doc.clone());
    state.ui.active_page_index = active_page_index;
    if has_authored_screen_marker(&state) {
        return None;
    }
    let mut sink = PreviewDocSink { state: &mut state };
    wire_screen_navigation(&mut sink);
    // #153: website header / footer links to pages and same-page sections.
    wire_web_nav_links(&mut sink);
    Some(state.doc)
}

#[cfg(test)]
#[path = "auto_wire_tests.rs"]
mod tests;
