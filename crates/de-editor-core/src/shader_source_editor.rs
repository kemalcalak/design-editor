//! The shader source editor's state and keyboard editing (#164).
//!
//! "Code" on a shader fill or effect opens a floating editor over the
//! panel with the program's SkSL. It is a plain multi-line text field:
//! Enter inserts a newline (keeping the line's indent), Tab four spaces
//! (tabs in an opened or pasted source become spaces too, so one char is
//! one column),
//! the arrows move by character and line. Apply writes the draft through
//! `EditorCommand::SetShaderSource` (one undo step) and keeps the editor
//! open, so a compile error can be fixed and applied again; Close or
//! Escape drops an unapplied draft.
//!
//! Same contract as the Scene Template Center's keyboard module: every
//! `Option` result is `None` while the editor is closed or unfocused and
//! `Some(changed)` once it owns the key, including keys it ignores, so
//! nothing reaches the canvas while the user types code.

use jian_core::text_input::TextInputState;

use crate::node_id::NodeId;
use crate::shader_check::ShaderCheckCache;
use crate::shader_edit::ShaderTarget;
use crate::state::EditorState;

/// Lines the editor shows at once; the widget sizes its text area by it.
pub const VISIBLE_LINES: usize = 18;

/// What Tab inserts.
const INDENT: &str = "    ";

/// Shader editing UI state: the open source editor, a live slider drag,
/// and the hosts' compile results. Never persisted.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ShaderEditingState {
    pub editor: Option<ShaderSourceEditor>,
    /// Row being dragged on a ranged-float slider, `(target, row)`. The
    /// first drag step snapshots history, the rest write in place.
    pub uniform_drag: Option<(ShaderTarget, usize)>,
    pub checks: ShaderCheckCache,
}

/// The open source editor.
#[derive(Debug, Clone, PartialEq)]
pub struct ShaderSourceEditor {
    /// The node it was opened on; Apply writes there even if the
    /// selection moved.
    pub node_id: NodeId,
    pub target: ShaderTarget,
    pub input: TextInputState,
    /// First visible line.
    pub scroll_line: usize,
    /// Whether it owns the keyboard: a press in it takes focus, a press
    /// elsewhere gives it up (the editor stays open).
    pub focused: bool,
}

impl ShaderSourceEditor {
    /// `(line, column)` of the caret; the column counts chars.
    pub fn caret_line_col(&self) -> (usize, usize) {
        line_col(self.input.text(), self.input.caret())
    }

    /// Scroll so the caret's line is on screen.
    fn reveal_caret(&mut self) {
        let (line, _) = self.caret_line_col();
        if line < self.scroll_line {
            self.scroll_line = line;
        } else if line >= self.scroll_line + VISIBLE_LINES {
            self.scroll_line = line + 1 - VISIBLE_LINES;
        }
    }

    pub fn line_count(&self) -> usize {
        self.input.text().split('\n').count()
    }

    /// Scroll by `lines` (negative = up), clamped to the text.
    pub fn scroll_by(&mut self, lines: i32) -> bool {
        let max = self.line_count().saturating_sub(VISIBLE_LINES);
        let next = (self.scroll_line as i64 + i64::from(lines)).clamp(0, max as i64) as usize;
        let changed = next != self.scroll_line;
        self.scroll_line = next;
        changed
    }

    /// Put the caret at `(line, column)` (clamped), as a click does.
    pub fn place_caret(&mut self, line: usize, column: usize, extend: bool, now_ms: u64) {
        let byte = byte_at(self.input.text(), line, column);
        if extend {
            self.input.drag_to(byte, now_ms);
        } else {
            self.input.set_caret(byte, now_ms);
        }
        self.reveal_caret();
    }
}

/// Byte offset → `(line, char column)`.
pub fn line_col(text: &str, byte: usize) -> (usize, usize) {
    let byte = byte.min(text.len());
    let before = &text[..byte];
    let line = before.matches('\n').count();
    let line_start = before.rfind('\n').map_or(0, |n| n + 1);
    (line, text[line_start..byte].chars().count())
}

/// `(line, char column)` → byte offset, clamped to the line and the text.
pub fn byte_at(text: &str, line: usize, column: usize) -> usize {
    let mut start = 0;
    for _ in 0..line {
        match text[start..].find('\n') {
            Some(n) => start += n + 1,
            None => return text.len(),
        }
    }
    let end = text[start..].find('\n').map_or(text.len(), |n| start + n);
    text[start..end]
        .char_indices()
        .nth(column)
        .map_or(end, |(offset, _)| start + offset)
}

impl EditorState {
    /// Open the source editor on the anchor's shader paint `target`. A
    /// fill expanded from a loader preset has no source and opens empty.
    pub fn open_shader_source_editor(&mut self, target: ShaderTarget) -> bool {
        let node_id = self.selection.anchor.clone();
        let Some(program) = self.selected_shader_program(target) else {
            return false;
        };
        // Tabs become spaces: the editor maps one char to one column.
        let text = program.sksl.unwrap_or_default().replace('\t', INDENT);
        let mut input = TextInputState::with_text(text);
        input.set_caret(0, 0);
        self.editor_ui.shader_editing.editor = Some(ShaderSourceEditor {
            node_id,
            target,
            input,
            scroll_line: 0,
            focused: true,
        });
        true
    }

    /// Close the editor, dropping an unapplied draft.
    pub fn close_shader_source_editor(&mut self) -> bool {
        self.editor_ui.shader_editing.editor.take().is_some()
    }

    /// Write the draft through `SetShaderSource`. The editor stays open;
    /// the caller owns history.
    pub fn apply_shader_source_editor(&mut self) -> bool {
        let Some(editor) = self.editor_ui.shader_editing.editor.as_ref() else {
            return false;
        };
        let command = crate::EditorCommand::SetShaderSource {
            node_id: editor.node_id.clone(),
            target: editor.target,
            sksl: editor.input.text().to_owned(),
        };
        self.apply(command)
    }

    /// Whether the draft differs from the stored source.
    pub fn shader_source_editor_dirty(&self) -> bool {
        let Some(editor) = self.editor_ui.shader_editing.editor.as_ref() else {
            return false;
        };
        let stored = crate::walkers::find_node(self.active_children(), &editor.node_id)
            .and_then(|node| crate::shader_edit::shader_program(node, editor.target))
            .map(|program| program.sksl.unwrap_or_default());
        stored != Some(editor.input.text())
    }
}

fn edit(
    state: &mut EditorState,
    apply: impl FnOnce(&mut ShaderSourceEditor) -> bool,
) -> Option<bool> {
    let editor = state
        .editor_ui
        .shader_editing
        .editor
        .as_mut()
        .filter(|editor| editor.focused)?;
    let changed = apply(editor);
    editor.reveal_caret();
    Some(changed)
}

/// Insert one typed character.
pub fn text(state: &mut EditorState, c: char, now_ms: u64) -> Option<bool> {
    edit(state, |editor| {
        if c == '\t' {
            editor.input.insert_str(INDENT, now_ms);
            return true;
        }
        if c.is_control() {
            return false;
        }
        let mut encoded = [0_u8; 4];
        editor.input.insert_str(c.encode_utf8(&mut encoded), now_ms);
        true
    })
}

/// Insert pasted text, newlines and tabs kept (code is multi-line).
pub fn paste(state: &mut EditorState, pasted: &str, now_ms: u64) -> Option<bool> {
    edit(state, |editor| {
        let cleaned: String = pasted
            .replace("\r\n", "\n")
            .replace('\t', INDENT)
            .chars()
            .filter(|c| !c.is_control() || *c == '\n')
            .collect();
        if cleaned.is_empty() {
            return false;
        }
        editor.input.insert_str(&cleaned, now_ms);
        true
    })
}

/// Enter: a newline carrying the current line's indent.
pub fn newline(state: &mut EditorState, now_ms: u64) -> Option<bool> {
    edit(state, |editor| {
        let text = editor.input.text();
        let caret = editor.input.caret().min(text.len());
        let line_start = text[..caret].rfind('\n').map_or(0, |n| n + 1);
        let indent: String = text[line_start..caret]
            .chars()
            .take_while(|c| *c == ' ' || *c == '\t')
            .collect();
        editor.input.insert_str(&format!("\n{indent}"), now_ms);
        true
    })
}

pub fn backspace(state: &mut EditorState, now_ms: u64) -> Option<bool> {
    edit(state, |editor| {
        let before = editor.input.text().len();
        editor.input.backspace(now_ms);
        editor.input.text().len() != before
    })
}

pub fn delete_forward(state: &mut EditorState, now_ms: u64) -> Option<bool> {
    edit(state, |editor| {
        let before = editor.input.text().len();
        editor.input.delete_forward(now_ms);
        editor.input.text().len() != before
    })
}

/// Left / Right by one character.
pub fn move_caret(
    state: &mut EditorState,
    forward: bool,
    extend: bool,
    now_ms: u64,
) -> Option<bool> {
    edit(state, |editor| {
        if forward {
            editor.input.move_right(extend, now_ms);
        } else {
            editor.input.move_left(extend, now_ms);
        }
        true
    })
}

/// Up / Down by one line, keeping the column where the line allows.
pub fn move_line(state: &mut EditorState, down: bool, extend: bool, now_ms: u64) -> Option<bool> {
    edit(state, |editor| {
        let (line, column) = editor.caret_line_col();
        // Up on the first line goes to its start, as text fields do.
        let byte = match (down, line) {
            (true, _) => byte_at(editor.input.text(), line + 1, column),
            (false, 0) => 0,
            (false, _) => byte_at(editor.input.text(), line - 1, column),
        };
        if extend {
            editor.input.drag_to(byte, now_ms);
        } else {
            editor.input.set_caret(byte, now_ms);
        }
        true
    })
}

pub fn select_all(state: &mut EditorState, now_ms: u64) -> Option<bool> {
    edit(state, |editor| {
        editor.input.select_all();
        editor.input.touch(now_ms);
        true
    })
}

/// Escape closes a focused editor, dropping an unapplied draft.
pub fn escape(state: &mut EditorState) -> Option<bool> {
    state
        .editor_ui
        .shader_editing
        .editor
        .as_ref()
        .filter(|editor| editor.focused)?;
    Some(state.close_shader_source_editor())
}

/// Give up the keyboard (the editor stays open).
pub fn blur(state: &mut EditorState) {
    if let Some(editor) = state.editor_ui.shader_editing.editor.as_mut() {
        editor.focused = false;
    }
}

/// Whether the editor owns the keyboard.
pub fn owns_keyboard(state: &EditorState) -> bool {
    state
        .editor_ui
        .shader_editing
        .editor
        .as_ref()
        .is_some_and(|editor| editor.focused)
}

/// The selected text, for Copy / Cut.
pub fn selected_text(state: &EditorState) -> Option<&str> {
    let editor = state
        .editor_ui
        .shader_editing
        .editor
        .as_ref()
        .filter(|editor| editor.focused)?;
    let (start, end) = editor.input.highlight_range()?;
    editor.input.text().get(start..end)
}

#[cfg(test)]
#[path = "shader_source_editor_tests.rs"]
mod tests;
