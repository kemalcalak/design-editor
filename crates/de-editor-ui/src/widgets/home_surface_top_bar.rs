//! Home's top bar: the page title and the Open file / New file pair.
//! It is opaque so the column scrolls under it.

use super::copy::tr;
use super::paint::{baseline, draw_text_weighted, outline};
use super::{HomeLayout, HomeSurface, SANS};
use crate::widgets::icons::{draw_icon, Icon};
use crate::widgets::PaintCx;
use crate::Point2D;
use de_editor_core::HomeHit;

pub(super) fn paint_top_bar(surface: &HomeSurface<'_>, cx: &mut PaintCx<'_>, layout: &HomeLayout) {
    let p = surface.palette;
    let bar = layout.top_bar;
    cx.backend.fill_rect(bar, p.bg);
    draw_text_weighted(
        cx,
        tr(surface.locale, "home.title"),
        SANS,
        15.0,
        p.text,
        600,
        Point2D::new(bar.origin.x + 28.0, baseline(bar, 15.0)),
    );

    // Language: globe + short code + chevron, opening the shared picker.
    let locale = layout.locale_button;
    let active = surface.ui.locale_picker.open || surface.state.hover == Some(HomeHit::Locale);
    if active {
        outline(cx, surface, locale, 8.0, true);
    }
    let ink = if active { p.text } else { p.text_2 };
    draw_icon(
        cx.backend,
        Icon::Globe,
        Point2D::new(locale.origin.x + 10.0, locale.origin.y + 9.0),
        16.0,
        ink,
        1.6,
    );
    draw_text_weighted(
        cx,
        &super::layout::locale_code(surface.locale),
        SANS,
        12.0,
        ink,
        600,
        Point2D::new(locale.origin.x + 32.0, baseline(locale, 12.0)),
    );
    draw_icon(
        cx.backend,
        Icon::ChevronDown,
        Point2D::new(
            locale.origin.x + locale.size.x - 20.0,
            locale.origin.y + 11.0,
        ),
        12.0,
        p.text_3,
        1.6,
    );

    // Open file: a quiet text button that gains a wash on hover.
    let open = layout.open_file;
    let hovered = matches!(surface.state.hover, Some(HomeHit::OpenFile));
    if hovered {
        outline(cx, surface, open, 8.0, true);
    }
    draw_icon(
        cx.backend,
        Icon::FolderOpen,
        Point2D::new(open.origin.x + 12.0, open.origin.y + 9.0),
        16.0,
        p.text_2,
        1.6,
    );
    draw_text_weighted(
        cx,
        tr(surface.locale, "home.top.openFile"),
        SANS,
        13.0,
        if hovered { p.text } else { p.text_2 },
        500,
        Point2D::new(open.origin.x + 36.0, baseline(open, 13.0)),
    );

    // New file: the page's only accent-filled control.
    let new = layout.new_file;
    let pressed = surface.state.pressed == Some(HomeHit::NewFile);
    let hovered = surface.state.hover == Some(HomeHit::NewFile);
    let fill = if pressed || hovered {
        super::fade(p.accent, 0.88)
    } else {
        p.accent
    };
    cx.backend.fill_round_rect(new, 8.0, fill);
    draw_icon(
        cx.backend,
        Icon::Plus,
        Point2D::new(new.origin.x + 14.0, new.origin.y + 9.0),
        16.0,
        p.on_accent,
        1.8,
    );
    draw_text_weighted(
        cx,
        tr(surface.locale, "home.top.newFile"),
        SANS,
        13.0,
        p.on_accent,
        600,
        Point2D::new(new.origin.x + 38.0, baseline(new, 13.0)),
    );
}
