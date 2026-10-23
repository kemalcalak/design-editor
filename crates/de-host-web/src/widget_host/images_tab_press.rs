//! Images tab press dispatch (#30). The tab fills the rail's body while
//! on show; the hit-test and the state change are shared
//! (`de_editor_ui::widgets::images_tab`), framing a pressed thumbnail is
//! this host's camera work.

use de_editor_ui::widgets::{host_overlay_geometry, images_tab};
use de_editor_ui::Point2D;

use super::WidgetHost;

impl WidgetHost {
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
    use de_editor_core::LeftPanelTab;
    use de_editor_ui::widgets::host_canvas_geometry;
    use jian_ops_schema::node::PenNode;

    #[test]
    fn search_on_an_empty_page_inserts_and_opens_the_popover() {
        let (vw, vh) = (1200.0, 800.0);
        let mut host = WidgetHost::new();
        host.last_viewport_w = vw;
        host.last_viewport_h = vh;
        host.editor_state.editor_ui.home.hide();
        host.editor_state.editor_ui.sidebar_open = true;
        host.editor_state.editor_ui.slides_panel.tab = LeftPanelTab::Images;
        let body = host_canvas_geometry::rail_body_rect(&host.editor_state, vh);
        let search = images_tab::layout(body, 0, 0.0).search;
        let (x, y) = (
            search.origin.x + search.size.x / 2.0,
            search.origin.y + search.size.y / 2.0,
        );
        assert!(host.apply_press(x, y, vw, vh));
        assert!(matches!(
            host.editor_state.selected_node(),
            Some(PenNode::Image(_))
        ));
        assert!(host.editor_state.editor_ui.image_panel.search_open);
    }
}
