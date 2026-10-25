//! Browser section (#20): the page address, the device preset (Desktop /
//! Tablet / Phone), the page zoom, the page scroll and the live-page row
//! (Reload with the load status, or Load pages while an opened document's
//! pages are held). Visible when the
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
use crate::{Point2D, Rect, TextLayout};
use de_editor_core::browser_live::{BrowserLiveState, BrowserPageStatus};
use de_editor_core::PropertyFocus;

/// One row's vertical advance (input + gap).
const ROW_ADVANCE: f32 = INPUT_HEIGHT + 6.0;
/// Space under the last row, above the divider.
const BOTTOM_PAD: f32 = 12.0;
const SEGMENT_FONT: f32 = 12.0;
/// Gap between the Scroll X and Scroll Y halves.
const HALF_GAP: f32 = 8.0;
/// Rows under the header: url, device, zoom, scroll, live page.
const ROWS: usize = 5;
/// Share of the live-page row the Reload button takes.
const RELOAD_SHARE: f32 = 0.4;

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
    /// Whether pages may load (see `de_editor_core::browser_live`).
    pub pages_allowed: bool,
    pub status: Option<BrowserPageStatus>,
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
        pages_allowed: true,
        status: None,
    })
}

/// Fill in the session's live-page state (the panel builder has it).
pub fn apply_live_state(summary: &mut BrowserSummary, live: &BrowserLiveState, node_id: &str) {
    summary.pages_allowed = live.pages_allowed();
    summary.status = live.status(node_id);
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

/// The live-page button: Load pages across the row while pages are held,
/// else Reload on its left part.
fn live_button_rect(x0: f32, top: f32, usable_w: f32, pages_allowed: bool) -> Rect {
    let row = row_rect(x0, top, 4, usable_w);
    if pages_allowed {
        Rect {
            origin: row.origin,
            size: Point2D::new(usable_w * RELOAD_SHARE, INPUT_HEIGHT),
        }
    } else {
        row
    }
}

/// The device cells and the live-page button, one action each.
pub fn push_browser_action_rects(
    out: &mut Vec<(PropertyPanelAction, Rect)>,
    x0: f32,
    top: f32,
    usable_w: f32,
    pages_allowed: bool,
) {
    let cells = segment_rects(row_rect(x0, top, 1, usable_w), PANEL_DEVICES.len());
    for (id, rect) in PANEL_DEVICES.into_iter().zip(cells) {
        out.push((PropertyPanelAction::SetBrowserDevice(id), rect));
    }
    let action = if pages_allowed {
        PropertyPanelAction::ReloadBrowser
    } else {
        PropertyPanelAction::AllowBrowserPages
    };
    out.push((action, live_button_rect(x0, top, usable_w, pages_allowed)));
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
    paint_live_row(cx, theme, summary, locale, x, top, usable_w);
    let y = top + SECTION_HEADER_HEIGHT + ROW_ADVANCE * ROWS as f32 + BOTTOM_PAD;
    paint_section_divider(cx, theme, x, y, width);
    y + SECTION_GAP
}

/// Load pages, or Reload and the status beside it.
fn paint_live_row(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    summary: &BrowserSummary,
    locale: de_editor_core::Locale,
    x: f32,
    top: f32,
    usable_w: f32,
) {
    let button = live_button_rect(x, top, usable_w, summary.pages_allowed);
    let key = if summary.pages_allowed {
        "browser.reload"
    } else {
        "browser.loadPages"
    };
    paint_segment_row(cx, theme, button, &[label(locale, key)], None, SEGMENT_FONT);
    let Some(status_key) = summary
        .status
        .filter(|_| summary.pages_allowed)
        .and_then(status_key)
    else {
        return;
    };
    let text = label(locale, status_key);
    let left = button.origin.x + button.size.x + HALF_GAP;
    let layout = TextLayout::single_run(
        text,
        crate::widgets::text_metrics::CHROME_FONT_FAMILY,
        SEGMENT_FONT,
        theme.muted_foreground.to_jian(),
        Point2D::new(0.0, 0.0),
    );
    let baseline = jian_widgets::centered_text_baseline_y(button, SEGMENT_FONT);
    cx.backend.draw_text(&layout, Point2D::new(left, baseline));
}

/// The i18n key of a status worth showing (Ready shows nothing).
fn status_key(status: BrowserPageStatus) -> Option<&'static str> {
    match status {
        BrowserPageStatus::Loading => Some("browser.status.loading"),
        BrowserPageStatus::NoBrowser => Some("browser.status.noBrowser"),
        BrowserPageStatus::Failed => Some("browser.status.failed"),
        BrowserPageStatus::Ready => None,
    }
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
