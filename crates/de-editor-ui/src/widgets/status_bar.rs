//! `StatusBar` — the floating zoom pill pinned to the canvas's
//! bottom-right corner (#30): `−  38%  +`.
//!
//! The `[-]` / `[+]` buttons step the zoom; the percentage itself frames
//! the page content (zoom to fit — the old magnifier's job, folded into
//! the label so the pill matches the design). [`StatusBar::control_at`]
//! resolves a pointer to a control for both the click dispatch and the
//! per-control `theme.button_hover` hover wash.

use crate::theme::Theme;
use crate::widgets::icons::{draw_icon, Icon};
use crate::widgets::text_metrics::{measure_chrome_weighted, CHROME_FONT_FAMILY};
use crate::widgets::{LayoutBox, LayoutCx, PaintCx, Widget, WidgetId};
use crate::{Color, Point2D, Rect, TextLayout};
use de_editor_core::StatusBarButton;

pub const STATUS_BAR_WIDTH: f32 = 107.0;
pub const STATUS_BAR_HEIGHT: f32 = 34.0;
const RADIUS: f32 = 10.0;
const ICON_SIZE: f32 = 14.0;
/// Square side of the minus / plus buttons.
const BUTTON: f32 = 26.0;
/// Inset of the minus / plus buttons from the pill's edges.
const INSET: f32 = 4.0;
const LABEL_FONT: f32 = 13.0;

pub struct StatusBar {
    pub id: WidgetId,
    pub zoom_percent: u32,
    pub theme: Theme,
    /// Which control the cursor is over — drives the per-control
    /// `theme.button_hover` wash. `None` = no hover.
    pub hover: Option<StatusBarButton>,
    pub pressed: Option<StatusBarButton>,
}

impl StatusBar {
    pub fn new() -> Self {
        Self {
            id: WidgetId::new(6000),
            zoom_percent: 100,
            theme: Theme::dark(),
            hover: None,
            pressed: None,
        }
    }

    pub fn with_zoom(zoom_percent: u32) -> Self {
        Self {
            zoom_percent,
            ..Self::new()
        }
    }

    /// Build the pill with theme + zoom from the editor state.
    pub fn for_editor(state: &de_editor_core::EditorState) -> Self {
        let zoom = (state.viewport.zoom * 100.0).round() as u32;
        Self {
            id: WidgetId::new(6000),
            zoom_percent: zoom.max(1),
            theme: crate::widgets::editor_state_ext::theme_for(&state.editor_ui),
            hover: state.editor_ui.statusbar_hover,
            pressed: match state.editor_ui.pressed_button {
                Some(de_editor_core::ButtonPressTarget::StatusBar(button)) => Some(button),
                _ => None,
            },
        }
    }

    /// The three control rects for a pill at `rect`: minus, the
    /// percentage (zoom to fit), plus. Shared by paint + [`control_at`]
    /// so the hover wash and the click target can never drift.
    fn control_rects(&self, rect: Rect) -> [(StatusBarButton, Rect); 3] {
        let y = rect.origin.y + (rect.size.y - BUTTON) / 2.0;
        let minus = Rect::xywh(rect.origin.x + INSET, y, BUTTON, BUTTON);
        let plus = Rect::xywh(
            rect.origin.x + rect.size.x - INSET - BUTTON,
            y,
            BUTTON,
            BUTTON,
        );
        let label_x = minus.origin.x + BUTTON;
        let label = Rect::xywh(label_x, y, plus.origin.x - label_x, BUTTON);
        [
            (StatusBarButton::ZoomOut, minus),
            (StatusBarButton::ZoomToFit, label),
            (StatusBarButton::ZoomIn, plus),
        ]
    }

    /// Resolve a pointer at `point` to a control.
    pub fn control_at(&self, rect: Rect, point: Point2D) -> Option<StatusBarButton> {
        if !rect.contains(point) {
            return None;
        }
        self.control_rects(rect)
            .into_iter()
            .find_map(|(control, r)| r.contains(point).then_some(control))
    }
}

impl Default for StatusBar {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for StatusBar {
    fn id(&self) -> WidgetId {
        self.id
    }

    fn layout(&self, _cx: &LayoutCx) -> LayoutBox {
        LayoutBox {
            rect: Rect {
                origin: Point2D::new(0.0, 0.0),
                size: Point2D::new(STATUS_BAR_WIDTH, STATUS_BAR_HEIGHT),
            },
        }
    }

    fn paint(&self, cx: &mut PaintCx<'_>, rect: Rect) {
        cx.backend.fill_round_rect(rect, RADIUS, self.theme.card);
        cx.backend
            .stroke_round_rect(rect, RADIUS, self.theme.border, 1.0);
        for (control, r) in self.control_rects(rect) {
            let color = paint_control_bg(cx, &self.theme, r, control, self.hover, self.pressed);
            match control {
                StatusBarButton::ZoomOut | StatusBarButton::ZoomIn => {
                    let icon = if control == StatusBarButton::ZoomOut {
                        Icon::Minus
                    } else {
                        Icon::Plus
                    };
                    draw_icon(
                        cx.backend,
                        icon,
                        Point2D::new(
                            r.origin.x + (BUTTON - ICON_SIZE) / 2.0,
                            r.origin.y + (BUTTON - ICON_SIZE) / 2.0,
                        ),
                        ICON_SIZE,
                        color,
                        1.5,
                    );
                }
                StatusBarButton::ZoomToFit => {
                    let text = format!("{}%", self.zoom_percent);
                    let w = measure_chrome_weighted(cx.backend, &text, LABEL_FONT, 600);
                    let label = TextLayout::single_run(
                        &text,
                        CHROME_FONT_FAMILY,
                        LABEL_FONT,
                        self.theme.foreground.to_jian(),
                        Point2D::ZERO,
                    )
                    .with_font_weight(600);
                    cx.backend.draw_text(
                        &label,
                        Point2D::new(
                            r.origin.x + (r.size.x - w) / 2.0,
                            jian_widgets::centered_text_baseline_y(r, LABEL_FONT),
                        ),
                    );
                }
            }
        }
    }

    fn access_node(&self) -> accesskit::Node {
        let mut node = accesskit::Node::new(accesskit::Role::Group);
        node.set_label("Zoom controls");
        node
    }
}

/// Paint the `theme.button_hover` wash behind a control when the cursor
/// rests on it, returning the glyph color: foreground while hovered,
/// muted otherwise.
fn paint_control_bg(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    rect: Rect,
    control: StatusBarButton,
    hover: Option<StatusBarButton>,
    pressed: Option<StatusBarButton>,
) -> Color {
    crate::widgets::button::paint_ghost_button_feedback(
        cx.backend,
        theme,
        rect,
        hover == Some(control),
        pressed == Some(control),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_zoom_is_100() {
        let bar = StatusBar::new();
        assert_eq!(bar.zoom_percent, 100);
    }

    #[test]
    fn with_zoom_overrides() {
        let bar = StatusBar::with_zoom(150);
        assert_eq!(bar.zoom_percent, 150);
    }

    #[test]
    fn control_at_resolves_minus_percentage_plus() {
        let bar = StatusBar::with_zoom(100);
        let rect = Rect::xywh(0.0, 0.0, STATUS_BAR_WIDTH, STATUS_BAR_HEIGHT);
        let cy = STATUS_BAR_HEIGHT / 2.0;
        assert_eq!(
            bar.control_at(rect, Point2D::new(INSET + BUTTON / 2.0, cy)),
            Some(StatusBarButton::ZoomOut)
        );
        // The percentage frames the content (zoom to fit).
        assert_eq!(
            bar.control_at(rect, Point2D::new(STATUS_BAR_WIDTH / 2.0, cy)),
            Some(StatusBarButton::ZoomToFit)
        );
        assert_eq!(
            bar.control_at(
                rect,
                Point2D::new(STATUS_BAR_WIDTH - INSET - BUTTON / 2.0, cy)
            ),
            Some(StatusBarButton::ZoomIn)
        );
        assert_eq!(
            bar.control_at(rect, Point2D::new(1.0, cy)),
            None,
            "edge padding"
        );
    }

    #[test]
    fn for_editor_picks_up_hover() {
        let mut state = de_editor_core::EditorState::new();
        state.editor_ui.statusbar_hover = Some(StatusBarButton::ZoomIn);
        let bar = StatusBar::for_editor(&state);
        assert_eq!(bar.hover, Some(StatusBarButton::ZoomIn));
    }

    #[test]
    fn for_editor_picks_up_pressed_button() {
        let mut state = de_editor_core::EditorState::new();
        state.editor_ui.pressed_button = Some(de_editor_core::ButtonPressTarget::StatusBar(
            StatusBarButton::ZoomIn,
        ));
        let bar = StatusBar::for_editor(&state);
        assert_eq!(bar.pressed, Some(StatusBarButton::ZoomIn));
    }

    #[test]
    fn layout_reports_pill_size() {
        let cx = LayoutCx {
            available_width: 9999.0,
            dpi: 1.0,
        };
        let lb = StatusBar::new().layout(&cx);
        assert_eq!(lb.rect.size.x, STATUS_BAR_WIDTH);
        assert_eq!(lb.rect.size.y, STATUS_BAR_HEIGHT);
    }
}
