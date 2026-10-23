//! Native arm of the in-app update modal (#111): paint, press and hover.
//!
//! The modal sits above every other surface — Home included, because the
//! launch-time update offer can land while Home is up — so each entry point
//! runs before the Home and top-most overlay tiers. While it is shown it owns
//! the pointer: presses outside the card are swallowed, not passed through.

use crate::backend::NativeFrameBackend;
use de_editor_ui::widgets::update_modal::{UpdateModal, UpdateModalHit};
use de_editor_ui::widgets::{PaintCx, Widget};
use de_editor_ui::{Point2D, Rect, RenderBackend};

use super::WidgetHostNative;

impl WidgetHostNative {
    pub(in crate::widget_host) fn paint_update_modal(
        &mut self,
        frame: &mut NativeFrameBackend<'_>,
        viewport_width: f32,
        viewport_height: f32,
    ) {
        let Some(modal) = UpdateModal::for_editor(&self.editor_state) else {
            return;
        };
        frame.fill_rect(
            Rect::xywh(0.0, 0.0, viewport_width, viewport_height),
            de_editor_ui::Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.5,
            },
        );
        let rect = modal.rect(viewport_width, viewport_height);
        let mut cx = PaintCx {
            backend: &mut *frame,
        };
        modal.paint(&mut cx, rect);
    }

    /// `None` when no modal is shown; otherwise the press is consumed.
    pub(in crate::widget_host) fn press_update_modal(
        &mut self,
        x: f32,
        y: f32,
        viewport_width: f32,
        viewport_height: f32,
    ) -> Option<bool> {
        let hit = {
            let modal = UpdateModal::for_editor(&self.editor_state)?;
            modal.hit_test(
                modal.rect(viewport_width, viewport_height),
                Point2D::new(x, y),
            )
        };
        if let UpdateModalHit::Button(button) = hit {
            self.editor_state.editor_ui.press_update_button(button);
            self.mark_dirty();
        }
        Some(true)
    }

    /// Button hover; `None` when no modal is shown.
    pub(in crate::widget_host) fn cursor_move_update_modal(
        &mut self,
        x: f32,
        y: f32,
        viewport_width: f32,
        viewport_height: f32,
    ) -> Option<bool> {
        let hover = {
            let modal = UpdateModal::for_editor(&self.editor_state)?;
            match modal.hit_test(
                modal.rect(viewport_width, viewport_height),
                Point2D::new(x, y),
            ) {
                UpdateModalHit::Button(button) => Some(button),
                _ => None,
            }
        };
        let state = self.editor_state.editor_ui.update_modal.as_mut()?;
        if state.hover != hover {
            state.hover = hover;
            self.mark_dirty();
            return Some(true);
        }
        Some(false)
    }
}

#[cfg(test)]
mod tests {
    use crate::widget_host::WidgetHostNative;
    use de_editor_core::editor_ui_state::{UpdateAction, UpdateModalState, UpdatePhase};
    use de_editor_ui::widgets::update_modal::UpdateModal;

    #[test]
    fn modal_press_routes_buttons_and_swallows_the_scrim() {
        let mut host = WidgetHostNative::new();
        host.editor_state.editor_ui.update_modal =
            Some(UpdateModalState::new("1.0.0", UpdatePhase::Offer));
        assert!(host.apply_press(2.0, 2.0, 1200.0, 800.0));
        assert!(host.editor_state.editor_ui.update_modal.is_some());

        let (_, update) = {
            let modal = UpdateModal::for_editor(&host.editor_state).unwrap();
            modal.button_rects(modal.rect(1200.0, 800.0))[1]
        };
        host.apply_press(update.origin.x + 4.0, update.origin.y + 4.0, 1200.0, 800.0);
        assert_eq!(
            host.editor_state.editor_ui.pending_update_action,
            Some(UpdateAction::Start)
        );
    }

    #[test]
    fn escape_cancels_a_running_download() {
        let mut host = WidgetHostNative::new();
        host.editor_state.editor_ui.update_modal = Some(UpdateModalState::new(
            "1.0.0",
            UpdatePhase::Downloading {
                received: 1,
                total: Some(10),
                bytes_per_sec: 1.0,
            },
        ));
        assert!(host.apply_escape());
        assert!(host.editor_state.editor_ui.update_modal.is_none());
        assert_eq!(
            host.editor_state.editor_ui.pending_update_action,
            Some(UpdateAction::Cancel)
        );
    }
}
