//! The panel toggle above the tool rail (#86).
//!
//! A square floating button in the chrome's card language. It shows its
//! pressed state while the left panel is open, and slides with the tool
//! rail (its rect comes from `floating_panels::panel_toggle_rect`).

use de_editor_core::editor_ui_state::EditorUiState;
use de_editor_core::ChromeButton;

use crate::theme::Theme;
use crate::widgets::editor_state_ext::theme_for;
use crate::widgets::floating_panels::CARD_RADIUS;
use crate::widgets::icons::{draw_icon, Icon};
use crate::widgets::PaintCx;
use crate::{Point2D, Rect};

const ICON: f32 = 16.0;
/// Inset of the pressed / hover wash inside the button.
const WASH_INSET: f32 = 4.0;

pub struct PanelToggle {
    theme: Theme,
    open: bool,
    hovered: bool,
    pressed: bool,
}

impl PanelToggle {
    pub fn for_editor_ui(ui: &EditorUiState) -> Self {
        Self {
            theme: theme_for(ui),
            open: ui.sidebar_open,
            hovered: ui.chrome_button_hover == Some(ChromeButton::TogglePanel),
            pressed: ui.pressed_button
                == Some(de_editor_core::ButtonPressTarget::Chrome(
                    ChromeButton::TogglePanel,
                )),
        }
    }

    pub fn paint(&self, cx: &mut PaintCx<'_>, rect: Rect) {
        let theme = &self.theme;
        cx.backend.fill_round_rect(rect, CARD_RADIUS, theme.card);
        cx.backend
            .stroke_round_rect(rect, CARD_RADIUS, theme.border, 1.0);
        let wash = Rect::xywh(
            rect.origin.x + WASH_INSET,
            rect.origin.y + WASH_INSET,
            rect.size.x - WASH_INSET * 2.0,
            rect.size.y - WASH_INSET * 2.0,
        );
        // Open = pressed state (the design's "toggle shows its pressed
        // state while the panel is open"); hover / press add the wash.
        if self.open {
            cx.backend.fill_round_rect(wash, 8.0, theme.muted);
        }
        if self.hovered || self.pressed {
            cx.backend.fill_round_rect(wash, 8.0, theme.button_hover);
        }
        let color = if self.open || self.hovered {
            theme.foreground
        } else {
            theme.muted_foreground
        };
        draw_icon(
            cx.backend,
            Icon::PanelLeft,
            Point2D::new(
                rect.origin.x + (rect.size.x - ICON) / 2.0,
                rect.origin.y + (rect.size.y - ICON) / 2.0,
            ),
            ICON,
            color,
            1.5,
        );
    }
}
