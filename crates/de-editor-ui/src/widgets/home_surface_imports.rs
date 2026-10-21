//! The three import cards under the composer: Figma, a saved web page,
//! and recreating a screenshot.

use super::copy::tr;
use super::paint::{draw_text, draw_text_weighted, outline};
use super::{HomeLayout, HomeSurface, SANS};
use crate::widgets::brand_icons::paint_figma_logo;
use crate::widgets::icons::{draw_icon, Icon};
use crate::widgets::PaintCx;
use crate::{Point2D, Rect};
use de_editor_core::HomeHit;

const CARDS: [(HomeHit, &str, &str); 3] = [
    (
        HomeHit::ImportFigma,
        "home.import.figma",
        "home.import.figmaSub",
    ),
    (HomeHit::ImportWeb, "home.import.web", "home.import.webSub"),
    (
        HomeHit::ImportScreenshot,
        "home.import.screenshot",
        "home.import.screenshotSub",
    ),
];

pub(super) fn paint_imports(surface: &HomeSurface<'_>, cx: &mut PaintCx<'_>, layout: &HomeLayout) {
    let p = surface.palette;
    for (rect, (hit, title, sub)) in layout.imports.iter().copied().zip(CARDS) {
        let hovered = surface.state.hover == Some(hit);
        outline(cx, surface, rect, 10.0, hovered);
        let tile = Rect::xywh(rect.origin.x + 16.0, rect.origin.y + 15.0, 34.0, 34.0);
        cx.backend.fill_round_rect(tile, 8.0, p.subtle);
        let glyph_at = Point2D::new(tile.origin.x + 9.0, tile.origin.y + 9.0);
        match hit {
            HomeHit::ImportFigma => paint_figma_logo(cx.backend, glyph_at, 16.0, p.text_2),
            HomeHit::ImportWeb => draw_icon(cx.backend, Icon::Globe, glyph_at, 16.0, p.text_2, 1.6),
            _ => draw_icon(cx.backend, Icon::ImagePlus, glyph_at, 16.0, p.text_2, 1.6),
        }
        let x = rect.origin.x + 62.0;
        cx.backend.save();
        cx.backend.clip_rect(Rect::xywh(
            x,
            rect.origin.y,
            rect.size.x - 70.0,
            rect.size.y,
        ));
        draw_text_weighted(
            cx,
            tr(surface.locale, title),
            SANS,
            13.0,
            p.text,
            600,
            Point2D::new(x, rect.origin.y + 29.0),
        );
        draw_text(
            cx,
            tr(surface.locale, sub),
            SANS,
            11.5,
            p.text_3,
            Point2D::new(x, rect.origin.y + 47.0),
        );
        cx.backend.restore();
    }
}
