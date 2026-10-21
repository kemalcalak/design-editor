//! Wheel and trackpad scrolling for the Home page column.

use super::WidgetHostNative;
use de_editor_ui::widgets::home_surface::HOME_TOPBAR_H;
use de_editor_ui::widgets::HomeSurface;

impl WidgetHostNative {
    pub(in crate::widget_host) fn try_scroll_home(
        &mut self,
        x: f32,
        y: f32,
        delta_y: f32,
        viewport_width: f32,
        viewport_height: f32,
    ) -> bool {
        if !self.home_visible() {
            return false;
        }
        // The top bar stays pinned; the sidebar does not scroll.
        let Some(home) = HomeSurface::for_editor_at(&self.editor_state, self.now_ms) else {
            return false;
        };
        let layout = home.layout(viewport_width, viewport_height);
        if y < HOME_TOPBAR_H || x < layout.sidebar.size.x {
            return true;
        }
        let max_scroll = home.max_scroll(viewport_width, viewport_height);
        let home = &mut self.editor_state.editor_ui.home;
        let next = (home.scroll_y - delta_y).clamp(0.0, max_scroll);
        if (next - home.scroll_y).abs() > f32::EPSILON {
            home.scroll_y = next;
            self.mark_dirty();
        }
        true
    }
}
