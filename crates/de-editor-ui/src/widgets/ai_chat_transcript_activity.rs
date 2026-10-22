//! Shared status primitive for every transcript activity card.
//!
//! CLI-backed design activities and built-in tool calls have different
//! backend payloads, but their user-facing lifecycle is identical. Both
//! adapters paint through this module so loading, success, and failure cannot
//! drift visually again.

use super::ai_chat_transcript::draw_line;
use crate::theme::Theme;
use crate::widgets::icons::{draw_icon, draw_spinning_loader, Icon};
use crate::widgets::PaintCx;
use crate::{Color, Point2D, Rect, TextLayout};

const CARD_RADIUS: f32 = 6.0;
const LABEL_X: f32 = 12.0;
const LABEL_FONT: f32 = 12.0;
const STATUS_GLYPH: f32 = 14.0;
/// The leading glyph of a tool-call row.
pub(crate) const ICON_GLYPH: f32 = 14.0;
const EXPANDER_GLYPH: f32 = 12.0;
/// Inset of the status glyph from the row's right edge.
const STATUS_RIGHT: f32 = 10.0;
const EXPANDER_GAP: f32 = 8.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TranscriptActivityStatus {
    Pending,
    Running,
    Done,
    Error,
    Stopped,
}

/// Shared visual chrome for CLI activities and built-in tool calls. Their
/// payload bodies remain backend-specific, while label, lifecycle and
/// disclosure affordance are painted once here. Rows are framed by the
/// list they sit in (#26); a row only paints its own surface when it has
/// one to call out (a destructive tool call).
pub(crate) struct TranscriptActivityChrome<'a> {
    pub rect: Rect,
    pub header: Rect,
    pub label: &'a str,
    pub label_color: Color,
    /// Quiet text after the label: the first detail of a collapsed row.
    pub detail: Option<&'a str>,
    /// Glyph leading the row (tool calls); progress steps have none.
    pub icon: Option<Icon>,
    /// `(fill, border)` for a row that stands out from its list.
    pub surface: Option<(Color, Color)>,
    pub status: TranscriptActivityStatus,
    /// `None` means this row has no disclosure body.
    pub expanded: Option<bool>,
    /// Space kept clear at the right for the status, expander and an
    /// optional source badge.
    pub right_reserve: f32,
}

/// Right edge of the label area for a row: what is left after the status
/// glyph and, when the row discloses a body, its chevron.
pub(crate) fn activity_status_rect(header: Rect) -> Rect {
    Rect::xywh(
        header.origin.x + header.size.x - STATUS_RIGHT - STATUS_GLYPH,
        header.origin.y + (header.size.y - STATUS_GLYPH) / 2.0,
        STATUS_GLYPH,
        STATUS_GLYPH,
    )
}

pub(crate) fn paint_activity_chrome(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    chrome: TranscriptActivityChrome<'_>,
    now_ms: u64,
) {
    if let Some((fill, border)) = chrome.surface {
        cx.backend.fill_round_rect(chrome.rect, CARD_RADIUS, fill);
        cx.backend
            .stroke_round_rect(chrome.rect, CARD_RADIUS, border, 1.0);
    }

    let icon_w = if let Some(icon) = chrome.icon {
        draw_icon(
            cx.backend,
            icon,
            Point2D::new(
                chrome.header.origin.x + LABEL_X,
                chrome.header.origin.y + (chrome.header.size.y - ICON_GLYPH) / 2.0,
            ),
            ICON_GLYPH,
            theme.muted_subtle,
            1.5,
        );
        ICON_GLYPH + 8.0
    } else {
        0.0
    };
    let label_x = chrome.header.origin.x + LABEL_X + icon_w;
    let label_w = (chrome.header.size.x - LABEL_X - icon_w - chrome.right_reserve).max(1.0);
    let baseline = chrome.header.origin.y + chrome.header.size.y / 2.0 + LABEL_FONT * 0.35;
    cx.backend.save();
    cx.backend.clip_rect(Rect::xywh(
        label_x,
        chrome.header.origin.y,
        label_w,
        chrome.header.size.y,
    ));
    let label =
        crate::widgets::text_metrics::fit_chrome(cx.backend, chrome.label, label_w, LABEL_FONT);
    let label_layout = TextLayout::single_run(
        &label,
        crate::widgets::home_surface::SANS,
        LABEL_FONT,
        chrome.label_color.to_jian(),
        Point2D::ZERO,
    )
    .with_font_weight(600);
    cx.backend
        .draw_text(&label_layout, Point2D::new(label_x, baseline));
    if let Some(detail) = chrome.detail.filter(|d| !d.trim().is_empty()) {
        let used = crate::widgets::text_metrics::measure_chrome_weighted(
            cx.backend, &label, LABEL_FONT, 600,
        );
        let detail_x = label_x + used + 8.0;
        let room = label_x + label_w - detail_x;
        if room > 24.0 {
            let detail =
                crate::widgets::text_metrics::fit_chrome(cx.backend, detail, room, LABEL_FONT);
            draw_line(
                cx,
                &detail,
                detail_x,
                baseline,
                LABEL_FONT,
                theme.muted_subtle,
            );
        }
    }
    cx.backend.restore();

    let status_rect = activity_status_rect(chrome.header);
    paint_activity_status(
        cx,
        theme,
        status_rect.origin,
        STATUS_GLYPH,
        chrome.status,
        now_ms,
    );

    let Some(expanded) = chrome.expanded else {
        return;
    };
    draw_icon(
        cx.backend,
        if expanded {
            Icon::ChevronDown
        } else {
            Icon::ChevronRight
        },
        Point2D::new(
            status_rect.origin.x - EXPANDER_GAP - EXPANDER_GLYPH,
            chrome.header.origin.y + (chrome.header.size.y - EXPANDER_GLYPH) / 2.0,
        ),
        EXPANDER_GLYPH,
        theme.muted_subtle,
        1.5,
    );
}

pub(crate) fn paint_activity_status(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    top_left: Point2D,
    size: f32,
    status: TranscriptActivityStatus,
    now_ms: u64,
) {
    match status {
        TranscriptActivityStatus::Pending => {
            let inset = size * 0.2;
            cx.backend.stroke_oval(
                Rect::xywh(
                    top_left.x + inset,
                    top_left.y + inset,
                    size - inset * 2.0,
                    size - inset * 2.0,
                ),
                theme.muted_subtle,
                1.2,
            );
        }
        TranscriptActivityStatus::Running => {
            draw_spinning_loader(cx.backend, top_left, size, theme.primary, 1.6, now_ms)
        }
        TranscriptActivityStatus::Error => draw_icon(
            cx.backend,
            Icon::Close,
            top_left,
            size,
            theme.destructive,
            1.8,
        ),
        TranscriptActivityStatus::Stopped => {
            let side = size * 0.5;
            cx.backend.fill_round_rect(
                Rect::xywh(
                    top_left.x + (size - side) / 2.0,
                    top_left.y + (size - side) / 2.0,
                    side,
                    side,
                ),
                1.5,
                theme.muted_subtle,
            );
        }
        TranscriptActivityStatus::Done => draw_icon(
            cx.backend,
            Icon::Check,
            top_left,
            size,
            theme.status_success,
            1.8,
        ),
    }
}
