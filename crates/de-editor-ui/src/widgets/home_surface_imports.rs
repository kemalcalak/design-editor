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
use crate::widgets::line_break::break_point;
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

#[cfg(test)]
#[path = "home_surface_imports_tests.rs"]
mod tests;
