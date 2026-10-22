//! Session row of [`super::ai_chat_panel::AIChatPlaceholder`] (#26):
//! a session select spanning the row and an outlined "New" button, plus
//! the maximize glyph between them while the chat floats. The select
//! opens `ai_chat_session_menu`, the list of this file's chats.
//!
//! [`header_layout`] is the single rect source paint and hit-test read.

use super::ai_chat_panel::{footer_label_width, HEADER_HEIGHT, PAD};
use crate::theme::Theme;
use crate::widgets::ai_chat_chrome::{chat_label, fit_label, label_baseline, outline_control};
use crate::widgets::home_surface::HomePalette;
use crate::widgets::icons::{draw_icon, Icon};
use crate::widgets::PaintCx;
use crate::{Point2D, Rect, TextLayout};
use de_editor_core::ChatHeaderButton;

/// Inset of the session row's controls from the panel edges.
pub(crate) const SESSION_ROW_PAD_X: f32 = 12.0;
/// Height of the session select and the New button.
pub(crate) const SESSION_CONTROL_H: f32 = 32.0;
/// Side of the maximize glyph's square hit target (floating only).
pub(crate) const MAXIMIZE_BOX: f32 = 28.0;
const MAXIMIZE_ICON: f32 = 16.0;
const CONTROL_GAP: f32 = 8.0;
const CONTROL_RADIUS: f32 = 7.0;
const LABEL_FONT: f32 = 13.0;
/// Title font of the composer-only card's slim header.
pub(crate) const TAB_FONT_SIZE: f32 = 12.0;
const SPINNER_D: f32 = 10.0;

/// Where the session row's controls sit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct HeaderLayout {
    pub(crate) select: Rect,
    pub(crate) new_chat: Rect,
    /// The maximize glyph; `None` when the chat is pinned into a column,
    /// which has no window to maximize.
    pub(crate) maximize: Option<Rect>,
}

/// Lay the session row out inside the panel `rect`. `new_label` is the
/// localised "New", whose width sizes the button.
pub(crate) fn header_layout(rect: Rect, pinned: bool, new_label: &str) -> HeaderLayout {
    let y = rect.origin.y + (HEADER_HEIGHT - SESSION_CONTROL_H) / 2.0;
    let left = rect.origin.x + SESSION_ROW_PAD_X;
    let right = rect.origin.x + rect.size.x - SESSION_ROW_PAD_X;
    let new_w = 10.0 + 14.0 + 6.0 + footer_label_width(new_label, LABEL_FONT) + 12.0;
    let new_chat = Rect::xywh(right - new_w, y, new_w, SESSION_CONTROL_H);
    let mut select_right = new_chat.origin.x - CONTROL_GAP;
    let maximize = (!pinned).then(|| {
        let r = Rect::xywh(
            select_right - MAXIMIZE_BOX,
            y + (SESSION_CONTROL_H - MAXIMIZE_BOX) / 2.0,
            MAXIMIZE_BOX,
            MAXIMIZE_BOX,
        );
        select_right = r.origin.x - 4.0;
        r
    });
    let select = Rect::xywh(left, y, (select_right - left).max(40.0), SESSION_CONTROL_H);
    HeaderLayout {
        select,
        new_chat,
        maximize,
    }
}

/// What the session row paints for one frame.
pub(crate) struct SessionHeader<'a> {
    pub(crate) title: &'a str,
    pub(crate) new_label: &'a str,
    pub(crate) menu_open: bool,
    pub(crate) running: bool,
    pub(crate) maximize_icon: Icon,
    pub(crate) hover: Option<ChatHeaderButton>,
    pub(crate) pressed: Option<ChatHeaderButton>,
    pub(crate) now_ms: u64,
}

/// Paint the session row and the hairline under it.
pub(crate) fn paint_session_header(
    cx: &mut PaintCx<'_>,
    palette: &HomePalette,
    rect: Rect,
    layout: &HeaderLayout,
    header: &SessionHeader<'_>,
) {
    let active = |button| header.hover == Some(button) || header.pressed == Some(button);
    cx.backend.fill_rect(
        Rect::xywh(
            rect.origin.x,
            rect.origin.y + HEADER_HEIGHT - 1.0,
            rect.size.x,
            1.0,
        ),
        palette.border,
    );

    // Session select: the active chat's title and a chevron.
    let select = layout.select;
    outline_control(
        cx,
        palette,
        select,
        CONTROL_RADIUS,
        header.menu_open || active(ChatHeaderButton::SessionMenu),
    );
    let chevron_x = select.origin.x + select.size.x - 10.0 - 14.0;
    let mut text_right = chevron_x - 8.0;
    if header.running {
        let cx_ = text_right - SPINNER_D / 2.0;
        let cy = select.origin.y + select.size.y / 2.0;
        paint_spinner(cx, palette, Point2D::new(cx_, cy), header.now_ms);
        text_right -= SPINNER_D + 6.0;
    }
    let title_x = select.origin.x + 10.0;
    let title = fit_label(
        cx,
        header.title,
        LABEL_FONT,
        (text_right - title_x).max(0.0),
    );
    chat_label(
        cx,
        &title,
        LABEL_FONT,
        palette.text,
        400,
        Point2D::new(title_x, label_baseline(select, LABEL_FONT)),
    );
    draw_icon(
        cx.backend,
        if header.menu_open {
            Icon::ChevronUp
        } else {
            Icon::ChevronDown
        },
        Point2D::new(chevron_x, select.origin.y + (select.size.y - 14.0) / 2.0),
        14.0,
        palette.text_3,
        1.6,
    );

    // Maximize / restore (floating only).
    if let Some(maximize) = layout.maximize {
        if active(ChatHeaderButton::ToggleMaximize) {
            cx.backend.fill_round_rect(maximize, 6.0, palette.subtle);
        }
        draw_icon(
            cx.backend,
            header.maximize_icon,
            Point2D::new(
                maximize.origin.x + (MAXIMIZE_BOX - MAXIMIZE_ICON) / 2.0,
                maximize.origin.y + (MAXIMIZE_BOX - MAXIMIZE_ICON) / 2.0,
            ),
            MAXIMIZE_ICON,
            palette.text_2,
            1.5,
        );
    }

    // New chat.
    let new_chat = layout.new_chat;
    outline_control(
        cx,
        palette,
        new_chat,
        CONTROL_RADIUS,
        active(ChatHeaderButton::NewChat),
    );
    draw_icon(
        cx.backend,
        Icon::Plus,
        Point2D::new(
            new_chat.origin.x + 10.0,
            new_chat.origin.y + (new_chat.size.y - 14.0) / 2.0,
        ),
        14.0,
        palette.text,
        1.6,
    );
    cx.backend.save();
    cx.backend.clip_rect(new_chat);
    chat_label(
        cx,
        header.new_label,
        LABEL_FONT,
        palette.text,
        400,
        Point2D::new(
            new_chat.origin.x + 30.0,
            label_baseline(new_chat, LABEL_FONT),
        ),
    );
    cx.backend.restore();
}

/// A small rotating arc: the session's agent is running.
pub(crate) fn paint_spinner(
    cx: &mut PaintCx<'_>,
    palette: &HomePalette,
    centre: Point2D,
    now_ms: u64,
) {
    let r = SPINNER_D / 2.0;
    let angle = (now_ms % 1000) as f32 / 1000.0 * std::f32::consts::TAU;
    cx.backend.stroke_oval(
        Rect::xywh(centre.x - r, centre.y - r, SPINNER_D, SPINNER_D),
        palette.border,
        1.4,
    );
    cx.backend.stroke_line(
        centre,
        Point2D::new(centre.x + angle.cos() * r, centre.y + angle.sin() * r),
        palette.accent,
        1.5,
    );
}

/// Paint the composer-only card's slim header: the session name, a
/// minimize glyph and an expand glyph. Both actions take the user to the
/// rail's Agent tab, which is where the conversation lives; the header
/// exists so that destination is visible before you commit to it.
pub(crate) fn paint_composer_header(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    rect: Rect,
    title: &str,
    hover: Option<de_editor_core::ChatHeaderButton>,
    pressed: Option<de_editor_core::ChatHeaderButton>,
) {
    use crate::widgets::ai_chat_panel::COMPOSER_HEADER_HEIGHT;
    let row = Rect {
        origin: rect.origin,
        size: Point2D::new(rect.size.x, COMPOSER_HEADER_HEIGHT),
    };
    let label_x = rect.origin.x + PAD;
    let baseline = row.origin.y + row.size.y / 2.0 + TAB_FONT_SIZE / 2.0 - 1.5;
    cx.backend.draw_text(
        &TextLayout::single_run(
            title,
            "system-ui",
            TAB_FONT_SIZE,
            theme.muted_foreground.to_jian(),
            Point2D::ZERO,
        )
        .with_font_weight(500),
        Point2D::new(label_x, baseline),
    );
    let glyph = 14.0_f32;
    let right = rect.origin.x + rect.size.x - PAD;
    for (index, (icon, button)) in [
        (
            Icon::Minimize,
            de_editor_core::ChatHeaderButton::ToggleCollapse,
        ),
        (
            Icon::Maximize,
            de_editor_core::ChatHeaderButton::ToggleMaximize,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let x = right - glyph - (1 - index) as f32 * (glyph + 10.0);
        let color = if hover == Some(button) || pressed == Some(button) {
            theme.foreground
        } else {
            theme.muted_foreground
        };
        draw_icon(
            cx.backend,
            icon,
            Point2D::new(x, row.origin.y + (row.size.y - glyph) / 2.0),
            glyph,
            color,
            1.4,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rail() -> Rect {
        Rect::xywh(0.0, 0.0, 320.0, 700.0)
    }

    #[test]
    fn pinned_row_is_select_then_new_with_no_maximize() {
        let l = header_layout(rail(), true, "New");
        assert!(l.maximize.is_none());
        assert_eq!(l.select.origin.x, SESSION_ROW_PAD_X);
        assert!(l.select.origin.x + l.select.size.x <= l.new_chat.origin.x - CONTROL_GAP + 0.01);
        assert!(
            (l.new_chat.origin.x + l.new_chat.size.x - (320.0 - SESSION_ROW_PAD_X)).abs() < 0.01
        );
        assert_eq!(l.select.size.y, SESSION_CONTROL_H);
        // Both controls sit inside the header band.
        assert!(l.select.origin.y >= 0.0 && l.select.origin.y + l.select.size.y <= HEADER_HEIGHT);
    }

    #[test]
    fn floating_row_puts_maximize_between_select_and_new() {
        let l = header_layout(rail(), false, "New");
        let m = l.maximize.expect("floating chats can maximize");
        assert!(l.select.origin.x + l.select.size.x <= m.origin.x);
        assert!(m.origin.x + m.size.x <= l.new_chat.origin.x);
    }

    #[test]
    fn a_longer_new_label_widens_the_button_and_narrows_the_select() {
        let short = header_layout(rail(), true, "New");
        let long = header_layout(rail(), true, "Nouvelle discussion");
        assert!(long.new_chat.size.x > short.new_chat.size.x);
        assert!(long.select.size.x < short.select.size.x);
    }

    #[test]
    fn the_select_never_collapses_in_a_narrow_panel() {
        let l = header_layout(
            Rect::xywh(0.0, 0.0, 120.0, 400.0),
            false,
            "Nouvelle discussion",
        );
        assert!(l.select.size.x >= 40.0);
    }
}
