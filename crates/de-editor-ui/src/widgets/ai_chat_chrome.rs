//! Small paint helpers the chat panel's #26 chrome shares: Home's sans
//! labels and the outlined control every row of the redesign uses.

use crate::widgets::home_surface::{HomePalette, SANS};
use crate::widgets::{text_metrics, PaintCx};
use crate::{Color, Point2D, Rect, TextLayout};

/// Paint `text` in Home's sans face with its baseline at `origin.y`.
pub(crate) fn chat_label(
    cx: &mut PaintCx<'_>,
    text: &str,
    size: f32,
    color: Color,
    weight: u16,
    origin: Point2D,
) {
    if text.is_empty() {
        return;
    }
    let layout = TextLayout::single_run(text, SANS, size, color.to_jian(), Point2D::ZERO)
        .with_font_weight(weight);
    cx.backend.draw_text(&layout, origin);
}

/// `text` shortened with an ellipsis to fit `max_w` at `size`.
pub(crate) fn fit_label(cx: &mut PaintCx<'_>, text: &str, size: f32, max_w: f32) -> String {
    text_metrics::fit_chrome(cx.backend, text, max_w, size)
}

/// Baseline that centres a `size` label vertically in `rect`.
pub(crate) fn label_baseline(rect: Rect, size: f32) -> f32 {
    jian_widgets::centered_text_baseline_y(rect, size)
}

/// An outlined control: hairline border, `subtle` wash when hovered or
/// pressed.
pub(crate) fn outline_control(
    cx: &mut PaintCx<'_>,
    palette: &HomePalette,
    rect: Rect,
    radius: f32,
    active: bool,
) {
    if active {
        cx.backend.fill_round_rect(rect, radius, palette.subtle);
    }
    cx.backend
        .stroke_round_rect(rect, radius, palette.border, 1.0);
}

/// A dropdown card: `surface` fill, border and the soft drop shadow
/// Home's popovers use.
pub(crate) fn dropdown_card(cx: &mut PaintCx<'_>, palette: &HomePalette, rect: Rect) {
    let shadow = Rect::xywh(rect.origin.x, rect.origin.y + 4.0, rect.size.x, rect.size.y);
    cx.backend.fill_round_rect(
        shadow,
        10.0,
        Color {
            a: palette.shadow.a * 0.5,
            ..palette.shadow
        },
    );
    cx.backend.fill_round_rect(rect, 10.0, palette.surface);
    cx.backend
        .stroke_round_rect(rect, 10.0, palette.border, 1.0);
}
