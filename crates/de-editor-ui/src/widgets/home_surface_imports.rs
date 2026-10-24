//! The four import cards under the composer: Figma, a saved web page,
//! recreating a screenshot, and a pen.dev design.

use super::copy::tr;
use super::layout::{
    IMPORT_H_TALL, IMPORT_TEXT_PAD_R, IMPORT_TEXT_X, IMPORT_TITLE_FONT, IMPORT_TITLE_KEYS,
};
use super::paint::{draw_text, draw_text_weighted, outline};
use super::{HomeLayout, HomeSurface, SANS};
use crate::widgets::brand_icons::paint_figma_logo;
use crate::widgets::icons::{draw_icon, Icon};
use crate::widgets::text_metrics::fit_in_family;
use crate::widgets::PaintCx;
use crate::{Point2D, Rect};
use de_editor_core::HomeHit;

const CARDS: [(HomeHit, &str); 4] = [
    (HomeHit::ImportFigma, "home.import.figmaSub"),
    (HomeHit::ImportWeb, "home.import.webSub"),
    (HomeHit::ImportScreenshot, "home.import.screenshotSub"),
    (HomeHit::ImportPenDev, "home.import.penDevSub"),
];

const TITLE_WEIGHT: u16 = 600;
const SUB_FONT: f32 = 12.0;
/// Baseline-to-baseline step between title lines.
const TITLE_LINE_H: f32 = 17.0;
/// Last title baseline to the subtitle baseline.
const SUB_STEP: f32 = 18.0;
/// The first baseline of a one-line title sits this far above the card's
/// vertical middle (29 px into the 64 px card).
const BLOCK_LIFT: f32 = 3.0;

/// The text box an import card's title and subtitle must stay inside.
pub(super) fn import_text_box(card: Rect) -> Rect {
    Rect::xywh(
        card.origin.x + IMPORT_TEXT_X,
        card.origin.y,
        (card.size.x - IMPORT_TEXT_X - IMPORT_TEXT_PAD_R).max(0.0),
        card.size.y,
    )
}

/// How many title lines a card of this height has room for.
fn title_lines_for(card: Rect) -> usize {
    if card.size.y >= IMPORT_H_TALL {
        2
    } else {
        1
    }
}

pub(super) fn paint_imports(surface: &HomeSurface<'_>, cx: &mut PaintCx<'_>, layout: &HomeLayout) {
    let p = surface.palette;
    for ((rect, (hit, sub)), title) in layout
        .imports
        .iter()
        .copied()
        .zip(CARDS)
        .zip(IMPORT_TITLE_KEYS)
    {
        let hovered = surface.state.hover == Some(hit);
        outline(cx, surface, rect, 10.0, hovered);
        let tile = Rect::xywh(
            rect.origin.x + 16.0,
            rect.origin.y + (rect.size.y - 34.0) / 2.0,
            34.0,
            34.0,
        );
        cx.backend.fill_round_rect(tile, 8.0, p.subtle);
        let glyph_at = Point2D::new(tile.origin.x + 9.0, tile.origin.y + 9.0);
        match hit {
            HomeHit::ImportFigma => paint_figma_logo(cx.backend, glyph_at, 16.0, p.text_2),
            HomeHit::ImportWeb => draw_icon(cx.backend, Icon::Globe, glyph_at, 16.0, p.text_2, 1.6),
            HomeHit::ImportPenDev => {
                draw_icon(cx.backend, Icon::PenTool, glyph_at, 16.0, p.text_2, 1.6)
            }
            _ => draw_icon(cx.backend, Icon::ImagePlus, glyph_at, 16.0, p.text_2, 1.6),
        }
        let text_box = import_text_box(rect);
        let lines = wrap_title(
            tr(surface.locale, title),
            text_box.size.x,
            title_lines_for(rect),
            |text| {
                cx.backend.measure_text_family_styled(
                    text,
                    IMPORT_TITLE_FONT,
                    SANS,
                    TITLE_WEIGHT,
                    false,
                )
            },
        );
        // Centre the title + subtitle block on the card, as the tile is.
        let extra = (lines.len().max(1) - 1) as f32 * TITLE_LINE_H;
        let mid = rect.origin.y + rect.size.y / 2.0;
        let mut y = mid - BLOCK_LIFT - extra / 2.0;
        for line in &lines {
            draw_text_weighted(
                cx,
                line,
                SANS,
                IMPORT_TITLE_FONT,
                p.text,
                TITLE_WEIGHT,
                Point2D::new(text_box.origin.x, y),
            );
            y += TITLE_LINE_H;
        }
        let sub = fit_in_family(
            cx.backend,
            tr(surface.locale, sub),
            text_box.size.x,
            SUB_FONT,
            SANS,
        );
        draw_text(
            cx,
            &sub,
            SANS,
            SUB_FONT,
            p.text_3,
            Point2D::new(text_box.origin.x, y - TITLE_LINE_H + SUB_STEP),
        );
    }
}

/// Wrap `text` into at most `max_lines` lines no wider than `max_w`,
/// breaking at spaces where it can and between characters where it
/// cannot (CJK and Thai carry no spaces). Text past the last line is
/// dropped and that line ends in `…`, so nothing is ever clipped.
pub(super) fn wrap_title(
    text: &str,
    max_w: f32,
    max_lines: usize,
    mut measure: impl FnMut(&str) -> f32,
) -> Vec<String> {
    let mut lines = Vec::new();
    let mut rest = text.trim();
    while !rest.is_empty() {
        if lines.len() + 1 == max_lines.max(1) || measure(rest) <= max_w {
            lines.push(crate::util::ellipsize_to_width(rest, max_w, &mut measure));
            break;
        }
        let end = break_point(rest, max_w, &mut measure);
        lines.push(rest[..end].trim_end().to_string());
        rest = rest[end..].trim_start();
    }
    lines
}

/// The byte index to end this line at: after the last space that still
/// fits, else after as many characters as fit (at least one cluster). A
/// character break never strands a combining mark (Thai vowels and tone
/// marks, Devanagari matras) away from its base.
fn break_point(text: &str, max_w: f32, measure: &mut impl FnMut(&str) -> f32) -> usize {
    let mut last_fit = None;
    let mut last_space = None;
    let mut chars = text.char_indices().peekable();
    while let Some((index, ch)) = chars.next() {
        let end = index + ch.len_utf8();
        // Everything before this character fits, so ending the line at
        // this space (dropped by the caller's trim) fits too.
        if ch == ' ' && index > 0 {
            last_space = Some(end);
        }
        if measure(&text[..end]) > max_w {
            break;
        }
        let next_joins = chars.peek().is_some_and(|&(_, next)| joins_previous(next));
        if !next_joins && ch != VIRAMA {
            last_fit = Some(end);
        }
    }
    last_space
        .or(last_fit)
        .unwrap_or_else(|| first_cluster_end(text))
}

const VIRAMA: char = '\u{094D}';

/// A mark drawn on the character before it, never at a line start.
fn joins_previous(ch: char) -> bool {
    matches!(ch as u32,
        0x0300..=0x036F
            | 0x0900..=0x0903
            | 0x093A..=0x093C
            | 0x093E..=0x094F
            | 0x0951..=0x0957
            | 0x0962..=0x0963
            | 0x0E31
            | 0x0E34..=0x0E3A
            | 0x0E47..=0x0E4E)
}

/// The end of the first character plus any marks that join it.
fn first_cluster_end(text: &str) -> usize {
    let mut chars = text.char_indices();
    let mut end = chars.next().map_or(text.len(), |(_, ch)| ch.len_utf8());
    for (index, ch) in chars {
        if !joins_previous(ch) {
            break;
        }
        end = index + ch.len_utf8();
    }
    end
}

#[cfg(test)]
#[path = "home_surface_imports_tests.rs"]
mod tests;
