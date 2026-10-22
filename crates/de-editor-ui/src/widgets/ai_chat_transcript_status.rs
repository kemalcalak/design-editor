//! The transcript's #26 status pieces: the "Thought for Ns" label, the
//! live "Working…" line under a streaming message, and the single
//! bordered list that consecutive design-progress rows share.

use super::ai_chat_transcript::{draw_line, ActionStep};
use crate::theme::Theme;
use crate::widgets::PaintCx;
use crate::Rect;
use de_editor_core::chat::ChatMessage;

/// Height of the live status line.
pub(crate) const STATUS_LINE_H: f32 = 18.0;
const STATUS_FONT: f32 = 12.0;
const STATUS_DOT: f32 = 6.0;
const LIST_RADIUS: f32 = 8.0;

/// The live line under a streaming message.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct StatusLine {
    pub rect: Rect,
    pub label: String,
}

/// The thinking block's header: "Thought for 6s" once the clock stopped,
/// "Thinking…" while the assistant is still reasoning, and the plain
/// "Thinking" for turns that never reported a duration.
pub(crate) fn thinking_label(msg: &ChatMessage, locale: de_editor_core::Locale) -> String {
    if let Some(ms) = msg.thinking_ms {
        let seconds = ((ms as f32 / 1000.0).round() as u64).max(1);
        return de_i18n::translate(locale, "chat.thought")
            .replace("{{seconds}}", &seconds.to_string());
    }
    let still_thinking =
        msg.streaming && msg.content.trim().is_empty() && msg.tool_calls.is_empty();
    de_i18n::translate(
        locale,
        if still_thinking {
            "chat.thinking"
        } else {
            "ai.thinkingProcess"
        },
    )
    .to_string()
}

/// Paint "● Working…": an accent dot that breathes, then the label.
pub(crate) fn paint_status_line(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    status: &StatusLine,
    now_ms: u64,
) {
    let rect = status.rect;
    let phase = (now_ms % 1200) as f32 / 1200.0 * std::f32::consts::TAU;
    let dot = Rect::xywh(
        rect.origin.x + 1.0,
        rect.origin.y + (rect.size.y - STATUS_DOT) / 2.0,
        STATUS_DOT,
        STATUS_DOT,
    );
    cx.backend.fill_oval(
        dot,
        theme.primary.with_alpha(0.55 + 0.45 * phase.cos().abs()),
    );
    draw_line(
        cx,
        &status.label,
        rect.origin.x + STATUS_DOT + 8.0,
        rect.origin.y + rect.size.y / 2.0 + STATUS_FONT * 0.35,
        STATUS_FONT,
        theme.muted_subtle,
    );
}

/// The bordered lists consecutive progress rows share: rows laid out
/// back to back (no gap) belong to one list; a row placed elsewhere
/// (after narration, say) starts the next.
pub(crate) fn step_lists(steps: &[ActionStep]) -> Vec<Rect> {
    let mut lists: Vec<Rect> = Vec::new();
    for step in steps {
        let r = step.rect;
        match lists.last_mut() {
            Some(list)
                if (list.origin.y + list.size.y - r.origin.y).abs() < 0.5
                    && (list.origin.x - r.origin.x).abs() < 0.5 =>
            {
                list.size.y = r.origin.y + r.size.y - list.origin.y;
            }
            _ => lists.push(r),
        }
    }
    lists
}

/// Frame each list of progress rows (the rows paint their own content).
pub(crate) fn paint_step_lists(cx: &mut PaintCx<'_>, theme: &Theme, steps: &[ActionStep]) {
    for list in step_lists(steps) {
        cx.backend
            .stroke_round_rect(list, LIST_RADIUS, theme.border, 1.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use de_editor_core::Locale;

    fn step(y: f32, h: f32) -> ActionStep {
        ActionStep {
            rect: Rect::xywh(16.0, y, 300.0, h),
            source_index: 0,
            label: String::new(),
            details: Vec::new(),
            expanded: false,
            done: true,
            active: false,
            failed: false,
            stopped: false,
            retryable: false,
        }
    }

    #[test]
    fn back_to_back_rows_share_one_list() {
        let lists = step_lists(&[step(0.0, 30.0), step(30.0, 30.0), step(100.0, 30.0)]);
        assert_eq!(lists.len(), 2);
        assert_eq!(lists[0].size.y, 60.0);
        assert_eq!(lists[1].origin.y, 100.0);
    }

    #[test]
    fn the_thinking_label_follows_the_clock() {
        let mut msg = ChatMessage::assistant_streaming();
        msg.thinking.push_str("Planning");
        assert_eq!(thinking_label(&msg, Locale::EnUs), "Thinking…");
        msg.thinking_ms = Some(5_600);
        assert_eq!(thinking_label(&msg, Locale::EnUs), "Thought for 6s");
        msg.thinking_ms = Some(200);
        assert_eq!(thinking_label(&msg, Locale::EnUs), "Thought for 1s");
        msg.thinking_ms = None;
        msg.streaming = false;
        assert_eq!(
            thinking_label(&msg, Locale::EnUs),
            de_i18n::translate(Locale::EnUs, "ai.thinkingProcess")
        );
    }
}
