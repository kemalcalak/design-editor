//! `CanvasActions` — the floating action group at the canvas top-right
//! (#30): `</>` Code view · Preview · Download · Export.
//!
//! Like the file pill, layout runs on a width estimate (the web host
//! hit-tests without a text backend) and paint centres the real glyphs in
//! the rects that estimate produced.

use crate::theme::Theme;
use crate::widgets::chrome_hit::ChromeHit;
use crate::widgets::editor_state_ext::{theme_for, translate};
use crate::widgets::home_surface::estimate_width;
use crate::widgets::icons::{draw_icon, Icon};
use crate::widgets::text_metrics::{measure_chrome_weighted, CHROME_FONT_FAMILY};
use crate::widgets::{LayoutBox, LayoutCx, PaintCx, Widget, WidgetId};
use crate::{Color, Point2D, Rect, TextLayout};
use de_editor_core::editor_ui_state::EditorUiState;
use de_editor_core::ChromeButton;

pub const CANVAS_ACTIONS_HEIGHT: f32 = 38.0;
const RADIUS: f32 = 10.0;
const PAD: f32 = 5.0;
const BUTTON: f32 = 28.0;
const ICON: f32 = 16.0;
const GAP: f32 = 3.0;
const EXPORT_FONT: f32 = 13.0;
const EXPORT_PAD_X: f32 = 12.0;
const EXPORT_MIN_W: f32 = 61.0;
const EXPORT_RADIUS: f32 = 7.0;
/// The width estimate runs a little narrow of semibold Manrope; the slack
/// keeps the real label inside its button.
const ESTIMATE_SLACK: f32 = 1.15;

/// The group's rects, from one layout pass shared by paint and hit-test.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CanvasActionsLayout {
    pub rect: Rect,
    pub code: Rect,
    pub preview: Rect,
    pub download: Rect,
    pub export: Rect,
}

pub struct CanvasActions {
    pub label_export: &'static str,
    pub theme: Theme,
    /// The right panel is showing generated code.
    pub code_active: bool,
    /// Canvas is in Preview (Play) mode — the Play glyph becomes Stop.
    pub preview_active: bool,
    pub hover: Option<ChromeButton>,
    pub pressed: Option<ChromeButton>,
}

impl CanvasActions {
    pub fn for_editor_ui(ui: &EditorUiState) -> Self {
        Self {
            label_export: translate(ui, "export.title"),
            theme: theme_for(ui),
            code_active: ui.property_tab == de_editor_core::PropertyTab::Code,
            preview_active: ui.preview.mode,
            hover: ui.chrome_button_hover,
            pressed: match ui.pressed_button {
                Some(de_editor_core::ButtonPressTarget::Chrome(button)) => Some(button),
                _ => None,
            },
        }
    }

    fn export_width(&self) -> f32 {
        (estimate_width(self.label_export, EXPORT_FONT) * ESTIMATE_SLACK + EXPORT_PAD_X * 2.0)
            .max(EXPORT_MIN_W)
    }

    /// Total width of the group.
    pub fn width(&self) -> f32 {
        PAD * 2.0 + BUTTON * 3.0 + GAP * 3.0 + self.export_width()
    }

    /// Lay the group out with its top-right corner at `(right, top)`.
    pub fn layout_right(&self, right: f32, top: f32) -> CanvasActionsLayout {
        let rect = Rect::xywh(
            right - self.width(),
            top,
            self.width(),
            CANVAS_ACTIONS_HEIGHT,
        );
        let y = top + (CANVAS_ACTIONS_HEIGHT - BUTTON) / 2.0;
        let slot = |i: f32| Rect::xywh(rect.origin.x + PAD + i * (BUTTON + GAP), y, BUTTON, BUTTON);
        CanvasActionsLayout {
            rect,
            code: slot(0.0),
            preview: slot(1.0),
            download: slot(2.0),
            export: Rect::xywh(
                rect.origin.x + PAD + 3.0 * (BUTTON + GAP),
                y,
                self.export_width(),
                BUTTON,
            ),
        }
    }

    pub fn hit_test(&self, layout: &CanvasActionsLayout, point: Point2D) -> Option<ChromeHit> {
        if !layout.rect.contains(point) {
            return None;
        }
        [
            (layout.code, ChromeHit::ToggleCodeView),
            (layout.preview, ChromeHit::TogglePreview),
            (layout.download, ChromeHit::OpenExportMenu),
            (layout.export, ChromeHit::OpenExportDialog),
        ]
        .into_iter()
        .find_map(|(rect, hit)| rect.contains(point).then_some(hit))
    }

    /// The rect a chrome button's tooltip / dropdown anchors to.
    pub fn button_rect(&self, layout: &CanvasActionsLayout, button: ChromeButton) -> Option<Rect> {
        match button {
            ChromeButton::ToggleCodeView => Some(layout.code),
            ChromeButton::TogglePreview => Some(layout.preview),
            ChromeButton::OpenExportMenu => Some(layout.download),
            ChromeButton::OpenExportDialog => Some(layout.export),
            _ => None,
        }
    }

    fn paint_icon_button(
        &self,
        cx: &mut PaintCx<'_>,
        rect: Rect,
        icon: Icon,
        button: ChromeButton,
        active: bool,
    ) {
        let hovered = self.hover == Some(button);
        let pressed = self.pressed == Some(button);
        if hovered || pressed || active {
            cx.backend
                .fill_round_rect(rect, 7.0, self.theme.button_hover);
        }
        let color = if active {
            self.theme.primary
        } else if hovered || pressed {
            self.theme.foreground
        } else {
            self.theme.muted_foreground
        };
        draw_icon(
            cx.backend,
            icon,
            Point2D::new(
                rect.origin.x + (BUTTON - ICON) / 2.0,
                rect.origin.y + (BUTTON - ICON) / 2.0,
            ),
            ICON,
            color,
            1.5,
        );
    }

    pub fn paint_layout(&self, cx: &mut PaintCx<'_>, layout: &CanvasActionsLayout) {
        let theme = self.theme;
        cx.backend.fill_round_rect(layout.rect, RADIUS, theme.card);
        cx.backend
            .stroke_round_rect(layout.rect, RADIUS, theme.border, 1.0);
        self.paint_icon_button(
            cx,
            layout.code,
            Icon::Code,
            ChromeButton::ToggleCodeView,
            self.code_active,
        );
        let preview_icon = if self.preview_active {
            Icon::Square
        } else {
            Icon::Play
        };
        self.paint_icon_button(
            cx,
            layout.preview,
            preview_icon,
            ChromeButton::TogglePreview,
            self.preview_active,
        );
        self.paint_icon_button(
            cx,
            layout.download,
            Icon::Download,
            ChromeButton::OpenExportMenu,
            false,
        );

        let lifted = self.hover == Some(ChromeButton::OpenExportDialog)
            || self.pressed == Some(ChromeButton::OpenExportDialog);
        let fill = if lifted {
            Color {
                a: theme.primary.a * 0.88,
                ..theme.primary
            }
        } else {
            theme.primary
        };
        cx.backend
            .fill_round_rect(layout.export, EXPORT_RADIUS, fill);
        let label_w = measure_chrome_weighted(cx.backend, self.label_export, EXPORT_FONT, 600);
        let x = layout.export.origin.x + (layout.export.size.x - label_w) / 2.0;
        let run = TextLayout::single_run(
            self.label_export,
            CHROME_FONT_FAMILY,
            EXPORT_FONT,
            theme.primary_foreground.to_jian(),
            Point2D::ZERO,
        )
        .with_font_weight(600);
        cx.backend.draw_text(
            &run,
            Point2D::new(
                x,
                jian_widgets::centered_text_baseline_y(layout.export, EXPORT_FONT),
            ),
        );
    }
}

impl Widget for CanvasActions {
    fn id(&self) -> WidgetId {
        WidgetId::new(5001)
    }

    fn layout(&self, _cx: &LayoutCx) -> LayoutBox {
        LayoutBox {
            rect: Rect {
                origin: Point2D::ZERO,
                size: Point2D::new(self.width(), CANVAS_ACTIONS_HEIGHT),
            },
        }
    }

    fn paint(&self, cx: &mut PaintCx<'_>, rect: Rect) {
        self.paint_layout(
            cx,
            &self.layout_right(rect.origin.x + rect.size.x, rect.origin.y),
        );
    }

    fn access_node(&self) -> accesskit::Node {
        let mut node = accesskit::Node::new(accesskit::Role::Toolbar);
        node.set_label("Canvas actions");
        node
    }
}
