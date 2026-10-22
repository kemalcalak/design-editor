use super::ai_chat_transcript::{
    draw_line, ActionStep, Collapsible, ACTION_DETAIL_GAP, ACTION_DETAIL_LINE_H, ACTION_STEP_H,
    BODY_FONT, HEADER_H, LINE_H,
};
use super::ai_chat_transcript_activity::{
    paint_activity_chrome, TranscriptActivityChrome, TranscriptActivityStatus,
};
use crate::theme::Theme;
use crate::widgets::icons::{draw_icon, Icon};
use crate::widgets::PaintCx;
use crate::{Point2D, Rect};

const RETRY_ICON_SIZE: f32 = 14.0;
/// Status glyph + its inset from the right edge (see `activity_status_rect`).
const STATUS_RESERVE: f32 = 24.0;
/// The disclosure chevron and the gap before it.
const EXPANDER_RESERVE: f32 = 20.0;
/// The retry glyph and the gap before it.
const RETRY_RESERVE: f32 = 22.0;

fn right_reserve(step: &ActionStep) -> f32 {
    STATUS_RESERVE
        + if step.details.is_empty() {
            0.0
        } else {
            EXPANDER_RESERVE
        }
        + if step.retryable { RETRY_RESERVE } else { 0.0 }
        + 8.0
}

/// The retry icon's paint/hit rect for a retryable failed row — `None` when
/// the row has nothing to retry (see `ActionStep::retryable`). Exposed
/// `pub(crate)` so the transcript hit-test uses the SAME rect the paint
/// below draws, instead of a second hand-derived copy that could drift.
pub(crate) fn retry_icon_rect(step: &ActionStep) -> Option<Rect> {
    if !step.retryable {
        return None;
    }
    let expander = if step.details.is_empty() {
        0.0
    } else {
        EXPANDER_RESERVE
    };
    let x = step.rect.origin.x + step.rect.size.x - STATUS_RESERVE - expander - RETRY_RESERVE;
    let y = step.rect.origin.y + (ACTION_STEP_H - RETRY_ICON_SIZE) / 2.0;
    Some(Rect::xywh(x, y, RETRY_ICON_SIZE, RETRY_ICON_SIZE))
}

pub(crate) fn paint_action_step(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    step: &ActionStep,
    now_ms: u64,
) {
    let label_color = if step.failed {
        theme.destructive
    } else if step.active {
        theme.foreground
    } else {
        theme.muted_foreground
    };
    let status = if step.failed {
        TranscriptActivityStatus::Error
    } else if step.stopped {
        TranscriptActivityStatus::Stopped
    } else if step.done {
        TranscriptActivityStatus::Done
    } else if step.active {
        TranscriptActivityStatus::Running
    } else {
        TranscriptActivityStatus::Pending
    };
    let header = Rect::xywh(
        step.rect.origin.x,
        step.rect.origin.y,
        step.rect.size.x,
        ACTION_STEP_H,
    );
    paint_activity_chrome(
        cx,
        theme,
        TranscriptActivityChrome {
            rect: step.rect,
            header,
            label: &step.label,
            label_color,
            // Collapsed, the first detail reads inline after the label.
            detail: (!step.expanded)
                .then(|| step.details.first().map(String::as_str))
                .flatten(),
            icon: None,
            surface: None,
            status,
            expanded: (!step.details.is_empty()).then_some(step.expanded),
            right_reserve: right_reserve(step),
        },
        now_ms,
    );
    if let Some(retry_rect) = retry_icon_rect(step) {
        draw_icon(
            cx.backend,
            Icon::RefreshCw,
            retry_rect.origin,
            RETRY_ICON_SIZE,
            theme.muted_foreground,
            1.4,
        );
    }

    if step.expanded && !step.details.is_empty() {
        cx.backend.save();
        cx.backend.clip_rect(step.rect);
        let mut baseline = step.rect.origin.y + ACTION_STEP_H + ACTION_DETAIL_GAP + 10.0;
        for line in &step.details {
            draw_line(
                cx,
                line,
                step.rect.origin.x + 12.0,
                baseline,
                11.0,
                theme.muted_subtle,
            );
            baseline += ACTION_DETAIL_LINE_H;
        }
        cx.backend.restore();
    }
}

/// The thinking block (#26): a quiet "Thought for Ns" header with a
/// chevron, and when open, the reasoning in the third text tone beside a
/// hairline.
pub(crate) fn paint_collapsible(cx: &mut PaintCx<'_>, theme: &Theme, block: &Collapsible) {
    let icon = if block.collapsed {
        Icon::ChevronRight
    } else {
        Icon::ChevronDown
    };
    draw_icon(
        cx.backend,
        icon,
        Point2D::new(
            block.header.origin.x,
            block.header.origin.y + (HEADER_H - 12.0) / 2.0,
        ),
        12.0,
        theme.muted_subtle,
        1.6,
    );
    draw_line(
        cx,
        &block.label,
        block.header.origin.x + 18.0,
        block.header.origin.y + HEADER_H / 2.0 + 12.0 * 0.35,
        12.0,
        theme.muted_subtle,
    );
    if !block.collapsed && !block.lines.is_empty() {
        cx.backend.fill_rect(
            Rect::xywh(
                block.body.origin.x + 5.0,
                block.body.origin.y,
                1.0,
                block.body.size.y,
            ),
            theme.border,
        );
        cx.backend.save();
        cx.backend.clip_rect(block.body);
        let mut baseline = block.body.origin.y + super::ai_chat_transcript::BUBBLE_PAD + 11.0;
        for line in &block.lines {
            draw_line(
                cx,
                line,
                block.body.origin.x + 18.0,
                baseline,
                BODY_FONT - 1.0,
                theme.muted_subtle,
            );
            baseline += LINE_H;
        }
        cx.backend.restore();
    }
}
