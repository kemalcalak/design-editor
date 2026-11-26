//! The native host's memory report (#456): every cache the editor keeps
//! across frames, from the document's undo history down to the backend's
//! decoded rasters, as one `MemoryReport`.
//!
//! Built on demand (Settings ▸ System, the debug tool, benchmarks); it
//! walks the undo history and the scene once, so it is never called per
//! frame.

use super::*;
use crate::NativeBackend;
use de_editor_core::memory_report::{self, CacheUsage, MemoryReport};

impl WidgetHostNative {
    /// What this host and `backend` hold right now.
    pub fn memory_report(&self, backend: &NativeBackend) -> MemoryReport {
        let mut report = MemoryReport::default();
        report.push(memory_report::history_usage(&self.editor_state));
        report.push(memory_report::chat_usage(&self.editor_state));
        report.push(self.layout_scene_usage());
        report.push(self.pan_cache_usage());
        let (thumbs, thumb_bytes) = self.slide_thumbs.usage();
        report.push(CacheUsage::exact(
            "slide_thumbnails",
            thumb_bytes as u64,
            thumbs as u64,
            None,
        ));
        report.push(de_editor_ui::widgets::canvas_text_layout_memo::memory_usage());
        report.push(de_editor_ui::widgets::canvas_viewport_image::data_url_cache_usage());
        report.push(de_editor_ui::widgets::browser_textures::memory_usage());
        backend.report_memory(&mut report);
        report
    }

    /// The canvas's layout-resolved scene (the active page's render tree),
    /// estimated from its node count.
    fn layout_scene_usage(&self) -> CacheUsage {
        fn count(nodes: &[de_editor_ui::layout_scene::SceneNode]) -> u64 {
            nodes.iter().map(|node| 1 + count(&node.children)).sum()
        }
        let nodes: u64 = self
            .layout_scene
            .pages
            .iter()
            .map(|page| count(&page.children))
            .sum();
        let bytes = nodes * std::mem::size_of::<de_editor_ui::layout_scene::SceneNode>() as u64;
        CacheUsage::estimate("layout_scene", bytes, nodes, None)
    }

    /// The pan cache's offscreen canvas layer.
    fn pan_cache_usage(&self) -> CacheUsage {
        let (entries, bytes) = match &self.pan_cache {
            Some(cache) => (
                1,
                (cache.surface.width().max(0) as u64) * (cache.surface.height().max(0) as u64) * 4,
            ),
            None => (0, 0),
        };
        CacheUsage::exact("pan_cache", bytes, entries, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_names_every_cache_once() {
        let host = WidgetHostNative::new();
        let backend = NativeBackend::with_dpi(1.0);
        let report = host.memory_report(&backend);
        let mut ids: Vec<&str> = report.caches.iter().map(|c| c.id).collect();
        let total = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), total, "ids are unique: {ids:?}");
        for id in [
            "undo_history",
            "layout_scene",
            "pan_cache",
            "image_rasters",
            "image_encoded_bytes",
            "svg_rasters",
        ] {
            assert!(ids.contains(&id), "{id} is reported");
        }
    }

    #[test]
    fn layout_scene_line_follows_the_painted_page() {
        let mut host = WidgetHostNative::new();
        let empty = host.layout_scene_usage();
        let doc = jian_ops_schema::load_str(
            r#"{"version":"1.0","children":[{"type":"frame","id":"f","width":100,"height":100,"children":[{"type":"rectangle","id":"r","width":10,"height":10}]}]}"#,
        )
        .expect("document")
        .value;
        *host.editor_state_mut() = de_editor_core::EditorState::from_document(doc);
        host.mark_paint_dirty_for_test();
        host.refresh_layout_scene();
        let built = host.layout_scene_usage();
        assert!(built.entries > empty.entries, "{empty:?} → {built:?}");
        assert!(built.bytes > empty.bytes);
    }
}
