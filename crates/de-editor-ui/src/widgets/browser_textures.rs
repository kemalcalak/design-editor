//! Paint ↔ host hand-off for live browser page snapshots (#20).
//!
//! The canvas painter only records which browser nodes it painted in its
//! last page frame ([`begin_frame`], [`note_visible`]) and draws the
//! snapshot last marked ready for a node ([`ready`]). The shared
//! `browser_render_queue` drains the visible set ([`take_visible`]) and,
//! when a host's render lands, [`install`]s the PNG. Nodes the painter culled are never
//! noted, so off-screen pages never load.
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
/// from here on replaces the previous frame's visible set. A `partial`
/// repaint (pan-cache strips and tiles, animated regions) only adds to it.
pub fn begin_frame(partial: bool) {
    if partial {
        return;
    }
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

/// A fresh image-cache id for a snapshot of `node_id`: never reused, so a
/// new picture for the same request (an MCP call changed the page) is not
/// served from the old picture's decoded raster.
fn fresh_texture_id(node_id: &str, key: u64) -> u64 {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let mut hasher = DefaultHasher::new();
    "de-browser-snapshot".hash(&mut hasher);
    node_id.hash(&mut hasher);
    key.hash(&mut hasher);
    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        .hash(&mut hasher);
    hasher.finish()
}

/// Host side: `png` is the newest picture of `node_id` for request `key`.
/// Stores it in the image byte cache, marks it ready and calls the
/// [`set_ready_notifier`] (a picture from another thread, an MCP call,
/// must wake the host to repaint).
pub fn install(node_id: &str, key: u64, png: Vec<u8>) {
    let texture = fresh_texture_id(node_id, key);
    super::canvas_viewport_image::store_remote_image_bytes(texture, png);
    registry().ready.insert(node_id.to_string(), (key, texture));
    if let Some(notify) = NOTIFIER.get() {
        notify();
    }
}

static NOTIFIER: OnceLock<Box<dyn Fn() + Send + Sync>> = OnceLock::new();

/// Register the host's wake-up for [`install`] (once per process).
pub fn set_ready_notifier(notify: impl Fn() + Send + Sync + 'static) {
    let _ = NOTIFIER.set(Box::new(notify));
}

/// Drop the snapshot of `node_id` (its address was cleared).
pub fn forget(node_id: &str) -> bool {
    registry().ready.remove(node_id).is_some()
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
        begin_frame(false);
        note_visible("bt-a");
        note_visible("bt-b");
        note_visible("bt-a");
        let visible: Vec<String> = take_visible()
            .unwrap_or_default()
            .into_iter()
            .filter(|id| id.starts_with("bt-"))
            .collect();
        assert_eq!(visible, ["bt-a", "bt-b"]);
        begin_frame(false);
        assert!(!take_visible()
            .unwrap_or_default()
            .iter()
            .any(|id| id.starts_with("bt-")));
        assert_eq!(ready("bt-a"), None);
        install("bt-a", 7, b"PNG".to_vec());
        let (key, first) = ready("bt-a").unwrap();
        assert_eq!(key, 7);
        install("bt-a", 7, b"PNG2".to_vec());
        let (_, second) = ready("bt-a").unwrap();
        assert_ne!(first, second, "a new picture never reuses a decoded raster");
        assert!(forget("bt-a"));
    }
}
