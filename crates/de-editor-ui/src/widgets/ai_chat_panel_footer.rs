//! The composer card's bottom row (#26): "+" on the left, the
//! "model · provider" chip and the round send / stop button on the right.
//!
//! The four controls that used to share this row (library, thinking,
//! parallel agents, attach) moved into the "+" menu
//! (`ai_chat_plus_menu`). The parallel-agents picker they open still lives
//! here, now anchored to "+".
//!
//! The `stop` rect shares the `send` slot — the circle toggles send ↑ ↔
//! stop ◼ in place while a turn streams.

use super::ai_chat_panel::{
    AIChatPlaceholder, FooterLayout, COMPOSER_TEXT_INSET, INPUT_TOOLBAR_HEIGHT,
};
use crate::widgets::ai_chat_chrome::{chat_label, fit_label, label_baseline};
use crate::widgets::icons::{draw_icon, Icon};
use crate::widgets::PaintCx;
use crate::{Point2D, Rect, TextLayout};

/// Number of rows in the Parallel Agents picker (1x..6x).
pub(crate) const PARALLEL_AGENTS_COUNT: u32 = 6;
/// Height of each row in the Parallel Agents picker overlay.
/// Exported as `_PUB` so the hit-test module can use it without a re-export dance.
pub(crate) const PARALLEL_AGENTS_ROW_H_PUB: f32 = 28.0;
const PARALLEL_AGENTS_ROW_H: f32 = PARALLEL_AGENTS_ROW_H_PUB;
/// Width of the Parallel Agents picker overlay.
const PARALLEL_AGENTS_PICKER_W: f32 = 130.0;

/// Side of the "+" button.
pub(crate) const FOOTER_PLUS_D: f32 = 26.0;
/// Diameter of the round send / stop button.
pub(crate) const FOOTER_CIRCLE_D: f32 = 28.0;
/// Height of the model chip.
const MODEL_CHIP_H: f32 = 26.0;
const MODEL_FONT: f32 = 12.0;
/// Widest the model chip grows before its label is ellipsized.
const MODEL_CHIP_MAX_W: f32 = 200.0;
const FOOTER_GAP: f32 = 6.0;
/// Inset of the row's ends from the card's edges.
const ROW_INSET: f32 = 12.0 - COMPOSER_TEXT_INSET;

impl<'a> AIChatPlaceholder<'a> {
    /// The row's rects for the panel laid out at `rect`.
    pub(crate) fn footer_layout(&self, rect: Rect) -> FooterLayout {
        let card = self.composer_card(rect);
        let cy = self.toolbar_top(rect) + INPUT_TOOLBAR_HEIGHT / 2.0 - 2.0;
        let left = card.origin.x + COMPOSER_TEXT_INSET + ROW_INSET;
        let right = card.origin.x + card.size.x - COMPOSER_TEXT_INSET - ROW_INSET;
        let plus = Rect::xywh(left, cy - FOOTER_PLUS_D / 2.0, FOOTER_PLUS_D, FOOTER_PLUS_D);
        let send = Rect::xywh(
            right - FOOTER_CIRCLE_D,
            cy - FOOTER_CIRCLE_D / 2.0,
            FOOTER_CIRCLE_D,
            FOOTER_CIRCLE_D,
        );
        let label = if self.usable_agent {
            self.model_chip_label.as_str()
        } else {
            self.label_no_models.as_str()
        };
        let room =
            (send.origin.x - FOOTER_GAP - (plus.origin.x + plus.size.x + FOOTER_GAP)).max(0.0);
        let want = super::ai_chat_panel::footer_label_width(label, MODEL_FONT) + 8.0 + 8.0 + 18.0;
        let model_w = want.min(MODEL_CHIP_MAX_W).min(room);
        let model = Rect::xywh(
            send.origin.x - FOOTER_GAP - model_w,
            cy - MODEL_CHIP_H / 2.0,
            model_w,
            MODEL_CHIP_H,
        );
        FooterLayout {
            plus,
            model,
            stop: send,
            send,
        }
    }
}

/// Ellipsize a chip label to `max_w`, measured through the backend in
/// the family the run paints with (the minimized bar's model label).
pub(crate) fn fit_footer_label(
    backend: &mut dyn crate::RenderBackend,
    label: &str,
    size: f32,
    max_w: f32,
) -> String {
    crate::widgets::text_metrics::fit_chrome(backend, label, max_w, size)
}

/// Returns the bounding rect of the Parallel Agents picker, opened from
/// its "+" menu row: it floats upward from the "+" button, where that
/// menu was.
pub(crate) fn parallel_agents_picker_rect(footer: &FooterLayout) -> Rect {
    let total_h = PARALLEL_AGENTS_ROW_H * PARALLEL_AGENTS_COUNT as f32 + 32.0; // 32 = title row
    let bottom = footer.plus.origin.y - 6.0;
    Rect::xywh(
        footer.plus.origin.x,
        bottom - total_h,
        PARALLEL_AGENTS_PICKER_W,
        total_h,
    )
}

/// Paint the Parallel Agents picker overlay above the ⚡ chip.
///
/// Draws a card with a "PARALLEL AGENTS" header and 6 rows "1x"–"6x".
/// The current value (`selected`) is highlighted. The hovered row
/// (`hover`) receives a subtle wash.
pub(crate) fn paint_parallel_agents_picker(
    cx: &mut PaintCx<'_>,
    theme: &crate::theme::Theme,
    footer: &FooterLayout,
    selected: u32,
    hover: Option<u32>,
) {
    let picker = parallel_agents_picker_rect(footer);

    // Card background + border.
    cx.backend.fill_round_rect(picker, 8.0, theme.card);
    cx.backend
        .stroke_round_rect(picker, 8.0, (theme.border).with_alpha(0.8), 1.0);

    // Title row — "PARALLEL AGENTS" in muted small caps.
    let title_x = picker.origin.x + 10.0;
    let title_y = picker.origin.y + 20.0; // baseline
    let title_label = TextLayout::single_run(
        "PARALLEL AGENTS",
        "system-ui",
        9.0,
        (theme.muted_foreground).to_jian(),
        Point2D::new(0.0, 0.0),
    );
    cx.backend
        .draw_text(&title_label, Point2D::new(title_x, title_y));

    // Rows 1x–6x.
    let rows_top = picker.origin.y + 32.0;
    for i in 1..=PARALLEL_AGENTS_COUNT {
        let row_y = rows_top + (i - 1) as f32 * PARALLEL_AGENTS_ROW_H;
        let row = Rect::xywh(
            picker.origin.x + 4.0,
            row_y,
            picker.size.x - 8.0,
            PARALLEL_AGENTS_ROW_H,
        );

        // Hover / selected highlight.
        let is_selected = i == selected;
        let is_hovered = hover == Some(i);
        if is_selected {
            cx.backend
                .fill_round_rect(row, 5.0, (theme.primary).with_alpha(0.18));
        } else if is_hovered {
            cx.backend
                .fill_round_rect(row, 5.0, (theme.muted).with_alpha(0.25));
        }

        // "Nx" label — gold when selected, muted-foreground otherwise.
        let label = format!("{}x", i);
        let label_color = if is_selected {
            theme.speed_accent
        } else {
            theme.muted_foreground
        };
        let label_baseline = row_y + PARALLEL_AGENTS_ROW_H / 2.0 + 11.0 * 0.35;
        let text = TextLayout::single_run(
            &label,
            "system-ui",
            11.0,
            label_color.to_jian(),
            Point2D::new(0.0, 0.0),
        );
        cx.backend
            .draw_text(&text, Point2D::new(row.origin.x + 10.0, label_baseline));

        // ⚡ icon in gold to the right for the selected row, for visual flair.
        if is_selected {
            draw_icon(
                cx.backend,
                Icon::Zap,
                Point2D::new(
                    row.origin.x + row.size.x - 20.0,
                    row_y + (PARALLEL_AGENTS_ROW_H - 12.0) / 2.0,
                ),
                12.0,
                theme.speed_accent,
                1.6,
            );
        }
    }
}

/// Paint the composer card's bottom row. `send_active` is true when the
/// input has something to send and a model can take it.
pub(crate) fn paint_bottom_toolbar(
    cx: &mut PaintCx<'_>,
    widget: &AIChatPlaceholder<'_>,
    footer: &FooterLayout,
    streaming: bool,
    send_active: bool,
) {
    use de_editor_core::ChatFooterButton;
    let p = &widget.palette;
    let active =
        |button| widget.footer_hover == Some(button) || widget.footer_pressed == Some(button);

    // "+" — opens the menu.
    if widget.plus_menu_open || active(ChatFooterButton::PlusMenu) {
        cx.backend.fill_round_rect(footer.plus, 6.0, p.subtle);
    }
    draw_icon(
        cx.backend,
        Icon::Plus,
        Point2D::new(
            footer.plus.origin.x + (FOOTER_PLUS_D - 16.0) / 2.0,
            footer.plus.origin.y + (FOOTER_PLUS_D - 16.0) / 2.0,
        ),
        16.0,
        p.text_2,
        1.6,
    );

    // "model · provider", or the link to add a provider when none can answer.
    let chip = footer.model;
    if chip.size.x > 0.0 {
        let baseline = label_baseline(chip, MODEL_FONT);
        if widget.usable_agent {
            let hovered = active(ChatFooterButton::ModelPicker) || widget.model_picker.open;
            if hovered {
                cx.backend.fill_round_rect(chip, 6.0, p.subtle);
            }
            let label = fit_label(
                cx,
                &widget.model_chip_label,
                MODEL_FONT,
                (chip.size.x - 8.0 - 22.0).max(0.0),
            );
            chat_label(
                cx,
                &label,
                MODEL_FONT,
                if hovered { p.text } else { p.text_2 },
                400,
                Point2D::new(chip.origin.x + 8.0, baseline),
            );
            draw_icon(
                cx.backend,
                Icon::ChevronDown,
                Point2D::new(
                    chip.origin.x + chip.size.x - 8.0 - 12.0,
                    chip.origin.y + (chip.size.y - 12.0) / 2.0,
                ),
                12.0,
                p.text_3,
                1.6,
            );
        } else {
            let hovered = active(ChatFooterButton::ModelPicker);
            let label = fit_label(
                cx,
                &widget.label_no_models,
                MODEL_FONT,
                (chip.size.x - 8.0 - 22.0).max(0.0),
            );
            chat_label(
                cx,
                &label,
                MODEL_FONT,
                if hovered { p.text } else { p.accent },
                500,
                Point2D::new(chip.origin.x + 8.0, baseline),
            );
            draw_icon(
                cx.backend,
                Icon::ArrowRight,
                Point2D::new(
                    chip.origin.x + chip.size.x - 8.0 - 12.0,
                    chip.origin.y + (chip.size.y - 12.0) / 2.0,
                ),
                12.0,
                p.accent,
                1.8,
            );
        }
    }

    // Round send, or stop in the same slot while a turn streams.
    let circle = footer.send;
    let r = FOOTER_CIRCLE_D / 2.0;
    if streaming {
        let pressed = widget.footer_pressed == Some(ChatFooterButton::Stop);
        let fill = if pressed || widget.footer_hover == Some(ChatFooterButton::Stop) {
            p.text_2
        } else {
            p.text
        };
        cx.backend.fill_round_rect(circle, r, fill);
        let glyph = 10.0;
        cx.backend.fill_round_rect(
            Rect::xywh(
                circle.origin.x + (FOOTER_CIRCLE_D - glyph) / 2.0,
                circle.origin.y + (FOOTER_CIRCLE_D - glyph) / 2.0,
                glyph,
                glyph,
            ),
            2.0,
            p.bg,
        );
        return;
    }
    let pressed = widget.footer_pressed == Some(ChatFooterButton::Send);
    let hovered = widget.footer_hover == Some(ChatFooterButton::Send);
    let (fill, ink) = if send_active {
        let alpha = if pressed {
            0.8
        } else if hovered {
            0.9
        } else {
            1.0
        };
        (p.accent.with_alpha(alpha), p.on_accent)
    } else {
        (p.subtle, p.text_3)
    };
    cx.backend.fill_round_rect(circle, r, fill);
    draw_icon(
        cx.backend,
        Icon::ArrowUp,
        Point2D::new(
            circle.origin.x + (FOOTER_CIRCLE_D - 15.0) / 2.0,
            circle.origin.y + (FOOTER_CIRCLE_D - 15.0) / 2.0,
        ),
        15.0,
        ink,
        1.8,
    );
}
