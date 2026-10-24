//! The floating shader source editor (#164).
//!
//! Opened from a shader body's Code button, it floats left of the
//! properties card: a header with the title and ✕, the SkSL in the mono
//! face with line numbers, and a footer with the compile result of the
//! draft (the hosts compile it, `de_editor_core::shader_check`) and the
//! Close / Apply buttons. Apply writes the draft as one undo step and keeps
//! the editor open, so an error can be fixed and applied again.
//!
//! State and keyboard editing live in `de_editor_core::shader_source_editor`;
//! this module is geometry, paint and the shared press / wheel flows the
//! hosts call.

use de_editor_core::shader_source_editor::{ShaderSourceEditor, VISIBLE_LINES};
use de_editor_core::{EditorState, ShaderCheck};

use crate::theme::Theme;
use crate::widgets::icons::{draw_icon, Icon};
use crate::widgets::text_metrics;
use crate::widgets::PaintCx;
use crate::{Point2D, Rect, TextLayout};

const WIDTH: f32 = 480.0;
const HEADER_H: f32 = 40.0;
const FOOTER_H: f32 = 64.0;
/// Code font size and line height.
pub const CODE_FONT_SIZE: f32 = 12.0;
pub const LINE_H: f32 = 18.0;
/// JetBrains Mono advances 0.6 em; clicks map to columns with it.
pub const CHAR_W: f32 = CODE_FONT_SIZE * 0.6;
const GUTTER_W: f32 = 40.0;
const TEXT_PAD: f32 = 8.0;
const GAP_TO_CARD: f32 = 12.0;
const MARGIN: f32 = 8.0;
const BUTTON_W: f32 = 72.0;
const BUTTON_H: f32 = 28.0;
const RADIUS: f32 = 10.0;

/// Where every part of the editor sits.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EditorLayout {
    pub panel: Rect,
    pub close: Rect,
    /// The code area (gutter + text).
    pub code: Rect,
    /// First text column's x.
    pub text_x: f32,
    pub status: Rect,
    pub cancel: Rect,
    pub apply: Rect,
}

/// The editor's geometry next to the properties card `property_rect`.
pub fn layout(property_rect: Rect, viewport_width: f32, viewport_height: f32) -> EditorLayout {
    let code_h = VISIBLE_LINES as f32 * LINE_H + TEXT_PAD * 2.0;
    let height = HEADER_H + code_h + FOOTER_H;
    let width = WIDTH.min(viewport_width - MARGIN * 2.0).max(200.0);
    let x = (property_rect.origin.x - GAP_TO_CARD - width).max(MARGIN);
    let y = property_rect
        .origin
        .y
        .min(viewport_height - height - MARGIN)
        .max(MARGIN);
    let panel = Rect::xywh(x, y, width, height);
    let close = Rect::xywh(x + width - 34.0, y + 8.0, 24.0, 24.0);
    let code = Rect::xywh(x + 1.0, y + HEADER_H, width - 2.0, code_h);
    let footer_y = y + HEADER_H + code_h;
    let button_y = footer_y + FOOTER_H - BUTTON_H - 12.0;
    let apply = Rect::xywh(x + width - 12.0 - BUTTON_W, button_y, BUTTON_W, BUTTON_H);
    let cancel = Rect::xywh(
        apply.origin.x - 8.0 - BUTTON_W,
        button_y,
        BUTTON_W,
        BUTTON_H,
    );
    let status = Rect::xywh(
        x + 12.0,
        footer_y + 6.0,
        (cancel.origin.x - x - 24.0).max(0.0),
        FOOTER_H - 12.0,
    );
    EditorLayout {
        panel,
        close,
        code,
        text_x: code.origin.x + GUTTER_W + TEXT_PAD,
        status,
        cancel,
        apply,
    }
}

/// What a press on the editor hit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorHit {
    Close,
    Apply,
    /// The code at `(line, column)`.
    Code {
        line: usize,
        column: usize,
    },
    /// The editor's chrome.
    Inside,
    Outside,
}

pub fn hit(layout: &EditorLayout, editor: &ShaderSourceEditor, point: Point2D) -> EditorHit {
    if !layout.panel.contains(point) {
        return EditorHit::Outside;
    }
    if layout.close.contains(point) || layout.cancel.contains(point) {
        return EditorHit::Close;
    }
    if layout.apply.contains(point) {
        return EditorHit::Apply;
    }
    if layout.code.contains(point) {
        let row = ((point.y - layout.code.origin.y - TEXT_PAD) / LINE_H)
            .floor()
            .max(0.0) as usize;
        let column = ((point.x - layout.text_x) / CHAR_W).round().max(0.0) as usize;
        return EditorHit::Code {
            line: editor.scroll_line + row,
            column,
        };
    }
    EditorHit::Inside
}

/// Press flow shared by both hosts. `None` when no editor is open or the
/// press is outside it (the press goes on to what lies below, and the
/// editor gives up the keyboard); `Some(changed)` when the editor took it.
/// Apply snapshots history around the write, so it is one undo step.
pub fn apply_press(
    state: &mut EditorState,
    layout: &EditorLayout,
    point: Point2D,
    extend: bool,
    now_ms: u64,
) -> Option<bool> {
    let editor = state.editor_ui.shader_editing.editor.as_mut()?;
    match hit(layout, editor, point) {
        EditorHit::Outside => {
            editor.focused = false;
            None
        }
        EditorHit::Close => Some(state.close_shader_source_editor()),
        EditorHit::Apply => {
            editor.focused = true;
            let before = state.snapshot_for_history();
            let wrote = state.apply_shader_source_editor();
            if wrote {
                state.history_push_past(before);
            }
            Some(true)
        }
        EditorHit::Code { line, column } => {
            editor.focused = true;
            editor.place_caret(line, column, extend, now_ms);
            Some(true)
        }
        EditorHit::Inside => {
            editor.focused = true;
            Some(true)
        }
    }
}

/// Wheel over the editor scrolls the code. `true` when the wheel was over
/// it (consumed, even at a scroll limit).
pub fn apply_wheel(
    state: &mut EditorState,
    layout: &EditorLayout,
    point: Point2D,
    dy: f32,
) -> bool {
    let Some(editor) = state.editor_ui.shader_editing.editor.as_mut() else {
        return false;
    };
    if !layout.panel.contains(point) {
        return false;
    }
    // A positive delta scrolls up, as in every panel (`scroll_flow`).
    let lines = (-dy / LINE_H).round() as i32;
    let lines = if lines == 0 && dy != 0.0 {
        -dy.signum() as i32
    } else {
        lines
    };
    editor.scroll_by(lines);
    true
}

/// Paint the open editor. `check` is the draft's compile result.
pub fn paint(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    locale: de_editor_core::Locale,
    layout: &EditorLayout,
    editor: &ShaderSourceEditor,
    check: &ShaderCheck,
    now_ms: u64,
) {
    let panel = layout.panel;
    cx.backend.fill_round_rect(panel, RADIUS, theme.popover);
    cx.backend
        .stroke_round_rect(panel, RADIUS, theme.border, 1.0);
    let title = de_i18n::translate(locale, "shader.sourceTitle");
    draw_chrome_text(
        cx,
        title,
        12.0,
        theme.foreground,
        Point2D::new(panel.origin.x + 14.0, panel.origin.y + 25.0),
    );
    draw_icon(
        cx.backend,
        Icon::Close,
        Point2D::new(layout.close.origin.x + 5.0, layout.close.origin.y + 5.0),
        14.0,
        theme.muted_foreground,
        1.4,
    );
    cx.backend.fill_rect(layout.code, theme.background);
    paint_code(cx, theme, layout, editor, now_ms);
    paint_footer(cx, theme, locale, layout, check);
}

fn draw_chrome_text(cx: &mut PaintCx<'_>, text: &str, size: f32, color: crate::Color, at: Point2D) {
    let layout = TextLayout::single_run(
        text,
        text_metrics::CHROME_FONT_FAMILY,
        size,
        color.to_jian(),
        Point2D::new(0.0, 0.0),
    );
    cx.backend.draw_text(&layout, at);
}

fn draw_code_text(cx: &mut PaintCx<'_>, text: &str, color: crate::Color, at: Point2D) {
    let layout = TextLayout::single_run(
        text,
        text_metrics::MONO_FONT_FAMILY,
        CODE_FONT_SIZE,
        color.to_jian(),
        Point2D::new(0.0, 0.0),
    );
    cx.backend.draw_text(&layout, at);
}

/// Line numbers, the visible lines, the selection and the caret.
fn paint_code(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    layout: &EditorLayout,
    editor: &ShaderSourceEditor,
    now_ms: u64,
) {
    let text = editor.input.text();
    let top = layout.code.origin.y + TEXT_PAD;
    let max_columns = ((layout.code.origin.x + layout.code.size.x - layout.text_x - TEXT_PAD)
        / CHAR_W)
        .floor()
        .max(1.0) as usize;
    let highlight = editor.input.highlight_range();
    let (caret_line, caret_column) = editor.caret_line_col();
    cx.backend.save();
    cx.backend.clip_rect(layout.code);
    let mut line_start = 0;
    for (line, content) in text.split('\n').enumerate() {
        let line_end = line_start + content.len();
        if line >= editor.scroll_line && line < editor.scroll_line + VISIBLE_LINES {
            let row = (line - editor.scroll_line) as f32;
            let baseline = top + row * LINE_H + LINE_H - 5.0;
            let number = (line + 1).to_string();
            let number_x = layout.code.origin.x + GUTTER_W - 6.0 - number.len() as f32 * CHAR_W;
            draw_code_text(
                cx,
                &number,
                theme.muted_subtle,
                Point2D::new(number_x, baseline),
            );
            if let Some((start, end)) = highlight {
                let from = start.clamp(line_start, line_end);
                let to = end.clamp(line_start, line_end);
                // A selection running past the line end covers its newline.
                let through_newline = start <= line_end && end > line_end;
                if to > from || through_newline {
                    let col0 = text[line_start..from].chars().count() as f32;
                    let cols = text[from..to].chars().count() as f32
                        + if through_newline { 1.0 } else { 0.0 };
                    cx.backend.fill_rect(
                        Rect::xywh(
                            layout.text_x + col0 * CHAR_W,
                            top + row * LINE_H,
                            cols * CHAR_W,
                            LINE_H,
                        ),
                        crate::util::alpha(theme.primary, 0.25),
                    );
                }
            }
            let visible: String = content.chars().take(max_columns).collect();
            let visible = visible.replace('\t', "    ");
            draw_code_text(
                cx,
                &visible,
                theme.foreground,
                Point2D::new(layout.text_x, baseline),
            );
            if editor.focused && line == caret_line && editor.input.caret_visible(now_ms) {
                let x = layout.text_x + caret_column.min(max_columns) as f32 * CHAR_W;
                cx.backend.fill_rect(
                    Rect::xywh(x, top + row * LINE_H + 2.0, 1.5, LINE_H - 4.0),
                    theme.foreground,
                );
            }
        }
        line_start = line_end + 1;
    }
    cx.backend.restore();
    // Gutter rule.
    cx.backend.fill_rect(
        Rect::xywh(
            layout.code.origin.x + GUTTER_W,
            layout.code.origin.y,
            1.0,
            layout.code.size.y,
        ),
        theme.border,
    );
}

/// The draft's compile result and the two buttons.
fn paint_footer(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    locale: de_editor_core::Locale,
    layout: &EditorLayout,
    check: &ShaderCheck,
) {
    let status = layout.status;
    let (lines, color): (Vec<String>, crate::Color) = match check {
        ShaderCheck::Pending => (Vec::new(), theme.muted_foreground),
        ShaderCheck::Ok => (
            vec![de_i18n::translate(locale, "shader.compiles").to_owned()],
            theme.muted_foreground,
        ),
        ShaderCheck::Error(message) => {
            let mut lines = vec![de_i18n::translate(locale, "shader.compileError").to_owned()];
            lines.extend(message.lines().take(2).map(str::to_owned));
            (lines, theme.destructive)
        }
    };
    for (i, line) in lines.iter().take(3).enumerate() {
        let size = if i == 0 { 11.0 } else { 10.5 };
        let fitted = text_metrics::fit_chrome(cx.backend, line, status.size.x, size);
        draw_chrome_text(
            cx,
            &fitted,
            size,
            color,
            Point2D::new(status.origin.x, status.origin.y + 13.0 + i as f32 * 15.0),
        );
    }
    paint_button(
        cx,
        theme,
        layout.cancel,
        de_i18n::translate(locale, "shader.close"),
        false,
    );
    paint_button(
        cx,
        theme,
        layout.apply,
        de_i18n::translate(locale, "shader.apply"),
        true,
    );
}

fn paint_button(cx: &mut PaintCx<'_>, theme: &Theme, rect: Rect, label: &str, primary: bool) {
    let (fill, ink) = if primary {
        (theme.primary, theme.primary_foreground)
    } else {
        (theme.muted, theme.foreground)
    };
    cx.backend.fill_round_rect(rect, 8.0, fill);
    let label = text_metrics::fit_chrome(cx.backend, label, rect.size.x - 12.0, 12.0);
    let w = text_metrics::measure_chrome(cx.backend, &label, 12.0);
    draw_chrome_text(
        cx,
        &label,
        12.0,
        ink,
        Point2D::new(
            rect.origin.x + (rect.size.x - w) / 2.0,
            rect.origin.y + rect.size.y / 2.0 + 4.0,
        ),
    );
}

#[cfg(test)]
#[path = "shader_source_editor_tests.rs"]
mod tests;
