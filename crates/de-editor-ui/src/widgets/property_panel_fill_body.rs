//! Fill-section per-type body paints — extracted from
//! `property_panel_fill.rs` to honour the 800-line ceiling. Contains
//! the Linear / Radial gradient body (angle row + stop rows) and the
//! small hex / angle formatters it uses. `paint_fill_image_body` lives in
//! `property_panel_fill_image_body.rs`.

use crate::theme::Theme;
use crate::widgets::icons::{draw_icon, Icon};
use crate::widgets::property_panel::NodeSnapshot;
use crate::widgets::property_panel_inputs::{
    INPUT_HEIGHT, INPUT_RADIUS, PAD_X, SECTION_HEADER_HEIGHT,
};
use crate::widgets::property_panel_sections::EditContext;
use crate::widgets::text_metrics;
use crate::widgets::PaintCx;
use crate::{Point2D, Rect, TextLayout};
use de_editor_core::PropertyFocus;

#[allow(clippy::too_many_arguments)]
pub(crate) fn paint_fill_gradient_body(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    edit: &EditContext<'_>,
    snapshot: &NodeSnapshot,
    locale: de_editor_core::Locale,
    x: f32,
    y: f32,
    width: f32,
    show_angle: bool,
) {
    let usable_w = width - PAD_X * 2.0;
    let mut yy = y;
    if show_angle {
        let angle_rect = Rect {
            origin: Point2D::new(x + PAD_X, yy),
            size: Point2D::new(usable_w, INPUT_HEIGHT),
        };
        let angle_focus = PropertyFocus::GradientAngle;
        let angle_focused = edit.focus == Some(angle_focus);
        cx.backend
            .fill_round_rect(angle_rect, INPUT_RADIUS, theme.muted);
        if angle_focused {
            cx.backend
                .stroke_round_rect(angle_rect, INPUT_RADIUS, theme.primary, 1.5);
        }
        let prefix = TextLayout::single_run(
            de_i18n::translate(locale, "fill.angle"),
            crate::widgets::text_metrics::CHROME_FONT_FAMILY,
            12.0,
            (theme.muted_foreground).to_jian(),
            Point2D::new(0.0, 0.0),
        );
        cx.backend.draw_text(
            &prefix,
            Point2D::new(
                angle_rect.origin.x + 10.0,
                angle_rect.origin.y + crate::widgets::property_panel_inputs::INPUT_TEXT_BASELINE,
            ),
        );
        let angle_owned = format_angle(snapshot.gradient_angle.unwrap_or(0.0));
        let value_text = edit.value_for(angle_focus, &angle_owned);
        let value_x = angle_rect.origin.x + 44.0;
        if !edit.paint_input_view_at(
            cx,
            theme,
            angle_focus,
            Rect {
                origin: Point2D::new(value_x, angle_rect.origin.y),
                size: Point2D::new(
                    (angle_rect.origin.x + angle_rect.size.x - 18.0 - value_x).max(0.0),
                    angle_rect.size.y,
                ),
            },
            12.0,
            0.0,
            angle_rect.origin.y + crate::widgets::property_panel_inputs::INPUT_TEXT_BASELINE,
        ) {
            let value = TextLayout::single_run(
                value_text,
                crate::widgets::text_metrics::CHROME_FONT_FAMILY,
                12.0,
                (theme.foreground).to_jian(),
                Point2D::new(0.0, 0.0),
            );
            edit.paint_selection_at(
                cx,
                theme,
                angle_focus,
                value_text,
                value_x,
                angle_rect.origin.y + crate::widgets::property_panel_inputs::INPUT_TEXT_BASELINE,
                12.0,
                angle_rect.origin.x + angle_rect.size.x - 8.0,
            );
            cx.backend.draw_text(
                &value,
                Point2D::new(
                    value_x,
                    angle_rect.origin.y
                        + crate::widgets::property_panel_inputs::INPUT_TEXT_BASELINE,
                ),
            );
            if let Some(pos) = edit.caret_at(angle_focus) {
                let w = text_metrics::measure_chrome(
                    cx.backend,
                    &value_text[..pos.min(value_text.len())],
                    12.0,
                );
                cx.backend.fill_rect(
                    Rect {
                        origin: Point2D::new(value_x + w, angle_rect.origin.y + 6.0),
                        size: Point2D::new(1.5, angle_rect.size.y - 12.0),
                    },
                    theme.foreground,
                );
            }
        }
        let unit = TextLayout::single_run(
            "°",
            crate::widgets::text_metrics::CHROME_FONT_FAMILY,
            12.0,
            (theme.muted_foreground).to_jian(),
            Point2D::new(0.0, 0.0),
        );
        cx.backend.draw_text(
            &unit,
            Point2D::new(
                angle_rect.origin.x + angle_rect.size.x - 14.0,
                angle_rect.origin.y + crate::widgets::property_panel_inputs::INPUT_TEXT_BASELINE,
            ),
        );
        yy += INPUT_HEIGHT + 6.0;
    }
    // Color stops header + plus.
    let header = TextLayout::single_run(
        de_i18n::translate(locale, "fill.stops"),
        crate::widgets::text_metrics::CHROME_FONT_FAMILY,
        12.0,
        (theme.foreground).to_jian(),
        Point2D::new(0.0, 0.0),
    );
    cx.backend
        .draw_text(&header, Point2D::new(x + PAD_X, yy + 16.0));
    draw_icon(
        cx.backend,
        Icon::Plus,
        Point2D::new(x + width - PAD_X - 14.0, yy + 6.0),
        14.0,
        theme.muted_foreground,
        1.4,
    );
    yy += SECTION_HEADER_HEIGHT;
    let pct_w = 56.0;
    let show_remove = snapshot.gradient_stops.len() > 2;
    let remove_w = if show_remove { 26.0 } else { 0.0 };
    let remove_gap = if show_remove { 6.0 } else { 0.0 };
    for (index, stop) in snapshot.gradient_stops.iter().enumerate() {
        let row_y = yy;
        let hex_w = usable_w - pct_w - 8.0 - remove_w - remove_gap;
        let hex_rect = Rect {
            origin: Point2D::new(x + PAD_X, row_y),
            size: Point2D::new(hex_w, INPUT_HEIGHT),
        };
        let hex_focus = PropertyFocus::GradientStopHex(index);
        let hex_focused = edit.focus == Some(hex_focus);
        cx.backend
            .fill_round_rect(hex_rect, INPUT_RADIUS, theme.muted);
        if hex_focused {
            cx.backend
                .stroke_round_rect(hex_rect, INPUT_RADIUS, theme.primary, 1.5);
        }
        let swatch = Rect {
            origin: Point2D::new(hex_rect.origin.x + 6.0, hex_rect.origin.y + 5.0),
            size: Point2D::new(16.0, 16.0),
        };
        jian_widgets::components::swatch::Swatch {
            color: stop.color,
            radius: 3.0,
            border: false,
        }
        .paint(
            cx.backend,
            swatch,
            &crate::widgets::button::tokens_from_theme(theme),
        );
        // Display only `#RRGGBB` — the swatch on the left already
        // conveys per-stop transparency. Alpha is preserved at
        // commit time, so the user never types raw alpha digits.
        let hex_owned = stop_hex_rgb_only(&stop.hex);
        let hex_text = edit.value_for(hex_focus, &hex_owned);
        let hex_text_x = hex_rect.origin.x + 30.0;
        if !edit.paint_input_view_at(
            cx,
            theme,
            hex_focus,
            Rect {
                origin: Point2D::new(hex_text_x, hex_rect.origin.y),
                size: Point2D::new(
                    (hex_rect.origin.x + hex_rect.size.x - 8.0 - hex_text_x).max(0.0),
                    hex_rect.size.y,
                ),
            },
            12.0,
            0.0,
            hex_rect.origin.y + crate::widgets::property_panel_inputs::INPUT_TEXT_BASELINE,
        ) {
            let hex_layout = TextLayout::single_run(
                hex_text,
                crate::widgets::text_metrics::CHROME_FONT_FAMILY,
                12.0,
                (theme.foreground).to_jian(),
                Point2D::new(0.0, 0.0),
            );
            edit.paint_selection_at(
                cx,
                theme,
                hex_focus,
                hex_text,
                hex_text_x,
                hex_rect.origin.y + crate::widgets::property_panel_inputs::INPUT_TEXT_BASELINE,
                12.0,
                hex_rect.origin.x + hex_rect.size.x - 8.0,
            );
            cx.backend.draw_text(
                &hex_layout,
                Point2D::new(
                    hex_text_x,
                    hex_rect.origin.y + crate::widgets::property_panel_inputs::INPUT_TEXT_BASELINE,
                ),
            );
            if let Some(pos) = edit.caret_at(hex_focus) {
                let w = text_metrics::measure_chrome(
                    cx.backend,
                    &hex_text[..pos.min(hex_text.len())],
                    12.0,
                );
                cx.backend.fill_rect(
                    Rect {
                        origin: Point2D::new(hex_text_x + w, hex_rect.origin.y + 6.0),
                        size: Point2D::new(1.5, hex_rect.size.y - 12.0),
                    },
                    theme.foreground,
                );
            }
        }
        let pct_rect = Rect {
            origin: Point2D::new(x + PAD_X + hex_w + 8.0, row_y),
            size: Point2D::new(pct_w, INPUT_HEIGHT),
        };
        let offset_focus = PropertyFocus::GradientStopOffset(index);
        let offset_focused = edit.focus == Some(offset_focus);
        cx.backend
            .fill_round_rect(pct_rect, INPUT_RADIUS, theme.muted);
        if offset_focused {
            cx.backend
                .stroke_round_rect(pct_rect, INPUT_RADIUS, theme.primary, 1.5);
        }
        let pct_owned = ((stop.offset * 100.0).round() as i32).to_string();
        let pct_text = edit.value_for(offset_focus, &pct_owned);
        let pct_x = pct_rect.origin.x + 12.0;
        if !edit.paint_input_view_at(
            cx,
            theme,
            offset_focus,
            Rect {
                origin: Point2D::new(pct_x, pct_rect.origin.y),
                size: Point2D::new(
                    (pct_rect.origin.x + pct_rect.size.x - 18.0 - pct_x).max(0.0),
                    pct_rect.size.y,
                ),
            },
            12.0,
            0.0,
            pct_rect.origin.y + crate::widgets::property_panel_inputs::INPUT_TEXT_BASELINE,
        ) {
            let pct_layout = TextLayout::single_run(
                pct_text,
                crate::widgets::text_metrics::CHROME_FONT_FAMILY,
                12.0,
                (theme.foreground).to_jian(),
                Point2D::new(0.0, 0.0),
            );
            edit.paint_selection_at(
                cx,
                theme,
                offset_focus,
                pct_text,
                pct_x,
                pct_rect.origin.y + crate::widgets::property_panel_inputs::INPUT_TEXT_BASELINE,
                12.0,
                pct_rect.origin.x + pct_rect.size.x - 8.0,
            );
            cx.backend.draw_text(
                &pct_layout,
                Point2D::new(
                    pct_x,
                    pct_rect.origin.y + crate::widgets::property_panel_inputs::INPUT_TEXT_BASELINE,
                ),
            );
            if let Some(pos) = edit.caret_at(offset_focus) {
                let w = text_metrics::measure_chrome(
                    cx.backend,
                    &pct_text[..pos.min(pct_text.len())],
                    12.0,
                );
                cx.backend.fill_rect(
                    Rect {
                        origin: Point2D::new(pct_x + w, pct_rect.origin.y + 6.0),
                        size: Point2D::new(1.5, pct_rect.size.y - 12.0),
                    },
                    theme.foreground,
                );
            }
        }
        let pct_unit = TextLayout::single_run(
            "%",
            crate::widgets::text_metrics::CHROME_FONT_FAMILY,
            12.0,
            (theme.muted_foreground).to_jian(),
            Point2D::new(0.0, 0.0),
        );
        cx.backend.draw_text(
            &pct_unit,
            Point2D::new(
                pct_rect.origin.x + pct_rect.size.x - 14.0,
                pct_rect.origin.y + crate::widgets::property_panel_inputs::INPUT_TEXT_BASELINE,
            ),
        );
        if show_remove {
            draw_icon(
                cx.backend,
                Icon::Close,
                // Centre the 14px glyph in its 28px RemoveGradientStop wash
                // cell (x+width-PAD_X-22, size 28 — see the action walker).
                Point2D::new(
                    x + width - PAD_X - 22.0 + (28.0 - 14.0) / 2.0,
                    row_y + (INPUT_HEIGHT - 14.0) / 2.0,
                ),
                14.0,
                theme.muted_foreground,
                1.4,
            );
        }
        yy += INPUT_HEIGHT + 4.0;
    }
    let _ = yy;
}

/// Format the gradient angle for the panel input — drop the
/// fractional part when the angle is whole-degree so 0/45/90/180
/// don't paint as "0.0", "45.0", etc.
fn format_angle(angle: f32) -> String {
    if angle.fract() == 0.0 {
        format!("{}", angle as i32)
    } else {
        format!("{}", angle)
    }
}

/// Strip alpha from a canonical-schema stop hex (`#RRGGBB` or
/// `#RRGGBBAA`) for the panel input. The swatch on the left already
/// paints with alpha so the input stays at 6 hex chars regardless
/// of authored transparency.
pub fn stop_hex_rgb_only(hex: &str) -> String {
    let trimmed = hex.trim();
    let stripped = trimmed.trim_start_matches('#');
    let body = if stripped.len() >= 6 {
        &stripped[..6]
    } else {
        stripped
    };
    format!("#{}", body.to_uppercase())
}
