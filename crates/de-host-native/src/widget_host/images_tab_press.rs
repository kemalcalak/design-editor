//! Images tab press dispatch (#30). The tab fills the rail's body while
//! on show; the hit-test and the state change are shared
//! (`de_editor_ui::widgets::images_tab`), framing a pressed thumbnail is
//! this host's camera work.

use de_editor_ui::widgets::{host_overlay_geometry, images_tab};
use de_editor_ui::Point2D;

use super::WidgetHostNative;

impl WidgetHostNative {
    /// Dispatch a press inside the rail's Images tab. Returns `true` when
    /// consumed.
    pub(in crate::widget_host) fn dispatch_images_tab_press(
        &mut self,
        x: f32,
        y: f32,
        viewport_width: f32,
        viewport_height: f32,
    ) -> bool {
        let Some(hit) = images_tab::hit_at(&self.editor_state, viewport_height, Point2D::new(x, y))
        else {
            return false;
        };
        // Every input lets go first, so a popover the press opens keeps
        // the keyboard.
        self.clear_image_input_selection_drag();
        self.blur_text_inputs_on_blank_press();
        if let Some(id) = images_tab::apply_hit(&mut self.editor_state, hit, self.now_ms) {
            self.refresh_layout_scene();
            host_overlay_geometry::zoom_to_fit_node(
                &mut self.editor_state,
                &self.layout_scene,
                id.as_str(),
                viewport_width,
                viewport_height,
            );
        }
        self.mark_dirty();
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use de_editor_core::{LeftPanelTab, NodeId};
    use de_editor_ui::widgets::host_canvas_geometry;
    use jian_ops_schema::node::PenNode;

    const VW: f32 = 1440.0;
    const VH: f32 = 900.0;

    fn host() -> WidgetHostNative {
        let mut host = WidgetHostNative::new();
        let state = host.editor_state_mut();
        state.editor_ui.home.hide();
        state.editor_ui.sidebar_open = true;
        state.editor_ui.slides_panel.tab = LeftPanelTab::Images;
        host
    }

    fn centre(r: de_editor_ui::Rect) -> (f32, f32) {
        (r.origin.x + r.size.x / 2.0, r.origin.y + r.size.y / 2.0)
    }

    fn tab_layout(host: &WidgetHostNative, count: usize) -> images_tab::ImagesTabLayout {
        let body = host_canvas_geometry::rail_body_rect(host.editor_state(), VH);
        images_tab::layout(body, count, 0.0)
    }

    #[test]
    fn generate_on_an_empty_page_inserts_and_opens_the_popover() {
        let mut host = host();
        let (x, y) = centre(tab_layout(&host, 0).generate);
        assert!(host.apply_press(x, y, VW, VH));
        let state = host.editor_state();
        assert!(matches!(state.selected_node(), Some(PenNode::Image(_))));
        assert!(state.editor_ui.image_panel.generate_open);
    }

    #[test]
    fn a_thumbnail_press_selects_and_frames_the_node() {
        let mut host = host();
        let id = host
            .editor_state_mut()
            .insert_image_node_at_viewport("Photo", "")
            .expect("inserted");
        host.editor_state_mut().clear_selection();
        host.mark_editor_state_dirty();
        // Look somewhere else first, so framing has to move the camera.
        host.editor_state_mut().viewport.pan_x += 2000.0;
        let before = host.editor_state().viewport;
        let (x, y) = centre(tab_layout(&host, 1).cells[0]);
        assert!(host.apply_press(x, y, VW, VH));
        assert_eq!(host.editor_state().selection.anchor, id);
        assert_ne!(host.editor_state().viewport, before, "the camera framed it");
    }

    #[test]
    fn the_layers_tab_never_reaches_the_images_press() {
        let mut host = host();
        let (x, y) = centre(tab_layout(&host, 0).generate);
        host.editor_state_mut().editor_ui.slides_panel.tab = LeftPanelTab::Layers;
        host.apply_press(x, y, VW, VH);
        assert!(!host.editor_state().editor_ui.image_panel.generate_open);
        assert_eq!(host.editor_state().selection.anchor, NodeId::default());
    }
}
