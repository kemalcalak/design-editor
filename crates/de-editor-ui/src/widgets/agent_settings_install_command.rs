//! An agent's install command as the Agents tab shows it (#304): an inset
//! mono well that wraps the whole command over as many lines as its
//! width needs, with a copy button pinned to its top-right. The CLI
//! status card and the ACP install hint both use it, so no install
//! command is ever fitted with an ellipsis or covered by a button.
//!
//! Geometry is painter-free (hit-tests have no painter): the command is
//! set in JetBrains Mono, whose advance is exactly 0.6 em, so its columns
//! are measured rather than estimated.

use crate::theme::Theme;
use crate::widgets::agent_settings_detail_chrome::{paint_icon_button, text, LABEL_FONT};
use crate::widgets::home_surface::MONO;
use crate::widgets::icons::Icon;
use crate::widgets::PaintCx;
use crate::Rect;

const FONT: f32 = LABEL_FONT;
/// JetBrains Mono's advance width, in em.
const ADVANCE_EM: f32 = 0.6;
const LINE_H: f32 = 18.0;
const PAD_X: f32 = 10.0;
const PAD_Y: f32 = 7.0;
/// Baseline of the first line below the text block's top.
const FIRST_BASELINE: f32 = 13.5;
pub(crate) const COPY_SIZE: f32 = 26.0;
const COPY_INSET: f32 = 4.0;
const COPY_GAP: f32 = 8.0;
/// Below this many columns beside the button, the button drops under the
/// text instead of squeezing it into a sliver.
const MIN_COLUMNS_BESIDE: usize = 16;

/// Where the well, its text and its copy button sit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CommandLayout {
    pub(crate) well: Rect,
    pub(crate) copy: Rect,
    /// Left edge and top of the text block.
    pub(crate) text_x: f32,
    pub(crate) text_y: f32,
    /// Characters per line.
    pub(crate) columns: usize,
}

fn columns_in(width: f32) -> usize {
    ((width / (FONT * ADVANCE_EM)).floor() as usize).max(1)
}

/// `command` broken into lines of at most `columns` characters: at
/// spaces where it can, mid-token only when one token is longer than a
/// line. Joining the lines with single spaces where a space was dropped
/// gives the command back, but the copy action never reads the lines.
pub(crate) fn wrap_command(command: &str, columns: usize) -> Vec<String> {
    let columns = columns.max(1);
    let mut lines = Vec::new();
    let mut line = String::new();
    let mut line_len = 0;
    for word in command.split(' ').filter(|word| !word.is_empty()) {
        let word_len = word.chars().count();
        if line_len > 0 && line_len + 1 + word_len <= columns {
            line.push(' ');
            line.push_str(word);
            line_len += 1 + word_len;
            continue;
        }
        if line_len > 0 {
            lines.push(std::mem::take(&mut line));
            line_len = 0;
        }
        let chars: Vec<char> = word.chars().collect();
        let mut chunks = chars.chunks(columns).peekable();
        while let Some(chunk) = chunks.next() {
            if chunks.peek().is_some() {
                lines.push(chunk.iter().collect());
            } else {
                line = chunk.iter().collect();
                line_len = chunk.len();
            }
        }
    }
    if line_len > 0 || lines.is_empty() {
        lines.push(line);
    }
    lines
}

/// Lay out `command` in a well at (`x`, `y`) that is `w` wide.
pub(crate) fn command_layout(x: f32, y: f32, w: f32, command: &str) -> CommandLayout {
    let inner = (w - PAD_X * 2.0).max(0.0);
    let beside = columns_in(inner - COPY_SIZE - COPY_GAP);
    let fits_beside = beside >= MIN_COLUMNS_BESIDE || command.chars().count() <= beside;
    let columns = if fits_beside {
        beside
    } else {
        columns_in(inner)
    };
    let lines = wrap_command(command, columns).len() as f32;
    let text_h = (lines * LINE_H).max(COPY_SIZE - 2.0 * (PAD_Y - COPY_INSET));
    let text_y = y + PAD_Y;
    let (copy, h) = if fits_beside {
        let copy = Rect::xywh(
            x + w - COPY_INSET - COPY_SIZE,
            y + COPY_INSET,
            COPY_SIZE,
            COPY_SIZE,
        );
        (copy, text_h + PAD_Y * 2.0)
    } else {
        let copy_y = text_y + text_h + COPY_GAP / 2.0;
        let copy = Rect::xywh(x + PAD_X - COPY_INSET, copy_y, COPY_SIZE, COPY_SIZE);
        (copy, copy_y + COPY_SIZE + COPY_INSET - y)
    };
    CommandLayout {
        well: Rect::xywh(x, y, w, h),
        copy,
        text_x: x + PAD_X,
        text_y,
        columns,
    }
}

/// The well, the wrapped command, and the copy button (a check while
/// `copied`).
pub(crate) fn paint_command(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    layout: &CommandLayout,
    command: &str,
    copied: bool,
) {
    cx.backend.fill_round_rect(layout.well, 6.0, theme.muted);
    for (index, line) in wrap_command(command, layout.columns).iter().enumerate() {
        text(
            cx,
            line,
            FONT,
            theme.foreground,
            400,
            MONO,
            layout.text_x,
            layout.text_y + FIRST_BASELINE + index as f32 * LINE_H,
        );
    }
    let (icon, color) = if copied {
        (Icon::Check, theme.status_success)
    } else {
        (Icon::Copy, theme.muted_foreground)
    };
    paint_icon_button(cx, theme, layout.copy, icon, color);
}

/// Whether the copy button should show its check for `command`.
pub(crate) fn copied(copied: Option<&(String, u64)>, command: &str, now_ms: u64) -> bool {
    copied.is_some_and(|(text, at)| {
        text == command
            && crate::widgets::agent_settings_mcp::copy_feedback_active(Some(*at), now_ms)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const ANTIGRAVITY: &str = "curl -fsSL https://antigravity.google/cli/install.sh | bash";

    fn rejoin(lines: &[String]) -> String {
        lines.concat().replace(' ', "")
    }

    #[test]
    fn wrapping_keeps_every_character_and_respects_the_width() {
        for columns in [1, 5, 12, 20, 40, 200] {
            let lines = wrap_command(ANTIGRAVITY, columns);
            assert!(lines.iter().all(|line| line.chars().count() <= columns));
            assert_eq!(rejoin(&lines), ANTIGRAVITY.replace(' ', ""));
        }
        assert_eq!(wrap_command(ANTIGRAVITY, 200), [ANTIGRAVITY]);
        assert_eq!(
            wrap_command("npm install -g @openai/codex", 14),
            ["npm install -g", "@openai/codex"]
        );
    }

    #[test]
    fn the_copy_button_never_overlaps_the_text() {
        for w in [60.0, 96.0, 140.0, 200.0, 300.0, 460.0, 700.0] {
            let layout = command_layout(10.0, 20.0, w, ANTIGRAVITY);
            let lines = wrap_command(ANTIGRAVITY, layout.columns);
            let text_w = layout.columns as f32 * FONT * ADVANCE_EM;
            let text = Rect::xywh(
                layout.text_x,
                layout.text_y,
                text_w,
                lines.len() as f32 * LINE_H,
            );
            let copy = layout.copy;
            let apart = copy.origin.x >= text.origin.x + text.size.x
                || copy.origin.y >= text.origin.y + text.size.y;
            assert!(apart, "w={w}: copy {copy:?} overlaps text {text:?}");
            // Both stay inside the well.
            let well = layout.well;
            assert!(copy.origin.y + copy.size.y <= well.origin.y + well.size.y + 0.01);
            assert!(text.origin.y + text.size.y <= well.origin.y + well.size.y + 0.01);
            assert!(text.origin.x + text_w <= well.origin.x + well.size.x - PAD_X + 0.01);
        }
    }
}
