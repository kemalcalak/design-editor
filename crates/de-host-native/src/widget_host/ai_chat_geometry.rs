//! AI chat floating-panel geometry.

use super::helpers::{AICHAT_INSET_BOTTOM, AICHAT_INSET_LEFT};
use super::WidgetHostNative;
use de_editor_core::ChatAnchor;
use de_editor_ui::widgets::{AI_CHAT_MINIMIZED_HEIGHT, AI_CHAT_MIN_HEIGHT, AI_CHAT_MIN_WIDTH};
use de_editor_ui::{Point2D, Rect};

impl WidgetHostNative {
    /// The panel's width in BOTH states — minimizing changes the height
    /// only, so the bar and the panel must read this one function or their
    /// edges drift apart as soon as the user resizes.
    pub(in crate::widget_host) fn ai_chat_panel_width(&self) -> f32 {
        self.editor_state.chat.panel_width.max(AI_CHAT_MIN_WIDTH)
    }

    pub(in crate::widget_host) fn ai_chat_size(&self) -> (f32, f32) {
        let height = if self.editor_state.chat.is_minimized() {
            AI_CHAT_MINIMIZED_HEIGHT
        } else {
            self.editor_state.chat.panel_height.max(AI_CHAT_MIN_HEIGHT)
        };
        (self.ai_chat_panel_width(), height)
    }

    pub(in crate::widget_host) fn ai_chat_rect(
        &self,
        viewport_w: f32,
        viewport_h: f32,
    ) -> Option<Rect> {
        // The chat is PINNED into a left column — the workspace's dock,
        // or the professional editor's Chat tab — whenever one owns it:
        // the panel stops being floating/minimized/draggable and fills
        // the column. A pinned-but-closed column (panel shut, another
        // tab showing, presenting) means no chat at all, never a
        // floating fallback. Both columns resolve through the ONE
        // shared helper so they cannot drift apart again.
        use de_editor_ui::widgets::host_canvas_geometry::{pinned_chat, PinnedChat};
        match pinned_chat(&self.editor_state, viewport_w, viewport_h) {
            Some(PinnedChat::At(rect)) => return Some(rect),
            Some(PinnedChat::Closed) => return None,
            Some(PinnedChat::Composer { x, bottom, width }) => {
                // Composer-only: the card's height follows the draft, the
                // chip rows and whether the input has focus, so the
                // widget resolves it and the card grows UPWARD from the
                // canvas floor.
                let panel = de_editor_ui::widgets::AIChatPlaceholder::from_editor_at(
                    &self.editor_state,
                    self.now_ms,
                );
                let height = panel.composer_only_height(width);
                return Some(Rect {
                    origin: Point2D::new(x, bottom - height),
                    size: Point2D::new(width, height),
                });
            }
            None => {}
        }
        let (cx0, cy0, cw, ch) = self.canvas_region(viewport_w, viewport_h);
        if self.editor_state.chat.is_minimized() {
            return de_editor_ui::widgets::host_canvas_geometry::minimized_chat_bar_rect(
                self.editor_state.chat.anchor,
                self.ai_chat_panel_width(),
                self.editor_state.chat.panel_position,
                cx0,
                cy0,
                cw,
                ch,
            );
        }
        let (panel_w, panel_h) = self.ai_chat_size();
        if self.editor_state.chat.maximized {
            let inset = 12.0;
            if cw <= inset * 2.0 + 16.0 || ch <= inset * 2.0 + 16.0 {
                return None;
            }
            return Some(Rect {
                origin: Point2D::new(cx0 + inset, cy0 + inset),
                size: Point2D::new(cw - inset * 2.0, ch - inset * 2.0),
            });
        }
        if cw <= panel_w + AICHAT_INSET_LEFT + 16.0 || ch <= panel_h + 16.0 {
            return None;
        }
        if let Some(d) = self.chat_drag {
            return Some(Rect {
                origin: Point2D::new(d.pos_x, d.pos_y),
                size: Point2D::new(panel_w, panel_h),
            });
        }
        if let Some((x, y)) = self.editor_state.chat.panel_position {
            return Some(Rect {
                origin: Point2D::new(x, y),
                size: Point2D::new(panel_w, panel_h),
            });
        }
        let (x, y) = match self.editor_state.chat.anchor {
            ChatAnchor::TopLeft => (cx0 + AICHAT_INSET_LEFT, cy0 + AICHAT_INSET_BOTTOM),
            ChatAnchor::TopRight => (
                cx0 + cw - panel_w - AICHAT_INSET_BOTTOM,
                cy0 + AICHAT_INSET_BOTTOM,
            ),
            ChatAnchor::BottomLeft => (
                cx0 + AICHAT_INSET_LEFT,
                cy0 + ch - panel_h - AICHAT_INSET_BOTTOM,
            ),
            ChatAnchor::BottomRight => (
                cx0 + cw - panel_w - AICHAT_INSET_BOTTOM,
                cy0 + ch - panel_h - AICHAT_INSET_BOTTOM,
            ),
        };
        Some(Rect {
            origin: Point2D::new(x, y),
            size: Point2D::new(panel_w, panel_h),
        })
    }
}
