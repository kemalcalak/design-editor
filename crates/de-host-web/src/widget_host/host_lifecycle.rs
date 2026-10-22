//! Construction + per-frame bookkeeping on the web `WidgetHost`: the
//! constructor, pending-auth drain, cache owner rotation, modifier
//! setters, theme sync, and the layout-scene refresh / transition starts.
//!
//! Split out of the `widget_host.rs` spine to keep it under the repo's
//! 800-line cap.

use super::*;

impl WidgetHost {
    pub fn new() -> Self {
        // A fresh launch opens with a single empty starter Frame —
        // see `EditorState::starter`.
        let mut editor_state = de_editor_core::EditorState::starter();
        editor_state.editor_ui.slide_thumbnails_supported = true;
        // Seed the render scene once up front; subsequent frames
        // re-derive only when `editor_state_dirty` is set.
        let layout_scene = de_pen_loader::editor_state_to_active_page_layout_scene(&editor_state);
        let last_chat_session_index = editor_state.chat.active_index();
        Self {
            editor_state,
            layout_scene,
            layout_transition: None,
            scene_cache: de_pen_loader::SceneBuildCache::new(),
            editor_state_dirty: false,
            #[cfg(feature = "canvaskit")]
            doc_sync_dirty: false,
            document_epoch: 0,
            document_import_generation: 0,
            theme: Theme::dark(),
            drag: None,
            space_pan: false,
            chat_drag: None,
            image_adjustment_drag: None,
            image_crop_drag: None,
            effect_radius_drag: None,
            code_selection_drag: None,
            chat_input_selection_drag: None,
            image_input_selection_drag: None,
            image_input_geometry: None,
            chat_text_selection_drag: None,
            create_drag: None,
            path_anchor_drag: None,
            marquee_drag: None,
            layer_drag: None,
            design_md_drag: None,
            component_browser_drag: None,
            icon_picker_drag: None,
            variables_resize: None,
            handle_drag: None,
            node_drag: None,
            option_drag_source_ids: Vec::new(),
            next_node_id: 100,
            shift_held: false,
            alt_held: false,
            now_ms: 0,
            wall_now_secs: 0,
            toast_rect: None,
            last_viewport_w: 0.0,
            last_viewport_h: 0.0,
            last_cursor_x: 0.0,
            last_cursor_y: 0.0,
            chat_panel_owner: de_editor_ui::widgets::AIChatPlaceholder::next_owner(),
            layer_panel_owner: de_editor_ui::widgets::LayerPanel::next_layer_panel_owner(),
            last_chat_session_index,
            #[cfg(feature = "canvaskit")]
            preview: None,
            preview_device_frame: None,
            preview_scroll_y: 0.0,
            preview_manual_pick: None,
            preview_surface_capture: None,
            preview_pressed_pids: Vec::new(),
            preview_last_doc_by_pid: std::collections::HashMap::new(),
            preview_edge_swipe_start_x: None,
            preview_edge_swipe_pid: None,
            preview_frame_viewport: None,
            #[cfg(feature = "canvaskit")]
            op_ck: None,
            #[cfg(feature = "canvaskit")]
            preview_mode_transition: None,
            slideshow_cursor: None,
            slideshow_press_screen: None,
            bundled_fonts_pending: false,
        }
    }

    /// Rotate the chat-panel transcript-cache owner when the active chat session
    /// (tab) changed since the last call (mirrors native). The new tab's build is
    /// then stamped with a fresh owner so it never cross-pairs with the previous
    /// tab's cached geometry. Called at the top of the paint / cursor-move entry.
    pub(in crate::widget_host) fn rotate_chat_owner_if_session_changed(&mut self) {
        bookkeeping::rotate_chat_owner_if_session_changed(
            &self.editor_state,
            &mut self.chat_panel_owner,
            &mut self.last_chat_session_index,
        );
    }

    /// Force a chat-panel transcript-cache owner rotation NOW, unconditionally —
    /// even when `chat.active_index()` is unchanged (mirrors native). Called
    /// synchronously at every host session-mutation site (tab switch / new tab /
    /// close tab) so a pointer move arriving before the next paint cannot pair the
    /// previous session's cached geometry with the new session's messages, and so
    /// same-index session replacement (close active tab 0 → next session at index
    /// 0; close the sole tab → replaced in place) — which the index-only poll in
    /// [`Self::rotate_chat_owner_if_session_changed`] misses — is still covered.
    /// `pub(crate)` because the tab-close path runs in `crate::web_chat`, outside
    /// the `widget_host` module.
    pub(crate) fn force_rotate_chat_owner(&mut self) {
        bookkeeping::force_rotate_chat_owner(
            &self.editor_state,
            &mut self.chat_panel_owner,
            &mut self.last_chat_session_index,
        );
    }

    /// Force a LayerPanel row-model-cache owner rotation NOW (mirrors native
    /// `WidgetHostNative::force_rotate_layer_panel_owner`). A freshly loaded /
    /// imported / live-synced document restarts its revision at 0 and its active
    /// page index at 0, so a WHOLE-DOCUMENT replacement leaves the cache key
    /// `(document_revision, active_page_index, collapsed_fp, rename_fp)`
    /// byte-identical to the previous document's while the owner never rotates
    /// on its own — the paint path would keep serving the previous document's
    /// cached rows. Rotating at every replacement seam forces the next owned
    /// paint resolve to rebuild. `pub(crate)` because the Open / New seams run in
    /// `crate::dom_io`, outside the `widget_host` module.
    pub(crate) fn force_rotate_layer_panel_owner(&mut self) {
        self.layer_panel_owner = de_editor_ui::widgets::LayerPanel::next_layer_panel_owner();
    }

    /// Build the Layer panel through this host's owner-scoped row cache so
    /// event-time hit tests reuse the same active-page rows as paint.
    pub(in crate::widget_host) fn layer_panel(&self) -> de_editor_ui::widgets::LayerPanel {
        de_editor_ui::widgets::LayerPanel::from_editor_owned(
            &self.editor_state,
            self.layer_panel_owner,
        )
    }

    /// Forward the latest shift-key state from the DOM listener
    /// so apply_press can branch on shift+click. Web reads
    /// `MouseEvent.shiftKey` / `KeyboardEvent.shiftKey` per event
    /// and calls this just before dispatch.
    pub fn set_modifier_shift(&mut self, held: bool) {
        self.shift_held = held;
    }

    pub fn set_modifier_alt(&mut self, held: bool) {
        self.alt_held = held;
    }

    pub fn set_space_pan(&mut self, held: bool) {
        self.space_pan = held;
    }

    /// Refresh host-level theme tokens from the canonical editor UI
    /// state. Most widgets derive their theme directly, but a few
    /// paint-layer affordances still read this host cache.
    pub(in crate::widget_host) fn sync_theme_from_editor(&mut self) {
        self.theme =
            de_editor_ui::widgets::editor_state_ext::theme_for(&self.editor_state.editor_ui);
    }

    /// Rebuild the layout-resolved `LayoutScene` from `editor_state`
    /// if `editor_state_dirty` is set; clear the flag. Cheap no-op
    /// when the scene is already current. The input hit-test + the
    /// paint pass both call this before reading `layout_scene`.
    pub(in crate::widget_host) fn refresh_layout_scene(&mut self) {
        if self.editor_state_dirty {
            // Only re-derive when the scene inputs (doc / theme / active page)
            // actually changed — most `editor_state_dirty` marks (hover, scroll,
            // selection, caret drafts, per-frame chat / codegen streaming) leave
            // them identical, and the scene carries no editor state.
            if let Some(scene) = self.scene_cache.maybe_rebuild(&self.editor_state) {
                self.layout_scene = scene;
            }
            self.editor_state_dirty = false;
        }
    }

    pub(in crate::widget_host) fn start_layout_transition_from_scene(
        &mut self,
        before: de_editor_ui::layout_scene::LayoutScene,
    ) {
        self.refresh_layout_scene();
        self.layout_transition =
            bookkeeping::transition_between(&before, &self.layout_scene, self.now_ms);
    }

    pub(in crate::widget_host) fn start_layout_transition_from_scene_excluding(
        &mut self,
        before: de_editor_ui::layout_scene::LayoutScene,
        excluded_id: &de_editor_core::NodeId,
    ) {
        self.refresh_layout_scene();
        self.layout_transition = bookkeeping::transition_between_excluding(
            &before,
            &self.layout_scene,
            self.now_ms,
            excluded_id,
        );
    }

    pub(in crate::widget_host) fn start_layout_transition_from_bounds(
        &mut self,
        node_id: &de_editor_core::NodeId,
        bounds: Rect,
    ) {
        self.layout_transition =
            bookkeeping::transition_from_single_bounds(node_id, bounds, self.now_ms);
    }

    /// Mark `editor_state` as mutated so the next `refresh_layout_scene()`
    /// re-derives the render scene. Call after any direct mutation of
    /// `self.editor_state`.
    pub(in crate::widget_host) fn mark_dirty(&mut self) {
        self.editor_state_dirty = true;
        #[cfg(feature = "canvaskit")]
        {
            self.doc_sync_dirty = true;
        }
    }

    /// Set the CanvasKit bridge object for preview text measurement.
    /// Called by the mount path after the backend is initialized.
    #[cfg(feature = "canvaskit")]
    pub(crate) fn set_op_ck(&mut self, op_ck: &crate::canvaskit::OpCk) {
        self.op_ck = Some(op_ck.clone());
    }
}
