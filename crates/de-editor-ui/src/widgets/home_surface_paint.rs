//! The Home paint pass: the composition order plus the small text and
//! button helpers every Home section shares.

use super::{composer, imports, mcp_popover, recent, sidebar, top_bar, HomeSurface};
use crate::widgets::paint_window_control_dots;
use crate::widgets::PaintCx;
use crate::{Color, Point2D, Rect, TextLayout};

/// Paint the whole surface. Order is z-order: the scrolled column, the
/// pinned top bar over it, the sidebar, then the popovers.
pub(super) fn paint_home(surface: &HomeSurface<'_>, cx: &mut PaintCx<'_>, rect: Rect) {
    let p = surface.palette;
    let (w, h) = (rect.size.x, rect.size.y);
    let layout = surface.layout(w, h);
    cx.backend.fill_rect(rect, p.bg);

    cx.backend.save();
    cx.backend.clip_rect(layout.viewport_body);
    composer::paint_chips(surface, cx, &layout);
    composer::paint_composer(surface, cx, &layout);
    imports::paint_imports(surface, cx, &layout);
    recent::paint_recent(surface, cx, &layout);
    cx.backend.restore();

    top_bar::paint_top_bar(surface, cx, &layout);
    sidebar::paint_sidebar(surface, cx, &layout);
    if surface.state.size_menu_open {
        composer::paint_size_menu(surface, cx, &layout);
    }
    if surface.state.mcp_popover_open {
        let popover = surface.mcp_popover(&layout, w);
        mcp_popover::paint_mcp_popover(surface, cx, &popover);
    }
    // The borderless window's own close / minimise / maximise dots
    // (Windows / Linux; macOS keeps its native traffic lights there).
    paint_window_control_dots(
        cx.backend,
        Rect::xywh(0.0, 0.0, layout.sidebar.size.x, super::WINDOW_STRIP_H),
        surface.ui.topbar_traffic_hover,
    );
}

pub(super) fn draw_text(
    cx: &mut PaintCx<'_>,
    content: &str,
    family: &str,
    size: f32,
    color: Color,
    origin: Point2D,
) {
    draw_text_weighted(cx, content, family, size, color, 400, origin);
}

pub(super) fn draw_text_weighted(
    cx: &mut PaintCx<'_>,
    content: &str,
    family: &str,
    size: f32,
    color: Color,
    weight: u16,
    origin: Point2D,
) {
    if content.is_empty() {
        return;
    }
    let layout = TextLayout::single_run(content, family, size, color.to_jian(), Point2D::ZERO)
        .with_font_weight(weight);
    cx.backend.draw_text(&layout, origin);
}

/// The baseline that vertically centres `size`-px text in `rect`.
pub(super) fn baseline(rect: Rect, size: f32) -> f32 {
    jian_widgets::centered_text_baseline_y(rect, size)
}

/// An outlined control: `surface` fill, hairline border, `subtle` wash on
/// hover or press.
pub(super) fn outline(
    cx: &mut PaintCx<'_>,
    surface: &HomeSurface<'_>,
    rect: Rect,
    radius: f32,
    active: bool,
) {
    let p = surface.palette;
    cx.backend
        .fill_round_rect(rect, radius, if active { p.subtle } else { p.surface });
    cx.backend
        .stroke_round_rect(rect, radius, if active { p.text_3 } else { p.border }, 1.0);
}

/// A small UPPERCASE caption ("RECENT", "PAGE SIZE").
pub(super) fn caption(cx: &mut PaintCx<'_>, surface: &HomeSurface<'_>, text: &str, x: f32, y: f32) {
    draw_text_weighted(
        cx,
        &text.to_uppercase(),
        super::SANS,
        11.0,
        surface.palette.text_3,
        600,
        Point2D::new(x, y),
    );
}
