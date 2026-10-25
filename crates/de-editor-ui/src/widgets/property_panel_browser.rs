//! Browser section (#20): the page address, the device preset (Desktop /
//! Tablet / Phone), the page zoom and the page scroll. Visible when the
//! selection is a browser node (`VisibleSections.browser`). Paint and both
//! layout walkers step through the same rows, like the Annotation section.

use jian_ops_schema::node::PenNode;

use crate::theme::Theme;
use crate::widgets::property_panel::{NodeSnapshot, PropertyPanelAction};
use crate::widgets::property_panel_inputs::{
    paint_input_with_prefix_focused_state, paint_section_divider, paint_section_label,
    INPUT_HEIGHT, PAD_X, SECTION_GAP, SECTION_HEADER_HEIGHT,
};
use crate::widgets::property_panel_sections::EditContext;
use crate::widgets::property_panel_segments::{paint_segment_row, segment_rects};
use crate::widgets::PaintCx;
use crate::{Point2D, Rect};
use de_editor_core::PropertyFocus;

/// One row's vertical advance (input + gap).
const ROW_ADVANCE: f32 = INPUT_HEIGHT + 6.0;
/// Space under the last row, above the divider.
const BOTTOM_PAD: f32 = 12.0;
const SEGMENT_FONT: f32 = 12.0;
/// Gap between the Scroll X and Scroll Y halves.
const HALF_GAP: f32 = 8.0;
/// Rows under the header: url, device, zoom, scroll.
const ROWS: usize = 4;

/// The device presets the segment row offers, in order. The rest of
/// `de_editor_core::browser_devices` stays reachable through MCP and import.
pub const PANEL_DEVICES: [&str; 3] = ["desktop", "tablet", "phone"];

/// The selected browser's editable fields.
#[derive(Debug, Clone, PartialEq)]
pub struct BrowserSummary {
    pub url: String,
    pub device_id: Option<String>,
    /// Page zoom in percent (100 = 1:1).
    pub zoom_percent: f64,
    pub scroll_x: f64,
    pub scroll_y: f64,
}

/// `Some` for a browser node.
pub fn browser_summary_of(node: &PenNode) -> Option<BrowserSummary> {
    let PenNode::Browser(b) = node else {
        return None;
    };
    Some(BrowserSummary {
        url: b.url.clone().unwrap_or_default(),
        device_id: b.device_id.clone(),
        zoom_percent: (b.zoom.unwrap_or(1.0) * 100.0).round(),
        scroll_x: b.scroll_x.unwrap_or(0.0),
        scroll_y: b.scroll_y.unwrap_or(0.0),
    })
}

/// Section height without the trailing gap (callers add `SECTION_GAP`).
pub fn browser_section_height() -> f32 {
    SECTION_HEADER_HEIGHT + ROW_ADVANCE * ROWS as f32 + BOTTOM_PAD
}

fn row_rect(x0: f32, top: f32, row: usize, usable_w: f32) -> Rect {
    Rect {
        origin: Point2D::new(
            x0 + PAD_X,
            top + SECTION_HEADER_HEIGHT + ROW_ADVANCE * row as f32,
        ),
        size: Point2D::new(usable_w, INPUT_HEIGHT),
    }
}

/// The Scroll X / Scroll Y halves of the last row.
fn scroll_rects(x0: f32, top: f32, usable_w: f32) -> [Rect; 2] {
    let row = row_rect(x0, top, 3, usable_w);
    let half = (usable_w - HALF_GAP) / 2.0;
    let left = Rect {
        origin: row.origin,
        size: Point2D::new(half, INPUT_HEIGHT),
    };
    let right = Rect {
        origin: Point2D::new(row.origin.x + half + HALF_GAP, row.origin.y),
        size: Point2D::new(half, INPUT_HEIGHT),
    };
    [left, right]
}

/// The device cells, one action each.
pub fn push_browser_action_rects(
    out: &mut Vec<(PropertyPanelAction, Rect)>,
    x0: f32,
    top: f32,
    usable_w: f32,
) {
    let cells = segment_rects(row_rect(x0, top, 1, usable_w), PANEL_DEVICES.len());
    for (id, rect) in PANEL_DEVICES.into_iter().zip(cells) {
        out.push((PropertyPanelAction::SetBrowserDevice(id), rect));
    }
}

/// The url, zoom and scroll input rects.
pub fn push_browser_input_rects(
    rects: &mut Vec<(PropertyFocus, Rect)>,
    x0: f32,
    top: f32,
    usable_w: f32,
) {
    rects.push((PropertyFocus::BrowserUrl, row_rect(x0, top, 0, usable_w)));
    rects.push((PropertyFocus::BrowserZoom, row_rect(x0, top, 2, usable_w)));
    let [sx, sy] = scroll_rects(x0, top, usable_w);
    rects.push((PropertyFocus::BrowserScrollX, sx));
    rects.push((PropertyFocus::BrowserScrollY, sy));
}

/// Paint the section. Returns the y below it, gap included.
#[allow(clippy::too_many_arguments)]
pub fn paint_browser_section(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    snapshot: &NodeSnapshot,
    edit: &EditContext<'_>,
    locale: de_editor_core::Locale,
    x: f32,
    y: f32,
    width: f32,
) -> f32 {
    let Some(summary) = snapshot.browser.as_ref() else {
        return y;
    };
    let usable_w = width - PAD_X * 2.0;
    let top = y;
    paint_section_label(cx, theme, label(locale, "browser.title"), x, y, width);
    let zoom = format_number(summary.zoom_percent);
    let scroll_x = format_number(summary.scroll_x);
    let scroll_y = format_number(summary.scroll_y);
    let [sx, sy] = scroll_rects(x, top, usable_w);
    let inputs = [
        (
            PropertyFocus::BrowserUrl,
            row_rect(x, top, 0, usable_w),
            label(locale, "browser.url"),
            summary.url.as_str(),
        ),
        (
            PropertyFocus::BrowserZoom,
            row_rect(x, top, 2, usable_w),
            label(locale, "browser.zoom"),
            zoom.as_str(),
        ),
        (
            PropertyFocus::BrowserScrollX,
            sx,
            label(locale, "browser.scrollX"),
            scroll_x.as_str(),
        ),
        (
            PropertyFocus::BrowserScrollY,
            sy,
            label(locale, "browser.scrollY"),
            scroll_y.as_str(),
        ),
    ];
    for (focus, rect, prefix, fallback) in inputs {
        paint_input_with_prefix_focused_state(
            cx,
            theme,
            rect,
            prefix,
            edit.value_for(focus, fallback),
            edit.focus == Some(focus),
            edit.caret_at(focus),
            edit.select_all_at(focus),
            edit.input_at(focus),
            edit.now_ms,
        );
    }
    let labels: Vec<&str> = PANEL_DEVICES
        .iter()
        .map(|id| label(locale, device_key(id)))
        .collect();
    let active = summary
        .device_id
        .as_deref()
        .and_then(|id| PANEL_DEVICES.iter().position(|d| *d == id));
    paint_segment_row(
        cx,
        theme,
        row_rect(x, top, 1, usable_w),
        &labels,
        active,
        SEGMENT_FONT,
    );
    let y = top + SECTION_HEADER_HEIGHT + ROW_ADVANCE * ROWS as f32 + BOTTOM_PAD;
    paint_section_divider(cx, theme, x, y, width);
    y + SECTION_GAP
}

/// Whole numbers without a trailing `.0`.
fn format_number(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{}", value as i64)
    } else {
        format!("{value:.2}")
    }
}

fn device_key(id: &str) -> &'static str {
    de_editor_core::browser_devices::browser_device(id)
        .map_or("browser.device.desktop", |device| device.label_key)
}

fn label(locale: de_editor_core::Locale, key: &'static str) -> &'static str {
    crate::i18n::translate(locale, key)
}

#[cfg(test)]
#[path = "property_panel_browser_tests.rs"]
mod tests;
