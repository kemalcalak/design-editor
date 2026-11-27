//! A byte budget for the undo history (#456).
//!
//! Snapshots share every unchanged top-level node with their neighbours
//! (`history_snapshot`), so 100 steps of small edits cost little. What grows
//! is a document whose edits land in one large top-level board: each step
//! keeps its own copy of that board, and 100 steps of a 14,000-node board
//! are gigabytes. [`EditorState::trim_history_to_budget`] drops the oldest
//! undo steps while the history holds more than [`HISTORY_BYTE_BUDGET`]
//! beyond the document itself, never below [`HISTORY_MIN_STEPS`].
//!
//! It walks every snapshot once, so it is not run per edit: the desktop
//! calls it after the editor has been idle for a while
//! ([`crate::memory_idle::IdleTrim`]), which keeps edits, undo and redo at
//! their old cost, and the file-backed MCP server every tenth write
//! ([`EditorState::trim_history_after_write`]). The dropped steps come back
//! to the caller, which frees them off its UI thread: a step that held the
//! only copy of a large board takes tens of milliseconds to free.
//!
//! What a step is counted at (estimates, `memory_report::approx_node_bytes_fast`):
//! each shared top-level node once however many steps hold it (node count ×
//! the measured per-node size, 2,952 bytes on #412's document), plus the
//! step's variable-ref maps. Not counted: the component library a step
//! keeps (`SharedComponents`, shared by `Arc` between steps and copied only
//! when a component changes), the document skeleton each step clones
//! (variables, themes, page metadata; small unless a document holds very
//! large variable sets), and strings inside nodes beyond the per-node
//! average (long text, inline `data:` images). A history heavy in those
//! reaches the budget later than its real size.

use std::collections::HashMap;
use std::sync::Arc;

use jian_ops_schema::node::PenNode;

use crate::history::EditorSnapshot;
use crate::memory_report::approx_node_bytes_fast;
use crate::state::EditorState;

/// Bytes the undo history may hold beyond one copy of the document.
pub const HISTORY_BYTE_BUDGET: u64 = 512 * 1024 * 1024;

/// How often the file-backed MCP server trims: every this many writes.
pub const TRIM_EVERY_WRITES: u64 = 10;

/// Undo steps always kept, however large.
pub const HISTORY_MIN_STEPS: usize = 20;

/// Estimated bytes of a snapshot's per-step maps (variable-ref caches).
fn own_bytes(snapshot: &EditorSnapshot) -> u64 {
    (snapshot.fill_refs.len() + snapshot.stroke_refs.len()) as u64 * 96
}

/// The shared top-level nodes of one snapshot.
fn nodes(snapshot: &EditorSnapshot) -> impl Iterator<Item = &Arc<PenNode>> {
    let doc = &snapshot.doc;
    let pages = (0..).map_while(move |page| doc.page_children(page));
    doc.root_children().iter().chain(pages.flatten())
}

impl EditorState {
    /// Drop the oldest undo steps while the history holds more than
    /// `budget` bytes beyond one copy of the document, keeping at least
    /// `min_steps`. Redo steps are never dropped. Returns the dropped
    /// steps for the caller to free, off its UI thread.
    pub fn trim_history_to_budget(&mut self, budget: u64, min_steps: usize) -> Vec<EditorSnapshot> {
        if self.history.past.len() <= min_steps {
            return Vec::new();
        }
        // How many snapshots hold each shared node, and the bytes of each.
        let mut holders: HashMap<*const PenNode, (u32, u64)> = HashMap::new();
        let mut total = 0u64;
        for snapshot in self.history.past.iter().chain(self.history.future.iter()) {
            total += own_bytes(snapshot);
            for node in nodes(snapshot) {
                let entry = holders
                    .entry(Arc::as_ptr(node))
                    .or_insert_with(|| (0, approx_node_bytes_fast(node)));
                if entry.0 == 0 {
                    total += entry.1;
                }
                entry.0 += 1;
            }
        }
        let document: u64 = self
            .doc
            .pages
            .iter()
            .flatten()
            .flat_map(|page| page.children.iter())
            .chain(self.doc.children.iter())
            .map(approx_node_bytes_fast)
            .sum();
        let limit = budget.saturating_add(document);
        let mut dropped = Vec::new();
        while total > limit && self.history.past.len() > min_steps {
            let Some(oldest) = self.history.past.pop_front() else {
                break;
            };
            total = total.saturating_sub(own_bytes(&oldest));
            for node in nodes(&oldest) {
                if let Some(entry) = holders.get_mut(&Arc::as_ptr(node)) {
                    entry.0 -= 1;
                    if entry.0 == 0 {
                        total = total.saturating_sub(entry.1);
                    }
                }
            }
            dropped.push(oldest);
        }
        dropped
    }

    /// The budget for a host with no idle loop and no UI thread (the
    /// file-backed MCP server): every [`TRIM_EVERY_WRITES`]-th write, by
    /// the host's own `write_count`, so a write pays the walk rarely.
    pub fn trim_history_after_write(&mut self, write_count: u64) -> Vec<EditorSnapshot> {
        if !write_count.is_multiple_of(TRIM_EVERY_WRITES) {
            return Vec::new();
        }
        self.trim_history_to_budget(HISTORY_BYTE_BUDGET, HISTORY_MIN_STEPS)
    }
}

#[cfg(test)]
#[path = "history_budget_tests.rs"]
mod tests;
