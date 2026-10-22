//! The session select's dropdown (#26): one row per chat in this file,
//! the active one ticked, a close glyph on the hovered row. Rects here are
//! the single source paint and hit-test read.

use crate::widgets::ai_chat_chrome::{chat_label, dropdown_card, fit_label, label_baseline};
use crate::widgets::home_surface::HomePalette;
use crate::widgets::icons::{draw_icon, Icon};
use crate::widgets::PaintCx;
use crate::{Point2D, Rect};

pub(crate) const SESSION_ROW_H: f32 = 32.0;
const MENU_PAD: f32 = 4.0;
const MENU_GAP: f32 = 4.0;
/// Rows shown at once; older chats beyond this stay reachable once newer
/// ones are closed.
const MAX_ROWS: usize = 8;
const GLYPH: f32 = 14.0;
const LABEL_FONT: f32 = 13.0;

/// Index of the first chat listed: the newest `MAX_ROWS` are shown.
pub(crate) fn first_visible(count: usize) -> usize {
    count.saturating_sub(MAX_ROWS)
}

/// The dropdown hanging under the session select.
pub(crate) fn session_menu_rect(select: Rect, count: usize) -> Rect {
    let rows = (count - first_visible(count)).max(1) as f32;
    Rect::xywh(
        select.origin.x,
        select.origin.y + select.size.y + MENU_GAP,
        select.size.x,
        rows * SESSION_ROW_H + MENU_PAD * 2.0,
    )
}

/// The row listing chat `index`, or `None` when it is not shown.
pub(crate) fn session_row_rect(menu: Rect, count: usize, index: usize) -> Option<Rect> {
    let first = first_visible(count);
    if index < first || index >= count {
        return None;
    }
    Some(Rect::xywh(
        menu.origin.x + MENU_PAD,
        menu.origin.y + MENU_PAD + (index - first) as f32 * SESSION_ROW_H,
        menu.size.x - MENU_PAD * 2.0,
        SESSION_ROW_H,
    ))
}

/// The close glyph's target at the right of a row.
pub(crate) fn session_close_rect(row: Rect) -> Rect {
    Rect::xywh(
        row.origin.x + row.size.x - 8.0 - GLYPH - 4.0,
        row.origin.y + (row.size.y - GLYPH - 8.0) / 2.0,
        GLYPH + 8.0,
        GLYPH + 8.0,
    )
}

/// The chat row under `point` and whether it is on that row's close glyph
/// (live only on the hovered row, where it is painted). `None` off the rows.
pub(crate) fn session_menu_hit(
    menu: Rect,
    count: usize,
    point: Point2D,
    hover: Option<usize>,
) -> Option<(usize, bool)> {
    (first_visible(count)..count).find_map(|index| {
        let row = session_row_rect(menu, count, index)?;
        row.contains(point).then(|| {
            let close = hover == Some(index) && session_close_rect(row).contains(point);
            (index, close)
        })
    })
}

/// Paint the dropdown. `titles` lists every chat; `running` is the
/// active chat's agent state.
pub(crate) fn paint_session_menu(
    cx: &mut PaintCx<'_>,
    palette: &HomePalette,
    menu: Rect,
    titles: &[String],
    active: usize,
    hover: Option<usize>,
) {
    dropdown_card(cx, palette, menu);
    let count = titles.len();
    for (index, title) in titles.iter().enumerate() {
        let Some(row) = session_row_rect(menu, count, index) else {
            continue;
        };
        let hovered = hover == Some(index);
        if hovered {
            cx.backend.fill_round_rect(row, 6.0, palette.subtle);
        }
        let right_glyph_x = row.origin.x + row.size.x - 8.0 - GLYPH;
        let text_x = row.origin.x + 10.0;
        let label = fit_label(
            cx,
            title,
            LABEL_FONT,
            (right_glyph_x - 8.0 - text_x).max(0.0),
        );
        chat_label(
            cx,
            &label,
            LABEL_FONT,
            if index == active {
                palette.text
            } else {
                palette.text_2
            },
            if index == active { 600 } else { 400 },
            Point2D::new(text_x, label_baseline(row, LABEL_FONT)),
        );
        let glyph_at = Point2D::new(right_glyph_x, row.origin.y + (row.size.y - GLYPH) / 2.0);
        if hovered {
            draw_icon(
                cx.backend,
                Icon::Close,
                glyph_at,
                GLYPH,
                palette.text_2,
                1.5,
            );
        } else if index == active {
            draw_icon(
                cx.backend,
                Icon::Check,
                glyph_at,
                GLYPH,
                palette.accent,
                1.8,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn select() -> Rect {
        Rect::xywh(12.0, 8.0, 220.0, 32.0)
    }

    #[test]
    fn menu_hangs_under_the_select_at_its_width() {
        let menu = session_menu_rect(select(), 3);
        assert_eq!(menu.origin.x, 12.0);
        assert_eq!(menu.size.x, 220.0);
        assert!(menu.origin.y >= 40.0);
        assert_eq!(menu.size.y, 3.0 * SESSION_ROW_H + MENU_PAD * 2.0);
    }

    #[test]
    fn rows_map_back_to_their_chat_and_close_needs_hover() {
        let menu = session_menu_rect(select(), 3);
        let row = session_row_rect(menu, 3, 1).unwrap();
        let centre = Point2D::new(row.origin.x + 20.0, row.origin.y + 16.0);
        assert_eq!(session_menu_hit(menu, 3, centre, None), Some((1, false)));
        let close = session_close_rect(row);
        let on_close = Point2D::new(close.origin.x + 8.0, close.origin.y + 8.0);
        assert_eq!(session_menu_hit(menu, 3, on_close, None), Some((1, false)));
        assert_eq!(
            session_menu_hit(menu, 3, on_close, Some(1)),
            Some((1, true))
        );
        assert_eq!(
            session_menu_hit(menu, 3, Point2D::new(0.0, 0.0), Some(1)),
            None
        );
    }

    #[test]
    fn a_long_history_lists_only_the_newest_chats() {
        let menu = session_menu_rect(select(), 12);
        assert_eq!(
            menu.size.y,
            MAX_ROWS as f32 * SESSION_ROW_H + MENU_PAD * 2.0
        );
        assert!(session_row_rect(menu, 12, 3).is_none());
        assert!(session_row_rect(menu, 12, 4).is_some());
        assert!(session_row_rect(menu, 12, 11).is_some());
    }
}
