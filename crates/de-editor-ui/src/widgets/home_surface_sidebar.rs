//! The Home sidebar: brand, navigation, the TEMPLATES group, the MCP
//! status card and Settings. In a narrow window it is a 64 px icon rail.

use super::copy::tr;
use super::icons::{draw_glyph, HomeGlyph};
use super::paint::{baseline, caption, draw_text, draw_text_weighted};
use super::{HomeLayout, HomeSurface, MONO, SANS};
use crate::widgets::icons::Icon;
use crate::widgets::PaintCx;
use crate::{Point2D, Rect};
use de_editor_core::HomeHit;

const NAV: [(HomeHit, HomeGlyph, &str); 5] = [
    (
        HomeHit::NavRecents,
        HomeGlyph::Shared(Icon::History),
        "home.nav.recents",
    ),
    (
        HomeHit::NavOpenFile,
        HomeGlyph::Shared(Icon::FolderOpen),
        "home.nav.openFile",
    ),
    (
        HomeHit::NavDesignSystems,
        HomeGlyph::SwatchBook,
        "home.nav.designSystems",
    ),
    (
        HomeHit::NavSceneTemplates,
        HomeGlyph::LayoutTemplate,
        "home.nav.sceneTemplates",
    ),
    (
        HomeHit::NavSettings,
        HomeGlyph::Shared(Icon::Settings),
        "home.nav.settings",
    ),
];

pub(super) fn paint_sidebar(surface: &HomeSurface<'_>, cx: &mut PaintCx<'_>, layout: &HomeLayout) {
    let p = surface.palette;
    let side = layout.sidebar;
    cx.backend.fill_rect(side, p.surface);
    cx.backend.fill_rect(
        Rect::xywh(side.origin.x + side.size.x - 1.0, 0.0, 1.0, side.size.y),
        p.border,
    );

    // Brand: the accent mark, plus the wordmark beside it when there is
    // room for one.
    cx.backend.fill_round_rect(layout.brand_mark, 5.0, p.accent);
    if !layout.rail {
        draw_text_weighted(
            cx,
            "Design Editor",
            SANS,
            15.0,
            p.text,
            600,
            Point2D::new(
                layout.brand_mark.origin.x + 28.0,
                baseline(layout.brand_mark, 15.0),
            ),
        );
    }

    for (index, (hit, glyph, key)) in NAV.into_iter().enumerate() {
        let rect = layout.nav[index];
        // Recents is where Home already is.
        let selected = hit == HomeHit::NavRecents;
        let hovered = surface.state.hover == Some(hit);
        if selected || hovered {
            cx.backend.fill_round_rect(rect, 8.0, p.subtle);
        }
        let color = if selected { p.text } else { p.text_2 };
        let icon_x = if layout.rail {
            rect.origin.x + (rect.size.x - 16.0) / 2.0
        } else {
            rect.origin.x + 10.0
        };
        draw_glyph(
            cx.backend,
            glyph,
            Point2D::new(icon_x, rect.origin.y + 9.0),
            16.0,
            color,
            1.6,
        );
        if !layout.rail {
            draw_text_weighted(
                cx,
                tr(surface.locale, key),
                SANS,
                13.0,
                color,
                if selected { 600 } else { 400 },
                Point2D::new(rect.origin.x + 36.0, baseline(rect, 13.0)),
            );
        }
    }
    if layout.rail {
        cx.backend.fill_rect(layout.rail_divider, p.border);
    } else {
        let label = layout.templates_label;
        caption(
            cx,
            surface,
            tr(surface.locale, "home.nav.templates"),
            label.origin.x,
            label.origin.y + 31.0,
        );
    }
    paint_mcp_card(surface, cx, layout);
}

fn paint_mcp_card(surface: &HomeSurface<'_>, cx: &mut PaintCx<'_>, layout: &HomeLayout) {
    let p = surface.palette;
    let card = layout.mcp_card;
    let active = surface.state.mcp_popover_open || surface.state.hover == Some(HomeHit::McpCard);
    cx.backend
        .fill_round_rect(card, 9.0, if active { p.subtle } else { p.surface });
    cx.backend
        .stroke_round_rect(card, 9.0, if active { p.text_3 } else { p.border }, 1.0);
    let dot = if surface.mcp.running { p.ok } else { p.text_3 };
    if layout.rail {
        let d = 8.0;
        cx.backend.fill_oval(
            Rect::xywh(
                card.origin.x + (card.size.x - d) / 2.0,
                card.origin.y + (card.size.y - d) / 2.0,
                d,
                d,
            ),
            dot,
        );
        return;
    }
    let x = card.origin.x + 12.0;
    cx.backend
        .fill_oval(Rect::xywh(x, card.origin.y + 17.0, 7.0, 7.0), dot);
    let state_key = if surface.mcp.running {
        "home.mcp.running"
    } else {
        "home.mcp.stopped"
    };
    draw_text_weighted(
        cx,
        tr(surface.locale, state_key),
        SANS,
        12.5,
        p.text,
        600,
        Point2D::new(x + 14.0, card.origin.y + 25.0),
    );
    draw_text(
        cx,
        surface
            .mcp
            .endpoint
            .trim_start_matches("http://")
            .trim_start_matches("https://"),
        MONO,
        11.0,
        p.text_2,
        Point2D::new(x, card.origin.y + 46.0),
    );
    let agent = match &surface.mcp.client {
        Some(name) => tr(surface.locale, "home.mcp.connected").replace("{{name}}", name),
        None => tr(surface.locale, "home.mcp.noAgent").to_string(),
    };
    cx.backend.save();
    cx.backend.clip_rect(Rect::xywh(
        x,
        card.origin.y,
        card.size.x - 24.0,
        card.size.y,
    ));
    draw_text(
        cx,
        &agent,
        SANS,
        11.5,
        p.text_2,
        Point2D::new(x, card.origin.y + 67.0),
    );
    cx.backend.restore();
}
