//! The Design view's closing "Code" section (#30): one chip per headline
//! framework. A chip picks that framework and opens the Code view, so the
//! generated code is one click from any selection without a tab strip.
//!
//! Geometry is backend-free ([`chip_rects`]) so the action-rect walker and
//! paint lay the chips out identically.

use de_editor_core::codegen::Framework;

use crate::theme::Theme;
use crate::widgets::property_panel_code::framework::framework_tab_label;
use crate::widgets::property_panel_inputs::{paint_section_label, PAD_X, SECTION_HEADER_HEIGHT};
use crate::widgets::text_metrics;
use crate::widgets::PaintCx;
use crate::{Point2D, Rect, TextLayout};

/// The frameworks the design offers here; the Code view lists them all.
pub const CODE_CHIPS: [Framework; 4] = [
    Framework::React,
    Framework::Vue,
    Framework::Html,
    Framework::SwiftUi,
];

const CHIP_H: f32 = 26.0;
const CHIP_GAP: f32 = 6.0;
const CHIP_PAD_X: f32 = 9.0;
const CHIP_RADIUS: f32 = 6.0;
const CHIP_FONT: f32 = 12.0;
const BOTTOM_PAD: f32 = 16.0;

/// Backend-free label width — a little wide of the shaper, so the chip
/// never clips its label.
fn label_width(s: &str) -> f32 {
    s.chars().count() as f32 * CHIP_FONT * 0.6
}

/// Total height the section takes, starting at its top `y`.
pub fn section_height() -> f32 {
    SECTION_HEADER_HEIGHT + CHIP_H + BOTTOM_PAD
}

/// The chips for a section whose top is `y`, left to right.
pub fn chip_rects(x0: f32, y: f32, width: f32) -> Vec<(Framework, Rect)> {
    let top = y + SECTION_HEADER_HEIGHT;
    let right = x0 + width - PAD_X;
    let mut x = x0 + PAD_X;
    let mut out = Vec::with_capacity(CODE_CHIPS.len());
    for fw in CODE_CHIPS {
        let w = label_width(framework_tab_label(fw)) + CHIP_PAD_X * 2.0;
        if x + w > right {
            break;
        }
        out.push((fw, Rect::xywh(x, top, w, CHIP_H)));
        x += w + CHIP_GAP;
    }
    out
}

/// Paint the section; `current` is the Code view's framework, shown as
/// the filled chip. Returns the y below the section.
pub fn paint_code_chips_section(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    title: &str,
    current: Framework,
    x: f32,
    y: f32,
    width: f32,
) -> f32 {
    paint_section_label(cx, theme, title, x, y, width);
    for (fw, rect) in chip_rects(x, y, width) {
        let active = fw == current;
        if active {
            cx.backend.fill_round_rect(rect, CHIP_RADIUS, theme.muted);
        } else {
            cx.backend
                .stroke_round_rect(rect, CHIP_RADIUS, theme.border, 1.0);
        }
        let text = framework_tab_label(fw);
        let w = text_metrics::measure_chrome_weighted(cx.backend, text, CHIP_FONT, 500);
        cx.backend.draw_text(
            &TextLayout::single_run(
                text,
                text_metrics::CHROME_FONT_FAMILY,
                CHIP_FONT,
                if active {
                    theme.foreground
                } else {
                    theme.muted_foreground
                }
                .to_jian(),
                Point2D::ZERO,
            )
            .with_font_weight(500),
            Point2D::new(
                rect.origin.x + (rect.size.x - w) / 2.0,
                jian_widgets::centered_text_baseline_y(rect, CHIP_FONT),
            ),
        );
    }
    y + section_height()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chips_sit_under_the_title_left_to_right() {
        let chips = chip_rects(1168.0, 500.0, 272.0);
        assert_eq!(chips.len(), CODE_CHIPS.len());
        assert_eq!(chips[0].1.origin.y, 500.0 + SECTION_HEADER_HEIGHT);
        assert!(chips
            .windows(2)
            .all(|w| w[0].1.origin.x + w[0].1.size.x < w[1].1.origin.x));
        let last = chips.last().unwrap().1;
        assert!(last.origin.x + last.size.x <= 1168.0 + 272.0 - PAD_X);
    }

    #[test]
    fn a_narrow_panel_drops_chips_rather_than_overflowing() {
        let chips = chip_rects(0.0, 0.0, 160.0);
        assert!(chips.len() < CODE_CHIPS.len());
        assert!(chips
            .iter()
            .all(|(_, r)| r.origin.x + r.size.x <= 160.0 - PAD_X));
    }
}
