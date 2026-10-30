//! Public viewport-fit adapter for desktop runner startup.

use super::WidgetHostNative;

impl WidgetHostNative {
    /// Rebuild the newly-active page scene before deriving its fit. Page
    /// navigation must use this helper instead of calling `zoom_to_fit`
    /// immediately after mutating `editor_state`, otherwise the lazy scene
    /// gate can still expose the previously painted page's bounds.
    pub(in crate::widget_host) fn fit_active_page_after_switch(
        &mut self,
        viewport_w: f32,
        viewport_h: f32,
    ) {
        self.mark_dirty();
        self.zoom_to_fit(viewport_w, viewport_h);
    }

    pub fn fit_content_to_viewport(&mut self, viewport_w: f32, viewport_h: f32) {
        self.zoom_to_fit(viewport_w, viewport_h);
    }

    /// Bring `node_id` on the active page into view when it is off screen,
    /// keeping the zoom where the node fits (#264). Returns whether the
    /// camera moved.
    pub fn reveal_node(&mut self, node_id: &str, viewport_w: f32, viewport_h: f32) -> bool {
        self.refresh_layout_scene();
        let moved = de_editor_ui::widgets::host_overlay_geometry::reveal_node(
            &mut self.editor_state,
            &self.layout_scene,
            node_id,
            viewport_w,
            viewport_h,
        );
        if moved {
            self.mark_dirty();
        }
        moved
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use de_editor_core::{EditorCommand, NodeId, PenNodeExt};

    const VW: f32 = 1440.0;
    const VH: f32 = 900.0;

    fn host_with_board_at(y: i32) -> (WidgetHostNative, String) {
        let mut host = WidgetHostNative::new();
        let state = host.editor_state_mut();
        state.editor_ui.home.hide();
        state.active_children_mut().clear();
        assert!(state.apply(EditorCommand::InsertNode {
            kind: "frame".into(),
            name: "Board".into(),
            x: 0,
            y,
            width: 400,
            height: 300,
            fill_hex: None,
            target_parent: NodeId::NONE,
            page_id: None,
        }));
        let id = state.active_children()[0].id_str().to_string();
        host.mark_editor_state_dirty();
        (host, id)
    }

    #[test]
    fn an_off_screen_board_comes_into_view_at_the_same_zoom() {
        // #264: the agent's board at y = 1000 is off screen at pan 406,94.
        let (mut host, id) = host_with_board_at(1000);
        let state = host.editor_state_mut();
        state.viewport.zoom = 1.0;
        state.viewport.pan_x = 406.0;
        state.viewport.pan_y = 94.0;
        assert!(host.reveal_node(&id, VW, VH), "the camera moved");
        assert_eq!(host.editor_state().viewport.zoom, 1.0, "zoom kept");
        assert!(!host.reveal_node(&id, VW, VH), "already in view");
    }
}
