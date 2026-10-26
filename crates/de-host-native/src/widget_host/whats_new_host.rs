//! Native arm of the "What's new" window (#251): paint, press and hover
//! through the shared `whats_new_flow`.
//!
//! The window opens at launch, usually over Home, so each entry point runs
//! before the Home and top-most overlay tiers, right under the update
//! modal. While it is shown it owns the pointer.

use crate::backend::NativeFrameBackend;
use de_editor_ui::widgets::{whats_new_flow, PaintCx};
use de_editor_ui::Point2D;

use super::WidgetHostNative;

impl WidgetHostNative {
    pub(in crate::widget_host) fn paint_whats_new(
        &mut self,
        frame: &mut NativeFrameBackend<'_>,
        viewport_width: f32,
        viewport_height: f32,
    ) {
        let mut cx = PaintCx {
            backend: &mut *frame,
        };
        whats_new_flow::paint(&mut cx, &self.editor_state, viewport_width, viewport_height);
    }

    /// `None` when the window is closed; otherwise the press is consumed.
    pub(in crate::widget_host) fn press_whats_new(
        &mut self,
        x: f32,
        y: f32,
        viewport_width: f32,
        viewport_height: f32,
    ) -> Option<bool> {
        let changed = whats_new_flow::press(
            &mut self.editor_state,
            Point2D::new(x, y),
            viewport_width,
            viewport_height,
        )?;
        if changed {
            self.mark_dirty();
        }
        Some(true)
    }

    /// Button hover; `None` when the window is closed.
    pub(in crate::widget_host) fn cursor_move_whats_new(
        &mut self,
        x: f32,
        y: f32,
        viewport_width: f32,
        viewport_height: f32,
    ) -> Option<bool> {
        let changed = whats_new_flow::hover(
            &mut self.editor_state,
            Point2D::new(x, y),
            viewport_width,
            viewport_height,
        )?;
        if changed {
            self.mark_dirty();
        }
        Some(changed)
    }
}

#[cfg(test)]
mod tests {
    use crate::widget_host::WidgetHostNative;
    use de_editor_core::editor_ui_state::WhatsNewState;
    use de_editor_core::release_notes::release_notes;

    fn host_with_window() -> WidgetHostNative {
        let version = release_notes()[0].version.clone();
        let mut host = WidgetHostNative::new();
        host.editor_state.editor_ui.whats_new = Some(WhatsNewState {
            versions: vec![version.clone()],
            page: 0,
            running: version,
            hover: None,
        });
        host
    }

    #[test]
    fn the_window_swallows_presses_behind_it() {
        let mut host = host_with_window();
        assert!(host.apply_press(2.0, 2.0, 1200.0, 800.0));
        assert!(host.editor_state.editor_ui.whats_new.is_some());
    }

    #[test]
    fn escape_closes_the_window_and_records_the_version() {
        let mut host = host_with_window();
        assert!(host.apply_escape());
        assert!(host.editor_state.editor_ui.whats_new.is_none());
        assert!(host.editor_state.editor_ui.last_seen_version.is_some());
    }
}
