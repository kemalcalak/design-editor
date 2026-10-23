//! Shared geometry helpers for the web widget host.

use super::WidgetHost;
use de_editor_ui::widgets::host_canvas_geometry as canvas_geometry;
use de_editor_ui::widgets::host_overlay_geometry as overlay_geometry;
use de_editor_ui::widgets::Toolbar;
use de_editor_ui::{Point2D, Rect};

impl WidgetHost {
    /// Canvas region (logical px, viewport-relative). The math is
    /// single-sourced with the native host — see the coordinate invariant
    /// in `de_editor_ui::widgets::host_canvas_geometry`.
    pub(in crate::widget_host) fn canvas_region(
        &self,
        viewport_w: f32,
        viewport_h: f32,
    ) -> (f32, f32, f32, f32) {
        canvas_geometry::canvas_region(&self.editor_state, viewport_w, viewport_h)
    }

    pub(in crate::widget_host) fn over_canvas(
        &self,
        x: f32,
        y: f32,
        viewport_w: f32,
        viewport_h: f32,
    ) -> bool {
        canvas_geometry::over_canvas(&self.editor_state, x, y, viewport_w, viewport_h)
    }

    /// `(anchor, viewport)` for the import dropdown — mirrors
    /// the native host so both chromes clamp the popup identically.
    /// `(anchor, viewport)` for the import dropdown — shared with
    /// the native host so both chromes clamp the popup identically.
    pub(in crate::widget_host) fn import_menu_anchor(
        &self,
        viewport_w: f32,
        viewport_h: f32,
    ) -> (Rect, Rect) {
        overlay_geometry::import_menu_anchor(&self.editor_state, viewport_w, viewport_h)
    }

    /// Close the import dropdown and clear its hover row.
    /// Close the import dropdown and clear its hover row.
    pub(in crate::widget_host) fn close_import_menu(&mut self) {
        overlay_geometry::close_import_menu(&mut self.editor_state);
    }

    pub(in crate::widget_host) fn locale_picker_rect(&self, viewport_w: f32) -> Rect {
        overlay_geometry::locale_picker_rect(&self.editor_state, viewport_w, self.last_viewport_h)
    }

    /// The rail rect the LAYERS tree gets — the whole rail, less the
    /// slides tab row when this document shows one. Every layer-tree
    /// caller goes through here, so the tab row can never shift the
    /// painted rows out from under their own click targets.
    /// Whether the Layers tree owns the rail's body — the rail is open,
    /// the workspace dock is not up, and the Layers tab is the one on
    /// show. Every layer-row interaction gates on this so an unseen tree
    /// never takes input under another tab (mirrors the native
    /// `layers_panel_visible`).
    pub(in crate::widget_host) fn layers_panel_visible(&self) -> bool {
        self.editor_state.editor_ui.sidebar_open
            && !canvas_geometry::workspace_docked(&self.editor_state)
            && de_editor_ui::widgets::slides_panel_flow::layers_tab_active(&self.editor_state)
    }

    pub(in crate::widget_host) fn layer_panel_rect(&self, viewport_h: f32) -> Rect {
        self.layers_content_rect(viewport_h)
    }

    pub(in crate::widget_host) fn toolbar_rect(
        &mut self,
        viewport_w: f32,
        viewport_h: f32,
    ) -> Rect {
        self.refresh_layout_scene();
        canvas_geometry::toolbar_rect_for(&self.editor_state, viewport_w, viewport_h)
    }

    /// Per-button hover wash on the floating toolbar. Mirrors
    /// `de_host_native::widget_host::geometry::update_toolbar_hover`.
    /// Returns `true` if the hover state changed.
    pub(in crate::widget_host) fn update_toolbar_hover(&mut self, x: f32, y: f32) -> bool {
        let rect = self.toolbar_rect(self.last_viewport_w, self.last_viewport_h);
        let toolbar = Toolbar::for_editor(&self.editor_state);
        let new_hover = toolbar
            .hit_test(rect, Point2D::new(x, y))
            .map(de_editor_ui::widgets::editor_state_ext::toolbar_hover);
        if new_hover != self.editor_state.editor_ui.toolbar_hover {
            self.editor_state.editor_ui.toolbar_hover = new_hover;
            self.mark_dirty();
            true
        } else {
            false
        }
    }

    /// Anchor / bezier-handle hit-test for the selected Path node.
    /// The math is shared with the native host — see
    /// `de_editor_ui::widgets::host_canvas_geometry::path_anchor_hit`.
    pub(in crate::widget_host) fn path_anchor_hit(
        &self,
        x: f32,
        y: f32,
        viewport_w: f32,
        viewport_h: f32,
    ) -> Option<(String, usize, super::AnchorDragTarget)> {
        canvas_geometry::path_anchor_hit(
            &self.editor_state,
            &self.layout_scene,
            x,
            y,
            viewport_w,
            viewport_h,
        )
    }
}
