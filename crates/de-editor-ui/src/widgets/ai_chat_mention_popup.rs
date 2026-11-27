//! The chat composer's `@` / `/` popup (#398): ranked pages, layers,
//! components, variables and project files after `@`, user skills after
//! `/`, opening upward from the composer card while the caret ends the
//! token.
//!
//! The rows come from `de_editor_core::chat_mentions` (resolved once per
//! query change, not per frame); this module owns only the geometry that
//! paint, hit-test and hover share, and the paint itself. Labels are
//! ellipsized through `text_metrics` in the family they are painted in.

use super::ai_chat_panel::AIChatPlaceholder;
use crate::widgets::ai_chat_chrome::{chat_label, dropdown_card, fit_label, label_baseline};
use crate::widgets::home_surface::HomePalette;
use crate::widgets::icons::{draw_icon, Icon};
use crate::widgets::{text_metrics, PaintCx};
use crate::{Point2D, Rect};
use de_editor_core::chat_mentions::{MentionKind, MentionPopup};
use de_editor_core::Locale;

const ROW_H: f32 = 32.0;
const PAD: f32 = 4.0;
const GAP: f32 = 6.0;
/// Widest the popup grows; narrower panels shrink it to the card.
const MAX_W: f32 = 360.0;
const GLYPH: f32 = 14.0;
const LABEL_FONT: f32 = 13.0;
const DETAIL_FONT: f32 = 11.5;
const TAG_FONT: f32 = 11.0;

/// The kind glyph shown at the start of a row.
pub(crate) fn kind_icon(kind: MentionKind) -> Icon {
    match kind {
        MentionKind::Page => Icon::Layers,
        MentionKind::Node => Icon::Frame,
        MentionKind::Component => Icon::Component,
        MentionKind::Variable => Icon::Braces,
        MentionKind::File => Icon::FileText,
        MentionKind::Skill => Icon::BookOpen,
    }
}

/// The popup for `rows` rows (an empty result still shows one "No matches"
/// row), its bottom edge `GAP` above the composer card.
pub(crate) fn mention_popup_rect(card: Rect, rows: usize) -> Rect {
    let h = rows.max(1) as f32 * ROW_H + PAD * 2.0;
    let w = (card.size.x - 8.0).clamp(0.0, MAX_W);
    Rect::xywh(card.origin.x + 4.0, card.origin.y - GAP - h, w, h)
}

fn row_rect(menu: Rect, row: usize) -> Rect {
    Rect::xywh(
        menu.origin.x + PAD,
        menu.origin.y + PAD + row as f32 * ROW_H,
        menu.size.x - PAD * 2.0,
        ROW_H,
    )
}

/// The row under `point`, or `None` off the rows.
pub(crate) fn mention_row_at(menu: Rect, rows: usize, point: Point2D) -> Option<usize> {
    (0..rows).find(|row| row_rect(menu, *row).contains(point))
}

impl<'a> AIChatPlaceholder<'a> {
    /// The open popup's rect, or `None` when no `@query` is being typed.
    pub(crate) fn mention_popup_bounds(&self, rect: Rect) -> Option<Rect> {
        let popup = self.state.mention_popup.as_ref()?;
        if self.state.is_minimized() && !self.composer_only {
            return None;
        }
        Some(mention_popup_rect(
            self.composer_card(rect),
            popup.rows.len(),
        ))
    }

    /// The popup row under the cursor — hosts copy it into the popup's
    /// highlight so hover and the keyboard share one marker.
    pub fn mention_hover_at(&self, rect: Rect, point: Point2D) -> Option<usize> {
        let popup = self.state.mention_popup.as_ref()?;
        mention_row_at(self.mention_popup_bounds(rect)?, popup.rows.len(), point)
    }
}

/// Paint the popup: kind glyph, label, a muted detail after it and the
/// localised kind tag on the right; the highlighted row gets the wash.
pub(crate) fn paint_mention_popup(
    cx: &mut PaintCx<'_>,
    palette: &HomePalette,
    menu: Rect,
    popup: &MentionPopup,
    locale: Locale,
) {
    dropdown_card(cx, palette, menu);
    if popup.rows.is_empty() {
        let row = row_rect(menu, 0);
        chat_label(
            cx,
            // An empty `/` list says where skills come from.
            de_i18n::translate(
                locale,
                if popup.trigger == de_editor_core::chat_mentions::SKILL_TRIGGER {
                    "chat.mention.noSkills"
                } else {
                    "chat.mention.noMatches"
                },
            ),
            LABEL_FONT,
            palette.text_3,
            400,
            Point2D::new(row.origin.x + 10.0, label_baseline(row, LABEL_FONT)),
        );
        return;
    }
    for (index, candidate) in popup.rows.iter().enumerate() {
        let row = row_rect(menu, index);
        if index == popup.highlight {
            cx.backend.fill_round_rect(row, 6.0, palette.subtle);
        }
        draw_icon(
            cx.backend,
            kind_icon(candidate.mention.kind),
            Point2D::new(
                row.origin.x + 10.0,
                row.origin.y + (row.size.y - GLYPH) / 2.0,
            ),
            GLYPH,
            palette.text_2,
            1.5,
        );
        let right = row.origin.x + row.size.x - 10.0;
        let tag_key = candidate
            .tag_key
            .unwrap_or_else(|| candidate.mention.kind.label_key());
        let tag = de_i18n::translate(locale, tag_key);
        let tag_w = text_metrics::measure_chrome(cx.backend, tag, TAG_FONT);
        let tag_x = right - tag_w;
        chat_label(
            cx,
            tag,
            TAG_FONT,
            palette.text_3,
            400,
            Point2D::new(tag_x, label_baseline(row, TAG_FONT)),
        );
        let text_x = row.origin.x + 10.0 + GLYPH + 10.0;
        let room = (tag_x - 12.0 - text_x).max(0.0);
        // The label takes the room it needs; the detail gets what is left.
        let label = fit_label(cx, &candidate.mention.label, LABEL_FONT, room);
        let label_w = text_metrics::measure_chrome(cx.backend, &label, LABEL_FONT);
        chat_label(
            cx,
            &label,
            LABEL_FONT,
            palette.text,
            if index == popup.highlight { 500 } else { 400 },
            Point2D::new(text_x, label_baseline(row, LABEL_FONT)),
        );
        let detail_x = text_x + label_w + 8.0;
        let detail_room = tag_x - 12.0 - detail_x;
        if !candidate.detail.is_empty() && detail_room > 24.0 {
            let detail = fit_label(cx, &candidate.detail, DETAIL_FONT, detail_room);
            chat_label(
                cx,
                &detail,
                DETAIL_FONT,
                palette.text_3,
                400,
                Point2D::new(detail_x, label_baseline(row, DETAIL_FONT)),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn popup_opens_above_the_card_and_fits_inside_it() {
        let card = Rect::xywh(20.0, 600.0, 300.0, 110.0);
        let menu = mention_popup_rect(card, 3);
        assert!(menu.origin.y + menu.size.y <= card.origin.y);
        assert!(menu.origin.x >= card.origin.x);
        assert!(menu.origin.x + menu.size.x <= card.origin.x + card.size.x);
        assert_eq!(menu.size.y, 3.0 * ROW_H + PAD * 2.0);
        // A wide card caps the popup width.
        assert_eq!(
            mention_popup_rect(Rect::xywh(0.0, 600.0, 900.0, 110.0), 1)
                .size
                .x,
            MAX_W
        );
        // No rows still reserves the "No matches" row.
        assert_eq!(mention_popup_rect(card, 0).size.y, ROW_H + PAD * 2.0);
    }

    #[test]
    fn rows_resolve_by_position() {
        let menu = mention_popup_rect(Rect::xywh(20.0, 600.0, 300.0, 110.0), 4);
        let mid = |i: usize| {
            let r = row_rect(menu, i);
            Point2D::new(r.origin.x + 30.0, r.origin.y + ROW_H / 2.0)
        };
        assert_eq!(mention_row_at(menu, 4, mid(0)), Some(0));
        assert_eq!(mention_row_at(menu, 4, mid(3)), Some(3));
        assert_eq!(mention_row_at(menu, 2, mid(3)), None);
        assert_eq!(mention_row_at(menu, 4, Point2D::new(0.0, 0.0)), None);
    }
}
