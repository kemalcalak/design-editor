//! Native arm over the shared Rename-layers preview (#383).
//!
//! All of the dialog's behaviour lives in `de_editor_core::rename_layers`
//! and `de_editor_ui::widgets::rename_layers_flow`; this file supplies the
//! viewport, the clock and the `mark_dirty` tails, and paints the modal
//! with its scrim.

use super::WidgetHostNative;
use crate::backend::NativeFrameBackend;
use de_editor_ui::widgets::rename_layers_flow as flow;
use de_editor_ui::widgets::{PaintCx, RenameLayersPanel};
use de_editor_ui::{Point2D, Rect, RenderBackend};

impl WidgetHostNative {
    /// Cmd+Shift+R / Edit ▸ Rename layers…: open the preview for the
    /// selection (the whole page when nothing is selected).
    pub fn apply_open_rename_layers(&mut self) -> bool {
        if self.preview.is_some() || self.editor_state.editor_ui.home.visible {
            return false;
        }
        self.commit_pending_input_pub();
        let opened = self.editor_state.open_rename_layers();
        if opened {
            self.mark_dirty();
        }
        opened
    }

    /// Whether the preview is open.
    pub fn rename_layers_open(&self) -> bool {
        self.editor_state.editor_ui.rename_layers.is_some()
    }

    /// Route a press. The dialog is modal: every press is consumed while it
    /// is open.
    pub(in crate::widget_host) fn dispatch_rename_layers_press(
        &mut self,
        x: f32,
        y: f32,
        viewport_width: f32,
        viewport_height: f32,
    ) -> bool {
        if !flow::press(
            &mut self.editor_state,
            viewport_width,
            viewport_height,
            Point2D::new(x, y),
            self.now_ms,
        ) {
            return false;
        }
        self.mark_dirty();
        true
    }

    /// Wheel: scrolls the rows, and is swallowed anywhere else while the
    /// modal is open.
    pub(in crate::widget_host) fn try_scroll_rename_layers(
        &mut self,
        x: f32,
        y: f32,
        delta_y: f32,
        viewport_width: f32,
        viewport_height: f32,
    ) -> bool {
        if !self.rename_layers_open() {
            return false;
        }
        if flow::scroll(
            &mut self.editor_state,
            x,
            y,
            delta_y,
            viewport_width,
            viewport_height,
        ) == Some(true)
        {
            self.mark_dirty();
        }
        true
    }

    /// Hover tint; `None` while the preview is closed.
    pub(in crate::widget_host) fn update_rename_layers_hover(
        &mut self,
        x: f32,
        y: f32,
    ) -> Option<bool> {
        if !self.rename_layers_open() {
            return None;
        }
        let changed = flow::hover(
            &mut self.editor_state,
            self.last_viewport_w,
            self.last_viewport_h,
            Point2D::new(x, y),
        );
        if changed {
            self.mark_dirty();
        }
        Some(changed)
    }

    pub(in crate::widget_host) fn paint_rename_layers(
        &mut self,
        frame: &mut NativeFrameBackend<'_>,
        viewport_width: f32,
        viewport_height: f32,
    ) {
        let Some(panel) = RenameLayersPanel::for_editor(&self.editor_state) else {
            return;
        };
        frame.fill_rect(
            Rect {
                origin: Point2D::new(0.0, 0.0),
                size: Point2D::new(viewport_width, viewport_height),
            },
            de_editor_ui::Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.4,
            },
        );
        let rect = panel.rect(viewport_width, viewport_height);
        let mut cx = PaintCx {
            backend: &mut *frame,
        };
        panel.paint_at(&mut cx, rect, self.now_ms);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use de_editor_core::NodeId;

    fn host() -> WidgetHostNative {
        let doc = serde_json::json!({"version": "1.0.0", "children": [
            {"type": "frame", "id": "cta", "name": "Rectangle 3", "width": 160, "height": 48,
             "fill": [{"type": "solid", "color": "#2563EB"}],
             "children": [{"type": "text", "id": "label", "name": "Text", "content": "Start"}]}
        ]});
        let mut host = WidgetHostNative::new();
        *host.editor_state_mut() = de_editor_core::EditorState::from_document(
            jian_ops_schema::load_str(&doc.to_string())
                .expect("fixture")
                .value,
        );
        host.editor_state_mut()
            .set_single_selection(NodeId::new("cta"));
        host
    }

    #[test]
    fn the_shortcut_opens_a_modal_that_owns_keys_and_presses() {
        let mut host = host();
        assert!(host.apply_open_rename_layers());
        assert!(host.input_active_pub(), "letters must not switch tools");
        // A press on the canvas behind the scrim is the dialog's.
        assert!(host.dispatch_rename_layers_press(5.0, 400.0, 1200.0, 800.0));
        assert!(host.rename_layers_open());
        assert!(host.apply_escape());
        assert!(!host.rename_layers_open());
        assert!(!host.dispatch_rename_layers_press(5.0, 400.0, 1200.0, 800.0));
    }

    #[test]
    fn enter_applies_as_one_undo_step() {
        let mut host = host();
        assert!(host.apply_open_rename_layers());
        assert!(host.apply_send());
        assert!(!host.rename_layers_open());
        let name = |host: &WidgetHostNative| {
            use de_editor_core::PenNodeExt;
            host.editor_state().active_children()[0].base().name.clone()
        };
        assert_eq!(name(&host).as_deref(), Some("Start screen"));
        assert!(host.apply_undo());
        assert_eq!(name(&host).as_deref(), Some("Rectangle 3"));
    }
}
