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
        report.caches.extend(self.cache_report(backend).caches);
        report
    }

    /// [`Self::memory_report`] without the undo history: no line here
    /// walks the document, so a timer on the UI thread may build it
    /// (Settings ▸ System, #456). The history is estimated off the thread
    /// from [`Self::history_nodes`].
    pub fn cache_report(&self, backend: &NativeBackend) -> MemoryReport {
        let mut report = MemoryReport::default();
        report.push(memory_report::chat_usage(&self.editor_state));
        report.push(memory_report::image_thumbs_usage(&self.editor_state));
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

    /// The undo history's top-level nodes, for a worker to estimate
    /// (`memory_report::history_usage_of`); one `Arc` clone per node.
    pub fn history_nodes(&self) -> Vec<memory_report::HistoryStep> {
        memory_report::history_nodes(&self.editor_state)
    }

    /// Which document the host shows (`document_generation`): a new one
    /// makes the published history estimate stale.
    pub fn document_generation(&self) -> u64 {
        self.editor_state.document_generation()
    }

    /// Whether Settings ▸ System, which shows the memory row, is on screen.
    pub fn settings_system_tab_open(&self) -> bool {
        let ui = &self.editor_state.editor_ui;
        ui.agent_settings_open
            && ui.agent_settings.tab == de_editor_core::agent_settings::AgentSettingsTab::System
    }

    /// When the host should wake for the idle memory trim (#456): once per
    /// quiet spell, [`de_editor_core::memory_idle::IDLE_TRIM_AFTER_MS`] after
    /// the last change. Separate from `next_animation_deadline_ms`, which
    /// means "something animates".
    pub fn memory_idle_deadline_ms(&self) -> Option<u64> {
        self.memory_idle.deadline_ms()
    }

    /// Hold the idle memory trim off: the host calls this on every wake
    /// while an agent works, since its writes are not input (#456).
    pub fn note_memory_activity(&mut self) {
        self.memory_idle.note_activity(self.now_ms);
    }

    /// Run the idle memory trim when it is due: the undo history is cut to
    /// its byte budget. `Some` when it ran; the report carries the dropped
    /// steps for the host to free off the UI thread, before it asks the
    /// allocator to give freed pages back.
    pub fn run_idle_memory_trim(&mut self) -> Option<de_editor_core::memory_idle::IdleTrimReport> {
        self.memory_idle
            .run_if_due(&mut self.editor_state, self.now_ms)
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
