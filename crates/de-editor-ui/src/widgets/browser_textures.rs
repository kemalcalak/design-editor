//! Paint ↔ host hand-off for live browser page snapshots (#20).
//!
//! The canvas painter only records which browser nodes it painted in its
//! last page frame ([`begin_frame`], [`note_visible`]) and draws the
//! snapshot the host last finished for a node ([`ready`]). A host that can
//! render pages drains the visible set ([`take_visible`]), renders off the UI thread, stores the PNG in the
//! shared image byte cache under [`texture_id`] and marks it ready. Nodes
//! the painter culled are never noted, so off-screen pages never load.
//!
//! The previous snapshot stays ready until its replacement lands, so a
//! changed address or size never flashes back to the placeholder.

use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::sync::{Mutex, MutexGuard, OnceLock};

#[derive(Default)]
struct Registry {
    /// A canvas page frame began since the host last looked.
    framed: bool,
    visible: HashSet<String>,
    /// node id → (request key, texture id) of its newest snapshot.
    ready: HashMap<String, (u64, u64)>,
}

static REGISTRY: OnceLock<Mutex<Registry>> = OnceLock::new();

fn registry() -> MutexGuard<'static, Registry> {
    REGISTRY
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

/// Paint side: the editor canvas starts painting the page; what it notes
/// from here on replaces the previous frame's visible set.
pub fn begin_frame() {
    let mut reg = registry();
    if !reg.framed {
        reg.framed = true;
        reg.visible.clear();
    }
}

/// Paint side: `node_id` is on screen this frame.
pub fn note_visible(node_id: &str) {
    let mut reg = registry();
    if !reg.visible.contains(node_id) {
        reg.visible.insert(node_id.to_string());
    }
}

/// Host side: the browser nodes of the canvas frames painted since the
/// last call, sorted; `None` when no page frame was painted meanwhile
/// (the last answer still holds).
pub fn take_visible() -> Option<Vec<String>> {
    let mut reg = registry();
    if !std::mem::take(&mut reg.framed) {
        return None;
    }
    let mut ids: Vec<String> = reg.visible.drain().collect();
    ids.sort_unstable();
    Some(ids)
}

/// The image-cache id a snapshot of `node_id` for request `key` lives under.
pub fn texture_id(node_id: &str, key: u64) -> u64 {
    let mut hasher = DefaultHasher::new();
    "de-browser-snapshot".hash(&mut hasher);
    node_id.hash(&mut hasher);
    key.hash(&mut hasher);
    hasher.finish()
}

/// Host side: the snapshot for `key` is in the image cache.
pub fn set_ready(node_id: &str, key: u64) {
    registry()
        .ready
        .insert(node_id.to_string(), (key, texture_id(node_id, key)));
}

/// `(request key, texture id)` of the newest snapshot of `node_id`.
pub fn ready(node_id: &str) -> Option<(u64, u64)> {
    registry().ready.get(node_id).copied()
}

/// Serializes tests that paint or drain the process-wide registry.
#[cfg(test)]
pub(crate) fn lock_for_tests() -> MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// Forget every snapshot (the document was replaced). The visible set
/// stays: it already describes the new document's frame.
pub fn clear_snapshots() {
    registry().ready.clear();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The registry is process-wide: tests lock it and use their own ids.
    #[test]
    fn visible_nodes_drain_once_and_ready_snapshots_resolve() {
        let _lock = lock_for_tests();
        begin_frame();
        note_visible("bt-a");
        note_visible("bt-b");
        note_visible("bt-a");
        let visible: Vec<String> = take_visible()
            .unwrap_or_default()
            .into_iter()
            .filter(|id| id.starts_with("bt-"))
            .collect();
        assert_eq!(visible, ["bt-a", "bt-b"]);
        begin_frame();
        assert!(!take_visible()
            .unwrap_or_default()
            .iter()
            .any(|id| id.starts_with("bt-")));
        assert_eq!(ready("bt-a"), None);
        set_ready("bt-a", 7);
        assert_eq!(ready("bt-a"), Some((7, texture_id("bt-a", 7))));
        assert_ne!(texture_id("bt-a", 7), texture_id("bt-a", 8));
        assert_ne!(texture_id("bt-a", 7), texture_id("bt-b", 7));
    }
}
