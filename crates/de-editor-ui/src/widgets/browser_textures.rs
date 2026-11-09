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
//!
//! A streamed node (#324) also has a live slot: one stable texture id per
//! node ([`live_texture`]) that the host overwrites with each decoded frame
//! (latest wins) and marks with [`set_live_frame`]. When the stream stops
//! because the node was deselected or scrolled away, or the window went
//! inactive, [`end_live`] keeps its last frame up as
//! the node's picture until the next still snapshot lands ([`install`]),
//! so the canvas never flashes back to an older still. Live frames never enter
//! the encoded-bytes cache; the painter draws the slot when its raster is
//! resident and the snapshot otherwise (while the stream starts, or with
//! the "Live pages" setting off).

use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::sync::{Mutex, MutexGuard, OnceLock};

#[derive(Default)]
struct Registry {
    /// A canvas page frame began since the host last looked.
    framed: bool,
    visible: HashSet<String>,
    /// On-screen size (logical px) of each node in `visible`.
    sizes: HashMap<String, [f32; 2]>,
    /// `sizes` as of the last [`take_visible`].
    last_sizes: HashMap<String, [f32; 2]>,
    /// The last [`take_visible`] found other sizes than the one before.
    sizes_changed: bool,
    /// node id → (request key, texture id) of its newest snapshot.
    ready: HashMap<String, (u64, u64)>,
    /// node id → why its page could not load (#283): the i18n key and its
    /// translation.
    failures: HashMap<String, (&'static str, &'static str)>,
    /// node id → its live slot.
    live: HashMap<String, LiveSlot>,
}

/// One node's live picture.
#[derive(Clone, Copy)]
struct LiveSlot {
    /// Stable image-cache id ([`live_texture`]).
    texture: u64,
    /// Sequence of the newest frame installed.
    seq: u64,
    /// The stream still runs; false for a kept last frame ([`end_live`]).
    streaming: bool,
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
        reg.sizes.clear();
    }
}

/// Paint side: `node_id` is on screen this frame, `size` logical px.
pub fn note_visible(node_id: &str, size: [f32; 2]) {
    let mut reg = registry();
    if !reg.visible.contains(node_id) {
        reg.visible.insert(node_id.to_string());
    }
    match reg.sizes.get_mut(node_id) {
        Some(known) => *known = size,
        None => {
            reg.sizes.insert(node_id.to_string(), size);
        }
    }
}

/// Whether the last [`take_visible`] found the visible nodes at other
/// on-screen sizes than the one before (a zoom).
pub fn visible_sizes_changed() -> bool {
    registry().sizes_changed
}

/// The on-screen size (logical px) `node_id` had in the frames the last
/// [`take_visible`] drained.
pub fn visible_size(node_id: &str) -> Option<[f32; 2]> {
    registry().last_sizes.get(node_id).copied()
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
    let sizes = std::mem::take(&mut reg.sizes);
    reg.sizes_changed = sizes != reg.last_sizes;
    reg.last_sizes = sizes;
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
    let mut reg = registry();
    reg.ready.insert(node_id.to_string(), (key, texture));
    // A kept last frame gives way to the new still; a running stream stays.
    if reg.live.get(node_id).is_some_and(|slot| !slot.streaming) {
        reg.live.remove(node_id);
    }
    drop(reg);
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
    let mut registry = registry();
    registry.failures.remove(node_id);
    registry.ready.remove(node_id).is_some()
}

/// Record why `node_id`'s page could not load (the queue's status line as
/// `(i18n key, translation)`), or clear it once a new page is on its way.
/// The canvas placeholder shows it under the address.
pub fn set_failure(node_id: &str, reason: Option<(&'static str, &'static str)>) {
    let mut registry = registry();
    match reason {
        Some(reason) => registry.failures.insert(node_id.to_string(), reason),
        None => registry.failures.remove(node_id),
    };
}

/// Translate every failure line again (the UI language changed).
pub fn retranslate_failures(translate: impl Fn(&'static str) -> &'static str) {
    for (key, label) in registry().failures.values_mut() {
        *label = translate(key);
    }
}

/// Why `node_id`'s page could not load, if it could not.
pub fn failure(node_id: &str) -> Option<&'static str> {
    registry().failures.get(node_id).map(|(_, label)| *label)
}

/// `(request key, texture id)` of the newest snapshot of `node_id`.
pub fn ready(node_id: &str) -> Option<(u64, u64)> {
    registry().ready.get(node_id).copied()
}

/// The image-cache id a host installs `node_id`'s live frames under: the
/// same for every frame of the node, so a new frame replaces the last one
/// in place instead of growing the cache.
pub fn live_texture(node_id: &str) -> u64 {
    if let Some(slot) = registry().live.get(node_id) {
        return slot.texture;
    }
    let mut hasher = DefaultHasher::new();
    "de-browser-live".hash(&mut hasher);
    node_id.hash(&mut hasher);
    hasher.finish()
}

/// Host side: the frame `seq` of `node_id`'s stream is installed under
/// [`live_texture`].
pub fn set_live_frame(node_id: &str, seq: u64) {
    let texture = live_texture(node_id);
    let mut registry = registry();
    match registry.live.get_mut(node_id) {
        Some(slot) => {
            slot.seq = seq;
            slot.streaming = true;
        }
        None => {
            registry.live.insert(
                node_id.to_string(),
                LiveSlot {
                    texture,
                    seq,
                    streaming: true,
                },
            );
        }
    }
}

/// `(texture id, frame sequence)` of `node_id`'s newest live frame.
pub fn live_frame(node_id: &str) -> Option<(u64, u64)> {
    registry()
        .live
        .get(node_id)
        .map(|slot| (slot.texture, slot.seq))
}

/// Paint side: the texture of `node_id`'s live frame and whether its
/// stream still runs (a kept last frame is a still picture).
pub fn live_for_paint(node_id: &str) -> Option<(u64, bool)> {
    registry()
        .live
        .get(node_id)
        .map(|slot| (slot.texture, slot.streaming))
}

/// Host side: `node_id`'s stream stopped but its last frame stays up as
/// the node's picture until the next still snapshot replaces it.
pub fn end_live(node_id: &str) {
    if let Some(slot) = registry().live.get_mut(node_id) {
        slot.streaming = false;
    }
}

/// Host side: drop every kept last frame (the "Live pages" setting went
/// off or the pages are held). Returns their texture ids.
pub fn forget_kept_live() -> Vec<u64> {
    let mut registry = registry();
    if registry.live.values().all(|slot| slot.streaming) {
        return Vec::new();
    }
    let mut dropped = Vec::new();
    registry.live.retain(|_, slot| {
        if !slot.streaming {
            dropped.push(slot.texture);
        }
        slot.streaming
    });
    dropped
}

/// Host side: `node_id` stopped streaming; its snapshot shows again.
/// Returns the texture id whose raster the host may drop.
pub fn forget_live(node_id: &str) -> Option<u64> {
    registry().live.remove(node_id).map(|slot| slot.texture)
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
    let mut registry = registry();
    registry.ready.clear();
    registry.failures.clear();
    registry.live.clear();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The registry is process-wide: tests lock it and use their own ids.
    #[test]
    fn visible_nodes_drain_once_and_ready_snapshots_resolve() {
        let _lock = lock_for_tests();
        begin_frame(false);
        note_visible("bt-a", [1.0, 1.0]);
        note_visible("bt-b", [1.0, 1.0]);
        note_visible("bt-a", [1.0, 1.0]);
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

    #[test]
    fn sizes_follow_the_drained_frame() {
        let _lock = lock_for_tests();
        begin_frame(false);
        note_visible("bt-size", [10.0, 20.0]);
        note_visible("bt-size", [30.0, 40.0]);
        assert_eq!(visible_size("bt-size"), None, "not drained yet");
        take_visible();
        assert_eq!(visible_size("bt-size"), Some([30.0, 40.0]));
        begin_frame(false);
        take_visible();
        assert_eq!(visible_size("bt-size"), None, "off screen");
    }

    #[test]
    fn a_live_slot_keeps_one_texture_and_the_newest_frame() {
        let _lock = lock_for_tests();
        let texture = live_texture("bt-live");
        assert_eq!(live_frame("bt-live"), None);
        set_live_frame("bt-live", 1);
        set_live_frame("bt-live", 5);
        assert_eq!(live_frame("bt-live"), Some((texture, 5)));
        assert_eq!(live_texture("bt-live"), texture);
        assert_ne!(live_texture("bt-other"), texture);
        assert_eq!(live_for_paint("bt-live"), Some((texture, true)));
        assert_eq!(forget_live("bt-live"), Some(texture));
        assert_eq!(live_frame("bt-live"), None);
        assert_eq!(forget_live("bt-live"), None);
    }

    #[test]
    fn a_kept_last_frame_stays_until_the_next_still_lands() {
        let _lock = lock_for_tests();
        let texture = live_texture("bt-kept");
        set_live_frame("bt-kept", 2);
        install("bt-kept", 1, b"PNG".to_vec());
        assert!(live_frame("bt-kept").is_some(), "a running stream stays");
        end_live("bt-kept");
        assert_eq!(live_for_paint("bt-kept"), Some((texture, false)));
        install("bt-kept", 2, b"PNG2".to_vec());
        assert_eq!(live_frame("bt-kept"), None, "the new still shows");
        set_live_frame("bt-kept", 3);
        end_live("bt-kept");
        assert_eq!(forget_kept_live(), vec![texture]);
        assert_eq!(live_frame("bt-kept"), None);
        assert!(forget_kept_live().is_empty());
        forget("bt-kept");
    }
}
