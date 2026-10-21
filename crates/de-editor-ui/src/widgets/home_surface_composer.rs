//! The kind chips, the prompt composer (input + toolbar) and the size
//! menu that opens under its size chip.

use super::copy::{
    kind_glyph, kind_label, placeholder, size_chip_label, size_glyph, size_name, tr,
};
use super::icons::draw_glyph;
use super::layout::{CHIP_FONT, SIZE_CHIP_FONT, SIZE_MENU_HEADER_H};
use super::model::paint_model_slot;
use super::paint::{baseline, caption, draw_text, draw_text_weighted, outline};
use super::{fade, HomeLayout, HomeSurface, MONO, SANS};
use crate::widgets::icons::{draw_icon, Icon};
use crate::widgets::PaintCx;
use crate::{Point2D, Rect, RenderBackend};
use de_editor_core::{HomeFamily, HomeHit};
use jian_widgets::components::text_area::TextArea;
use jian_widgets::Tokens;

const INPUT_FONT: f32 = 15.0;
/// jian's TextArea pads y by 4; the rect shifts so text sits 18 px in.
const INPUT_PAD_Y: f32 = 14.0;
const INPUT_PAD_X: f32 = 20.0;

pub(super) fn paint_chips(surface: &HomeSurface<'_>, cx: &mut PaintCx<'_>, layout: &HomeLayout) {
    let p = surface.palette;
    for (rect, family) in layout.chips.iter().copied().zip(HomeFamily::ALL) {
        let selected = surface.state.task == family;
        let hovered = surface.state.hover == Some(HomeHit::Kind(family));
        outline(cx, surface, rect, 8.0, selected || hovered);
        let color = if selected { p.text } else { p.text_2 };
        draw_glyph(
            cx.backend,
            kind_glyph(family),
            Point2D::new(rect.origin.x + 12.0, rect.origin.y + 9.0),
            16.0,
            color,
            1.6,
        );
        draw_text_weighted(
            cx,
            kind_label(surface.locale, family),
            SANS,
            CHIP_FONT,
            color,
            if selected { 600 } else { 500 },
            Point2D::new(rect.origin.x + 36.0, baseline(rect, CHIP_FONT)),
        );
    }
}

fn input_tokens(surface: &HomeSurface<'_>) -> Tokens {
    let p = surface.palette;
    let mut tokens = Tokens::light();
    tokens.foreground = p.text;
    tokens.muted_foreground = p.text_3;
    tokens.primary = p.accent;
    tokens.accent = fade(p.accent, 0.35);
    tokens
}

fn text_rect(input: Rect) -> Rect {
    Rect::xywh(
        input.origin.x,
        input.origin.y + INPUT_PAD_Y,
        input.size.x,
        input.size.y - INPUT_PAD_Y - 8.0,
    )
}

pub(super) fn paint_composer(surface: &HomeSurface<'_>, cx: &mut PaintCx<'_>, layout: &HomeLayout) {
    let p = surface.palette;
    let composer = layout.composer;
    // A soft accent halo under the card, as in the frames.
    cx.backend.fill_round_rect(
        Rect::xywh(
            composer.origin.x - 6.0,
            composer.origin.y + 10.0,
            composer.size.x + 12.0,
            composer.size.y + 6.0,
        ),
        16.0,
        fade(p.accent, 0.06),
    );
    cx.backend.fill_round_rect(composer, 12.0, p.surface);
    cx.backend.stroke_round_rect(composer, 12.0, p.border, 1.0);
    cx.backend.fill_rect(
        Rect::xywh(
            composer.origin.x + 1.0,
            layout.toolbar.origin.y,
            composer.size.x - 2.0,
            1.0,
        ),
        p.border,
    );

    let text_area = TextArea {
        state: &surface.state.input,
        placeholder: placeholder(surface.locale, surface.state.task),
        focused: surface.state.visible
            && !surface.state.mcp_popover_open
            && !surface.ui.agent_settings_open,
        font_size: INPUT_FONT,
        now_ms: surface.now_ms,
        pad_x: INPUT_PAD_X,
        max_visible_lines: 3,
    };
    // `draw_text` is baseline-relative but jian's TextArea positions glyph
    // runs from the line top, so every draw shifts down by the ascent —
    // the same wrapper the chat composer's input paints through.
    let mut backend = crate::widgets::text_input_backend::BaselineAdjustingBackend {
        inner: cx.backend,
        baseline_delta_y: INPUT_FONT,
    };
    backend.save();
    backend.clip_rect(layout.input_box);
    text_area.paint(
        &mut backend,
        text_rect(layout.input_box),
        &input_tokens(surface),
    );
    backend.restore();

    paint_toolbar(surface, cx, layout);
}

fn paint_toolbar(surface: &HomeSurface<'_>, cx: &mut PaintCx<'_>, layout: &HomeLayout) {
    let p = surface.palette;
    let hover = surface.state.hover;

    let attach = layout.attach;
    if hover == Some(HomeHit::Attach) {
        cx.backend.fill_round_rect(attach, 6.0, p.subtle);
    }
    draw_icon(
        cx.backend,
        Icon::Plus,
        Point2D::new(attach.origin.x + 5.0, attach.origin.y + 5.0),
        16.0,
        p.text_2,
        1.7,
    );

    let chip = layout.size_chip;
    let size = surface.state.task_draft().size;
    let active = surface.state.size_menu_open || hover == Some(HomeHit::SizeChip);
    cx.backend
        .fill_round_rect(chip, 6.0, if active { p.subtle } else { p.surface });
    cx.backend
        .stroke_round_rect(chip, 6.0, if active { p.text_3 } else { p.border }, 1.0);
    draw_glyph(
        cx.backend,
        size_glyph(size),
        Point2D::new(chip.origin.x + 8.0, chip.origin.y + 5.5),
        13.0,
        p.text_3,
        1.6,
    );
    draw_text(
        cx,
        &size_chip_label(surface.locale, size),
        SANS,
        SIZE_CHIP_FONT,
        p.text_2,
        Point2D::new(chip.origin.x + 27.0, baseline(chip, SIZE_CHIP_FONT)),
    );

    // Staged attachments, as chips after the size chip while they fit.
    let mut x = chip.origin.x + chip.size.x + 8.0;
    let limit = layout.model_chip.origin.x - 8.0;
    for name in &surface.attachment_names {
        let label =
            crate::widgets::text_metrics::fit_in_family(cx.backend, name, 140.0, 11.0, SANS);
        let w = cx.backend.measure_text_family(&label, 11.0, SANS) + 30.0;
        if x + w > limit {
            break;
        }
        let rect = Rect::xywh(x, chip.origin.y, w, chip.size.y);
        cx.backend.fill_round_rect(rect, 6.0, p.subtle);
        draw_icon(
            cx.backend,
            Icon::Image,
            Point2D::new(rect.origin.x + 7.0, rect.origin.y + 6.0),
            12.0,
            p.text_3,
            1.6,
        );
        draw_text(
            cx,
            &label,
            SANS,
            11.0,
            p.text_2,
            Point2D::new(rect.origin.x + 23.0, baseline(rect, 11.0)),
        );
        x += w + 6.0;
    }

    paint_model_slot(surface, cx, layout.model_chip);

    let send = layout.send;
    let enabled = surface.usable_agent && !surface.state.draft.trim().is_empty();
    let hovered = hover == Some(HomeHit::Send);
    let fill = if !surface.usable_agent {
        p.subtle
    } else if enabled && hovered {
        fade(p.accent, 0.88)
    } else if enabled {
        p.accent
    } else {
        fade(p.accent, 0.45)
    };
    cx.backend.fill_oval(send, fill);
    draw_icon(
        cx.backend,
        Icon::ArrowUp,
        Point2D::new(send.origin.x + 8.0, send.origin.y + 8.0),
        14.0,
        if surface.usable_agent {
            p.on_accent
        } else {
            p.text_3
        },
        2.0,
    );
}

pub(super) fn paint_size_menu(
    surface: &HomeSurface<'_>,
    cx: &mut PaintCx<'_>,
    layout: &HomeLayout,
) {
    let p = surface.palette;
    let menu = layout.size_menu;
    paint_popover_card(cx, surface, menu);
    let header_key = if surface.state.task == HomeFamily::Slides {
        "home.size.slideHeader"
    } else {
        "home.size.pageHeader"
    };
    caption(
        cx,
        surface,
        tr(surface.locale, header_key),
        menu.origin.x + 14.0,
        menu.origin.y + 6.0 + SIZE_MENU_HEADER_H - 8.0,
    );
    let current = surface.state.task_draft().size;
    for (rect, size) in layout
        .size_rows
        .iter()
        .copied()
        .zip(surface.state.task.sizes())
    {
        let selected = *size == current;
        let hovered = surface.state.hover == Some(HomeHit::SizeOption(*size));
        if selected || hovered {
            cx.backend.fill_round_rect(rect, 6.0, p.subtle);
        }
        let color = if selected { p.text } else { p.text_2 };
        draw_glyph(
            cx.backend,
            size_glyph(*size),
            Point2D::new(rect.origin.x + 8.0, rect.origin.y + 9.0),
            14.0,
            if selected { p.text } else { p.text_3 },
            1.6,
        );
        let name = size_name(surface.locale, *size);
        draw_text(
            cx,
            name,
            SANS,
            13.0,
            color,
            Point2D::new(rect.origin.x + 30.0, baseline(rect, 13.0)),
        );
        let name_w = cx.backend.measure_text_family(name, 13.0, SANS);
        draw_text(
            cx,
            size.detail(),
            MONO,
            11.0,
            p.text_3,
            Point2D::new(rect.origin.x + 38.0 + name_w, baseline(rect, 11.0)),
        );
        if selected {
            draw_icon(
                cx.backend,
                Icon::Check,
                Point2D::new(rect.origin.x + rect.size.x - 22.0, rect.origin.y + 9.0),
                14.0,
                p.accent,
                2.0,
            );
        }
    }
}

/// The floating card every Home popover sits on: a soft drop shadow,
/// `surface` fill and a hairline.
pub(super) fn paint_popover_card(cx: &mut PaintCx<'_>, surface: &HomeSurface<'_>, rect: Rect) {
    let p = surface.palette;
    for (spread, alpha) in [(10.0, 0.25), (5.0, 0.45), (2.0, 0.7)] {
        cx.backend.fill_round_rect(
            Rect::xywh(
                rect.origin.x - spread / 2.0,
                rect.origin.y + spread / 2.0,
                rect.size.x + spread,
                rect.size.y + spread / 2.0,
            ),
            10.0 + spread / 2.0,
            fade(p.shadow, alpha * 0.35),
        );
    }
    cx.backend.fill_round_rect(rect, 10.0, p.surface);
    cx.backend.stroke_round_rect(rect, 10.0, p.border, 1.0);
}

/// The caret rect for the platform IME panel: the start of the input's
/// first line, nudged right by an estimate of the text before the caret.
pub(super) fn caret_rect(surface: &HomeSurface<'_>, layout: &HomeLayout) -> Rect {
    let input = layout.input_box;
    let text = surface.state.input.text();
    let caret = jian_core::text_input::prev_char_boundary(
        text,
        surface.state.input.caret().min(text.len()),
    );
    let before = super::copy::estimate_width(&text[..caret], INPUT_FONT);
    let x = input.origin.x + INPUT_PAD_X + before.min(input.size.x - 2.0 * INPUT_PAD_X);
    Rect::xywh(
        x,
        input.origin.y + INPUT_PAD_Y + 4.0,
        1.5,
        INPUT_FONT * 1.35,
    )
}
