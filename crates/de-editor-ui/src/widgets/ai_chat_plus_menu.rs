//! The composer's "+" menu (#26): attach a file, open the style library,
//! cycle the thinking mode and pick the parallel-agents count. These
//! used to be four icons in the composer row; the redesign keeps that row
//! to "+", the model chip and send. Rects here are the single source paint
//! and hit-test read.

use crate::widgets::ai_chat_chrome::{chat_label, dropdown_card, fit_label, label_baseline};
use crate::widgets::ai_chat_hit::AIChatHit;
use crate::widgets::home_surface::HomePalette;
use crate::widgets::icons::{draw_icon, Icon};
use crate::widgets::PaintCx;
use crate::{Point2D, Rect};
use de_editor_core::chat::ThinkingMode;
use de_editor_core::Locale;

pub(crate) const PLUS_MENU_ROWS: u8 = 4;
const ROW_H: f32 = 34.0;
const MENU_W: f32 = 236.0;
const MENU_PAD: f32 = 4.0;
const MENU_GAP: f32 = 6.0;
const GLYPH: f32 = 15.0;
const LABEL_FONT: f32 = 13.0;

/// The menu, opening upward from the "+" button and kept inside `panel`.
pub(crate) fn plus_menu_rect(plus: Rect, panel: Rect) -> Rect {
    let h = PLUS_MENU_ROWS as f32 * ROW_H + MENU_PAD * 2.0;
    let max_x = (panel.origin.x + panel.size.x - 8.0 - MENU_W).max(panel.origin.x + 8.0);
    let x = plus.origin.x.min(max_x);
    Rect::xywh(x, plus.origin.y - MENU_GAP - h, MENU_W, h)
}

fn row_rect(menu: Rect, row: u8) -> Rect {
    Rect::xywh(
        menu.origin.x + MENU_PAD,
        menu.origin.y + MENU_PAD + row as f32 * ROW_H,
        menu.size.x - MENU_PAD * 2.0,
        ROW_H,
    )
}

/// The row under `point`, or `None` off the rows.
pub(crate) fn plus_menu_row_at(menu: Rect, point: Point2D) -> Option<u8> {
    (0..PLUS_MENU_ROWS).find(|row| row_rect(menu, *row).contains(point))
}

/// Rows that wait while a turn streams: attaching (as the old paperclip
/// did) and the team size (the old chip was inert mid-turn).
fn row_waits_for_turn(row: u8) -> bool {
    row == 0 || row == 3
}

/// What pressing `row` does.
pub(crate) fn plus_menu_row_hit(row: u8, streaming: bool) -> AIChatHit {
    if streaming && row_waits_for_turn(row) {
        return AIChatHit::Inside;
    }
    match row {
        0 => AIChatHit::AddAttachment,
        1 => AIChatHit::OpenPromptCenter,
        2 => AIChatHit::CycleThinking,
        _ => AIChatHit::ToggleParallelAgentsPicker,
    }
}

/// What the menu shows for one frame.
pub(crate) struct PlusMenu {
    pub(crate) thinking: ThinkingMode,
    pub(crate) agent_team_size: u32,
    pub(crate) streaming: bool,
    pub(crate) hover: Option<u8>,
    pub(crate) locale: Locale,
}

pub(crate) fn paint_plus_menu(
    cx: &mut PaintCx<'_>,
    palette: &HomePalette,
    menu: Rect,
    state: &PlusMenu,
) {
    dropdown_card(cx, palette, menu);
    let t = |key| de_i18n::translate(state.locale, key);
    let team = format!("{}x", state.agent_team_size);
    let rows: [(Icon, &str, Option<&str>); 4] = [
        (Icon::Paperclip, t("chat.plus.attach"), None),
        (Icon::Library, t("chat.plus.library"), None),
        (
            Icon::Brain,
            t(crate::widgets::ai_chat_thinking_toggle::thinking_tooltip_key(state.thinking)),
            None,
        ),
        (Icon::Zap, t("chat.plus.parallel"), Some(team.as_str())),
    ];
    for (index, (icon, label, value)) in rows.into_iter().enumerate() {
        let row = row_rect(menu, index as u8);
        let disabled = state.streaming && row_waits_for_turn(index as u8);
        if state.hover == Some(index as u8) && !disabled {
            cx.backend.fill_round_rect(row, 6.0, palette.subtle);
        }
        let ink = if disabled {
            palette.text_3
        } else {
            palette.text
        };
        draw_icon(
            cx.backend,
            icon,
            Point2D::new(
                row.origin.x + 10.0,
                row.origin.y + (row.size.y - GLYPH) / 2.0,
            ),
            GLYPH,
            if disabled {
                palette.text_3
            } else {
                palette.text_2
            },
            1.5,
        );
        let text_x = row.origin.x + 10.0 + GLYPH + 10.0;
        let value_w = if value.is_some() { 36.0 } else { 0.0 };
        let fitted = fit_label(
            cx,
            label,
            LABEL_FONT,
            (row.origin.x + row.size.x - 10.0 - value_w - text_x).max(0.0),
        );
        let baseline = label_baseline(row, LABEL_FONT);
        chat_label(
            cx,
            &fitted,
            LABEL_FONT,
            ink,
            400,
            Point2D::new(text_x, baseline),
        );
        if let Some(value) = value {
            let w = crate::widgets::text_metrics::measure_chrome(cx.backend, value, LABEL_FONT);
            chat_label(
                cx,
                value,
                LABEL_FONT,
                palette.text_3,
                400,
                Point2D::new(row.origin.x + row.size.x - 10.0 - w, baseline),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_opens_upward_from_the_plus_and_stays_in_the_panel() {
        let panel = Rect::xywh(0.0, 0.0, 320.0, 800.0);
        let plus = Rect::xywh(24.0, 740.0, 26.0, 26.0);
        let menu = plus_menu_rect(plus, panel);
        assert_eq!(menu.origin.x, 24.0);
        assert!(menu.origin.y + menu.size.y <= plus.origin.y);
        assert!(menu.origin.x + menu.size.x <= panel.size.x);
        // A narrow floating panel still keeps the card on its left edge.
        let narrow = plus_menu_rect(plus, Rect::xywh(0.0, 0.0, 200.0, 800.0));
        assert_eq!(narrow.origin.x, 8.0);
    }

    #[test]
    fn rows_resolve_to_the_controls_they_replace() {
        let menu = plus_menu_rect(
            Rect::xywh(24.0, 740.0, 26.0, 26.0),
            Rect::xywh(0.0, 0.0, 320.0, 800.0),
        );
        let row = |i: u8| {
            let r = row_rect(menu, i);
            Point2D::new(r.origin.x + 20.0, r.origin.y + 10.0)
        };
        assert_eq!(plus_menu_row_at(menu, row(2)), Some(2));
        assert_eq!(plus_menu_row_at(menu, Point2D::new(0.0, 0.0)), None);
        assert_eq!(plus_menu_row_hit(0, false), AIChatHit::AddAttachment);
        assert_eq!(plus_menu_row_hit(0, true), AIChatHit::Inside);
        assert_eq!(plus_menu_row_hit(1, false), AIChatHit::OpenPromptCenter);
        assert_eq!(plus_menu_row_hit(2, true), AIChatHit::CycleThinking);
        assert_eq!(
            plus_menu_row_hit(3, false),
            AIChatHit::ToggleParallelAgentsPicker
        );
        assert_eq!(plus_menu_row_hit(3, true), AIChatHit::Inside);
    }
}
