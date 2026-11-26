//! The composer's context panel (#398): a popover above the footer's
//! context button listing what the next turn carries — the document, the
//! canvas selection, staged `@`-mentions and attachments — each staged
//! item with a ✕ that unstages it.
//!
//! Rows are built by `de_editor_core::chat_context::chat_context_rows`, and
//! only while the panel is open (see `AIChatPlaceholder::from_editor_at`).

use super::ai_chat_panel::AIChatPlaceholder;
use crate::widgets::ai_chat_chrome::{chat_label, dropdown_card, fit_label, label_baseline};
use crate::widgets::ai_chat_hit::AIChatHit;
use crate::widgets::home_surface::HomePalette;
use crate::widgets::icons::{draw_icon, Icon};
use crate::widgets::PaintCx;
use crate::{Point2D, Rect};
use de_editor_core::chat_context::{ChatContextItem, ChatContextRow};
use de_editor_core::Locale;

const TITLE_H: f32 = 30.0;
const ROW_H: f32 = 30.0;
const PAD: f32 = 4.0;
const GAP: f32 = 6.0;
const PANEL_W: f32 = 300.0;
/// Narrowest the tag column ("Layer", "File", …) gets; it grows to the
/// widest measured tag, up to [`TAG_MAX_SHARE`] of the panel.
const TAG_MIN_W: f32 = 48.0;
const TAG_MAX_SHARE: f32 = 0.4;
const TAG_GAP: f32 = 12.0;
const CLEAR_W: f32 = 24.0;
const TITLE_FONT: f32 = 11.5;
const LABEL_FONT: f32 = 12.5;
const TAG_FONT: f32 = 11.0;

/// The panel for `rows` rows, opening upward from `button` and kept inside
/// `panel` horizontally.
pub(crate) fn context_panel_rect(button: Rect, panel: Rect, rows: usize) -> Rect {
    let h = TITLE_H + rows as f32 * ROW_H + PAD * 2.0;
    let w = PANEL_W.min((panel.size.x - 16.0).max(0.0));
    let max_x = (panel.origin.x + panel.size.x - 8.0 - w).max(panel.origin.x + 8.0);
    let x = button.origin.x.min(max_x);
    Rect::xywh(x, button.origin.y - GAP - h, w, h)
}

fn row_rect(menu: Rect, row: usize) -> Rect {
    Rect::xywh(
        menu.origin.x + PAD,
        menu.origin.y + PAD + TITLE_H + row as f32 * ROW_H,
        menu.size.x - PAD * 2.0,
        ROW_H,
    )
}

fn clear_rect(row: Rect) -> Rect {
    Rect::xywh(
        row.origin.x + row.size.x - CLEAR_W,
        row.origin.y,
        CLEAR_W,
        row.size.y,
    )
}

/// What a press inside the open panel does: a ✕ unstages its item, any
/// other point is swallowed.
pub(crate) fn context_panel_hit(menu: Rect, rows: &[ChatContextRow], point: Point2D) -> AIChatHit {
    for (index, row) in rows.iter().enumerate() {
        let rect = row_rect(menu, index);
        if row.item.removable() && clear_rect(rect).contains(point) {
            return match row.item {
                ChatContextItem::Mention(i, _) => AIChatHit::RemoveMention(i),
                ChatContextItem::Attachment(i) => AIChatHit::RemoveAttachment(i),
                ChatContextItem::Selection => AIChatHit::ClearSelection,
                ChatContextItem::Document
                | ChatContextItem::LastReply
                | ChatContextItem::ChatTotal => AIChatHit::Inside,
            };
        }
    }
    AIChatHit::Inside
}

impl<'a> AIChatPlaceholder<'a> {
    /// The open context panel, or `None` when it is closed.
    pub(crate) fn context_panel_bounds(&self, rect: Rect) -> Option<Rect> {
        let rows = self.context_rows.as_ref()?;
        if self.state.is_minimized() && !self.composer_only {
            return None;
        }
        Some(context_panel_rect(
            self.footer_layout(rect).context,
            rect,
            rows.len(),
        ))
    }
}

/// Paint the panel: a title, then one row per item — tag, label and,
/// for staged items, a ✕.
pub(crate) fn paint_context_panel(
    cx: &mut PaintCx<'_>,
    palette: &HomePalette,
    menu: Rect,
    rows: &[ChatContextRow],
    locale: Locale,
) {
    dropdown_card(cx, palette, menu);
    let title = Rect::xywh(
        menu.origin.x + PAD,
        menu.origin.y + PAD,
        menu.size.x - PAD * 2.0,
        TITLE_H,
    );
    chat_label(
        cx,
        de_i18n::translate(locale, "chat.context.title"),
        TITLE_FONT,
        palette.text_3,
        500,
        Point2D::new(title.origin.x + 10.0, label_baseline(title, TITLE_FONT)),
    );
    // The tag column is paint-only (no hit target depends on it), so it is
    // sized from the tags as painted, in the active locale.
    let widest_tag = rows
        .iter()
        .map(|row| {
            crate::widgets::text_metrics::measure_chrome(
                cx.backend,
                de_i18n::translate(locale, row.item.tag_key()),
                TAG_FONT,
            )
        })
        .fold(0.0_f32, f32::max);
    let tag_w = (widest_tag + 10.0 + TAG_GAP).clamp(TAG_MIN_W, menu.size.x * TAG_MAX_SHARE);
    for (index, row) in rows.iter().enumerate() {
        let rect = row_rect(menu, index);
        let tag = fit_label(
            cx,
            de_i18n::translate(locale, row.item.tag_key()),
            TAG_FONT,
            (tag_w - 10.0 - TAG_GAP).max(0.0),
        );
        chat_label(
            cx,
            &tag,
            TAG_FONT,
            palette.text_3,
            400,
            Point2D::new(rect.origin.x + 10.0, label_baseline(rect, TAG_FONT)),
        );
        let text_x = rect.origin.x + tag_w;
        let right = if row.item.removable() {
            rect.origin.x + rect.size.x - CLEAR_W - 4.0
        } else {
            rect.origin.x + rect.size.x - 10.0
        };
        let label = fit_label(cx, &row.label, LABEL_FONT, (right - text_x).max(0.0));
        chat_label(
            cx,
            &label,
            LABEL_FONT,
            palette.text,
            400,
            Point2D::new(text_x, label_baseline(rect, LABEL_FONT)),
        );
        if row.item.removable() {
            let clear = clear_rect(rect);
            draw_icon(
                cx.backend,
                Icon::Close,
                Point2D::new(
                    clear.origin.x + (CLEAR_W - 12.0) / 2.0,
                    clear.origin.y + (ROW_H - 12.0) / 2.0,
                ),
                12.0,
                palette.text_3,
                1.5,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use de_editor_core::chat_mentions::MentionKind;

    fn rows() -> Vec<ChatContextRow> {
        vec![
            ChatContextRow {
                item: ChatContextItem::Document,
                label: "Home".into(),
            },
            ChatContextRow {
                item: ChatContextItem::Selection,
                label: "Hero".into(),
            },
            ChatContextRow {
                item: ChatContextItem::Mention(0, MentionKind::Node),
                label: "Title".into(),
            },
            ChatContextRow {
                item: ChatContextItem::Attachment(1),
                label: "ref.png".into(),
            },
        ]
    }

    #[test]
    fn panel_opens_upward_and_stays_in_the_chat() {
        let panel = Rect::xywh(0.0, 0.0, 280.0, 800.0);
        let button = Rect::xywh(60.0, 740.0, 26.0, 26.0);
        let menu = context_panel_rect(button, panel, 4);
        assert!(menu.origin.y + menu.size.y <= button.origin.y);
        assert!(menu.origin.x + menu.size.x <= panel.size.x - 8.0 + 0.01);
        assert!(menu.origin.x >= 8.0);
    }

    #[test]
    fn each_clear_unstages_its_own_item() {
        let rows = rows();
        let menu = context_panel_rect(
            Rect::xywh(40.0, 740.0, 26.0, 26.0),
            Rect::xywh(0.0, 0.0, 400.0, 800.0),
            rows.len(),
        );
        let clear_of = |i: usize| {
            let c = clear_rect(row_rect(menu, i));
            Point2D::new(c.origin.x + 6.0, c.origin.y + 10.0)
        };
        assert_eq!(
            context_panel_hit(menu, &rows, clear_of(0)),
            AIChatHit::Inside
        );
        assert_eq!(
            context_panel_hit(menu, &rows, clear_of(1)),
            AIChatHit::ClearSelection
        );
        assert_eq!(
            context_panel_hit(menu, &rows, clear_of(2)),
            AIChatHit::RemoveMention(0)
        );
        assert_eq!(
            context_panel_hit(menu, &rows, clear_of(3)),
            AIChatHit::RemoveAttachment(1)
        );
        let label = row_rect(menu, 2);
        assert_eq!(
            context_panel_hit(
                menu,
                &rows,
                Point2D::new(label.origin.x + 20.0, label.origin.y + 5.0)
            ),
            AIChatHit::Inside
        );
    }
}
