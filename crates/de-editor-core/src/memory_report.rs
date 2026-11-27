//! What the editor's long-lived caches and histories hold (#456).
//!
//! Every host cache reports one [`CacheUsage`] line into a
//! [`MemoryReport`]; the Settings ▸ System memory line and the debug MCP
//! tool read the same report, so a field report comes with numbers.
//!
//! Byte figures are exact where the cache stores raw pixels or bytes
//! (rasters, encoded images) and estimates where it stores Rust structures
//! ([`approx_node_bytes`]); `exact` says which.
//!
//! Computing a report walks the undo history (once per unique shared
//! node), so it is made on demand (a Settings paint is rate-limited by the
//! host, a debug call asks once), never per frame.

use crate::history_snapshot::SharedDoc;
use crate::pen_node_ext::PenNodeExt;
use crate::state::EditorState;
use jian_ops_schema::node::PenNode;
use std::collections::HashSet;
use std::io::Write;
use std::sync::Arc;

/// One cache or history and what it holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheUsage {
    /// Stable identifier (`snake_case`), used by the debug tool's JSON.
    pub id: &'static str,
    /// Bytes held (see [`Self::exact`]).
    pub bytes: u64,
    /// Entries held: rasters, snapshots, messages, …
    pub entries: u64,
    /// The byte budget the cache evicts at, when it has one.
    pub budget: Option<u64>,
    /// `true` when `bytes` counts stored bytes exactly, `false` for an
    /// estimate of Rust structures.
    pub exact: bool,
}

impl CacheUsage {
    /// An exact line.
    pub fn exact(id: &'static str, bytes: u64, entries: u64, budget: Option<u64>) -> Self {
        Self {
            id,
            bytes,
            entries,
            budget,
            exact: true,
        }
    }

    /// An estimated line.
    pub fn estimate(id: &'static str, bytes: u64, entries: u64, budget: Option<u64>) -> Self {
        Self {
            id,
            bytes,
            entries,
            budget,
            exact: false,
        }
    }
}

/// Every cache line one host reported, in report order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MemoryReport {
    pub caches: Vec<CacheUsage>,
}

impl MemoryReport {
    pub fn push(&mut self, usage: CacheUsage) {
        self.caches.push(usage);
    }

    /// Sum of every line's bytes.
    pub fn total_bytes(&self) -> u64 {
        self.caches.iter().map(|c| c.bytes).sum()
    }

    /// The `n` largest lines, largest first (ties keep report order).
    pub fn largest(&self, n: usize) -> Vec<&CacheUsage> {
        let mut lines: Vec<&CacheUsage> = self.caches.iter().filter(|c| c.bytes > 0).collect();
        lines.sort_by(|a, b| b.bytes.cmp(&a.bytes));
        lines.truncate(n);
        lines
    }

    /// The report as a JSON object for the debug tool:
    /// `{"totalBytes": n, "caches": [{id, bytes, entries, budget, exact}]}`.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "totalBytes": self.total_bytes(),
            "caches": self.caches.iter().map(|c| serde_json::json!({
                "id": c.id,
                "bytes": c.bytes,
                "entries": c.entries,
                "budget": c.budget,
                "exact": c.exact,
            })).collect::<Vec<_>>(),
        })
    }
}

/// Estimated heap bytes of one node subtree: the inline size of every node
/// plus the bytes of its serialized form, which tracks the strings, vectors
/// and maps hanging off it. Calibrated against a counting allocator in
/// `de-host-services/tests/large_document_memory.rs`.
pub fn approx_node_bytes(node: &PenNode) -> u64 {
    let inline = subtree_nodes(node) * std::mem::size_of::<PenNode>() as u64;
    let mut counter = ByteCounter(0);
    // Serialization into a counter cannot fail on I/O; a node that does
    // not serialize counts its inline size only.
    let _ = serde_json::to_writer(&mut counter, node);
    inline + counter.0
}

/// Heap bytes a node holds beyond its inline size, on average (#412's
/// document: 2,931 bytes per node with a 2,792-byte `PenNode`).
const NODE_OUTLINE_BYTES: u64 = 160;

/// [`approx_node_bytes`] from the node count alone: a walk with no
/// serialization, for budgets checked on the UI thread. Leaves out large
/// strings a node carries (long text, inline data).
pub fn approx_node_bytes_fast(node: &PenNode) -> u64 {
    subtree_nodes(node) * (std::mem::size_of::<PenNode>() as u64 + NODE_OUTLINE_BYTES)
}

fn subtree_nodes(node: &PenNode) -> u64 {
    1 + node
        .children()
        .map(|children| children.iter().map(subtree_nodes).sum::<u64>())
        .unwrap_or(0)
}

/// A `Write` sink that only counts.
struct ByteCounter(u64);

impl Write for ByteCounter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0 += buf.len() as u64;
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Snapshots and estimated bytes the undo / redo history holds beyond the
/// live document: each shared top-level node is counted once, however many
/// snapshots point at it, plus every snapshot's own variable-ref maps.
pub fn history_usage(state: &EditorState) -> CacheUsage {
    let mut seen: HashSet<*const PenNode> = HashSet::new();
    let mut bytes = 0u64;
    let snapshots = state.history.past.iter().chain(state.history.future.iter());
    let mut count = 0u64;
    for snapshot in snapshots {
        count += 1;
        bytes += shared_doc_new_bytes(&snapshot.doc, &mut seen);
        let refs = snapshot.fill_refs.len() + snapshot.stroke_refs.len();
        // Key (NodeId) + value (String) + table slot, per entry.
        bytes += refs as u64 * 96;
    }
    CacheUsage::estimate("undo_history", bytes, count, None)
}

fn shared_doc_new_bytes(doc: &SharedDoc, seen: &mut HashSet<*const PenNode>) -> u64 {
    let mut bytes = 0;
    let mut page = 0;
    let mut visit = |nodes: &[Arc<PenNode>]| {
        for node in nodes {
            if seen.insert(Arc::as_ptr(node)) {
                bytes += approx_node_bytes(node);
            }
        }
    };
    visit(doc.root_children());
    while let Some(nodes) = doc.page_children(page) {
        visit(nodes);
        page += 1;
    }
    bytes
}

/// Messages and their text across every chat tab.
pub fn chat_usage(state: &EditorState) -> CacheUsage {
    use crate::chat::{ChatImage, ChatMessage, ChatToolCall};
    let mut messages = 0u64;
    let mut bytes = 0usize;
    for tab in state.chat.tabs() {
        for message in &tab.messages {
            messages += 1;
            bytes += std::mem::size_of::<ChatMessage>()
                + message.content.capacity()
                + message.thinking.capacity()
                + message
                    .tool_calls
                    .iter()
                    .map(|call| {
                        std::mem::size_of::<ChatToolCall>() + call.name.len() + call.args.len()
                    })
                    .sum::<usize>()
                + message
                    .activities
                    .iter()
                    .map(|a| {
                        std::mem::size_of_val(a)
                            + a.id.len()
                            + a.title.len()
                            + a.detail.as_ref().map_or(0, String::len)
                    })
                    .sum::<usize>()
                + message
                    .images
                    .iter()
                    .map(|image| std::mem::size_of::<ChatImage>() + image.data.len())
                    .sum::<usize>();
        }
    }
    CacheUsage::estimate("chat_transcripts", bytes as u64, messages, None)
}

#[cfg(test)]
#[path = "memory_report_tests.rs"]
mod tests;
