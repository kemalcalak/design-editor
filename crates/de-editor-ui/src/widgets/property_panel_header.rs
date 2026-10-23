//! The right panel's pinned header (#30), replacing the Design / Code tab
//! strip. In Design it names the selection: an accent kind glyph, the
//! node's name, and its kind on the right. In the Code view (opened by
//! the canvas `</>` button or a Code chip) it reads "Code" with a close
//! button that returns to Design.
//!
//! Its band is [`PANEL_HEADER_HEIGHT`] tall, the same step every section
//! y-walk starts with, so paint and hit-test stay aligned.

use de_editor_core::PropertyTab;

use crate::theme::Theme;
use crate::widgets::icons::{draw_icon, Icon};
use crate::widgets::property_panel_inputs::{
    COMPONENT_ACCENT, INSTANCE_ACCENT, PAD_X, PANEL_HEADER_HEIGHT,
};
use crate::widgets::property_panel_snapshot::NodeSnapshot;
use crate::widgets::text_metrics;
use crate::widgets::PaintCx;
use crate::{Color, Point2D, Rect, TextLayout};

const ICON: f32 = 16.0;
const ICON_GAP: f32 = 8.0;
const TITLE_FONT: f32 = 14.0;
const KIND_FONT: f32 = 12.0;
const CLOSE: f32 = 24.0;

/// What the header shows.
pub enum PanelHeader<'a> {
    Node {
        icon: Icon,
        accent: Color,
        title: &'a str,
        kind: &'a str,
    },
    Code {
        title: &'a str,
    },
}

/// The header for a selection: the layer rail's glyph, tinted purple /
/// violet for components and instances (their diamond) and accent
/// otherwise.
pub fn node_header<'a>(theme: &Theme, snapshot: &'a NodeSnapshot) -> PanelHeader<'a> {
    let accent = if snapshot.is_reusable {
        COMPONENT_ACCENT
    } else if snapshot.is_instance {
        INSTANCE_ACCENT
    } else {
        theme.primary
    };
    let title = if snapshot.name.is_empty() {
        snapshot.kind.as_str()
    } else {
        snapshot.name.as_str()
    };
    PanelHeader::Node {
        icon: snapshot.kind_icon,
        accent,
        title,
        kind: snapshot.kind.as_str(),
    }
}

/// The band itself.
pub fn header_rect(panel_rect: Rect) -> Rect {
    Rect {
        origin: panel_rect.origin,
        size: Point2D::new(panel_rect.size.x, PANEL_HEADER_HEIGHT),
    }
}

/// The Code view's close button.
pub fn close_rect(panel_rect: Rect) -> Rect {
    Rect {
        origin: Point2D::new(
            panel_rect.origin.x + panel_rect.size.x - PAD_X + 4.0 - CLOSE,
            panel_rect.origin.y + (PANEL_HEADER_HEIGHT - CLOSE) / 2.0,
        ),
        size: Point2D::new(CLOSE, CLOSE),
    }
}

/// The tab a press on the header switches to: only the Code view's close
/// button does anything, and it goes back to Design.
pub fn header_hit(tab: PropertyTab, panel_rect: Rect, point: Point2D) -> Option<PropertyTab> {
    (tab == PropertyTab::Code && close_rect(panel_rect).contains(point))
        .then_some(PropertyTab::Design)
}

/// Paint the header band, its bottom rule included.
pub fn paint_panel_header(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    panel_rect: Rect,
    header: &PanelHeader<'_>,
    close_hovered: bool,
) {
    let band = header_rect(panel_rect);
    let x = band.origin.x + PAD_X;
    let right = band.origin.x + band.size.x - PAD_X;
    match header {
        PanelHeader::Node {
            icon,
            accent,
            title,
            kind,
        } => {
            draw_icon(
                cx.backend,
                *icon,
                Point2D::new(x, band.origin.y + (band.size.y - ICON) / 2.0),
                ICON,
                *accent,
                1.6,
            );
            let kind_w = text_metrics::measure_chrome(cx.backend, kind, KIND_FONT);
            text(
                cx,
                kind,
                right - kind_w,
                band,
                KIND_FONT,
                400,
                theme.muted_subtle,
            );
            let title_x = x + ICON + ICON_GAP;
            let title = text_metrics::fit_chrome(
                cx.backend,
                title,
                (right - kind_w - 12.0 - title_x).max(0.0),
                TITLE_FONT,
            );
            text(cx, &title, title_x, band, TITLE_FONT, 600, theme.foreground);
        }
        PanelHeader::Code { title } => {
            text(cx, title, x, band, TITLE_FONT, 600, theme.foreground);
            let close = close_rect(panel_rect);
            if close_hovered {
                cx.backend.fill_round_rect(close, 6.0, theme.button_hover);
            }
            draw_icon(
                cx.backend,
                Icon::Close,
                Point2D::new(close.origin.x + 4.0, close.origin.y + 4.0),
                CLOSE - 8.0,
                if close_hovered {
                    theme.foreground
                } else {
                    theme.muted_foreground
                },
                1.6,
            );
        }
    }
    cx.backend.fill_rect(
        Rect {
            origin: Point2D::new(band.origin.x, band.origin.y + band.size.y - 1.0),
            size: Point2D::new(band.size.x, 1.0),
        },
        theme.border,
    );
}

fn text(cx: &mut PaintCx<'_>, s: &str, x: f32, band: Rect, size: f32, weight: u16, color: Color) {
    if s.is_empty() {
        return;
    }
    cx.backend.draw_text(
        &TextLayout::single_run(
            s,
            text_metrics::CHROME_FONT_FAMILY,
            size,
            color.to_jian(),
            Point2D::ZERO,
        )
        .with_font_weight(weight),
        Point2D::new(x, jian_widgets::centered_text_baseline_y(band, size)),
    );
}

#[cfg(test)]
#[path = "property_panel_header_tests.rs"]
mod property_panel_header_tests;
