//! Row toggles, inline name editing and the keyboard of the rename-layers
//! preview. The dialog is modal: while it is open it owns every key, so a
//! typed letter never switches tools and an arrow never nudges a layer.
//!
//! The keyboard transitions return `None` when no dialog is open (the host
//! keeps walking its ladder) and `Some(changed)` when the dialog took the
//! key.

use crate::EditorState;

impl EditorState {
    /// Whether the preview owns the keyboard.
    pub fn rename_layers_owns_keyboard(&self) -> bool {
        self.editor_ui.rename_layers.is_some()
    }

    /// Tick or untick row `index`.
    pub fn toggle_rename_layers_row(&mut self, index: usize) -> bool {
        self.commit_rename_layers_edit();
        let Some(row) = self
            .editor_ui
            .rename_layers
            .as_mut()
            .and_then(|dialog| dialog.rows.get_mut(index))
        else {
            return false;
        };
        row.accepted = !row.accepted;
        true
    }

    /// Start editing row `index`'s proposed name, seeded with it.
    pub fn start_rename_layers_edit(&mut self, index: usize, now_ms: u64) -> bool {
        self.commit_rename_layers_edit();
        let Some(dialog) = self.editor_ui.rename_layers.as_mut() else {
            return false;
        };
        let Some(row) = dialog.rows.get(index) else {
            return false;
        };
        dialog.input.set_text(row.proposed.clone());
        dialog.input.touch(now_ms);
        dialog.editing = Some(index);
        true
    }

    /// Write the edited name into its row (an empty draft keeps the old
    /// proposal) and stop editing. `true` when an edit was open.
    pub fn commit_rename_layers_edit(&mut self) -> bool {
        let Some(dialog) = self.editor_ui.rename_layers.as_mut() else {
            return false;
        };
        let Some(index) = dialog.editing.take() else {
            return false;
        };
        let draft = dialog.input.text().trim().to_string();
        if let Some(row) = dialog.rows.get_mut(index) {
            if !draft.is_empty() && draft != row.proposed {
                row.proposed = draft;
                row.edited = true;
                row.accepted = true;
            }
        }
        true
    }

    /// Drop the inline edit without writing it.
    fn cancel_rename_layers_edit(&mut self) -> bool {
        self.editor_ui
            .rename_layers
            .as_mut()
            .and_then(|dialog| dialog.editing.take())
            .is_some()
    }
}

/// A typed character: into the edited name, otherwise swallowed.
pub fn rename_layers_text(state: &mut EditorState, c: char, now_ms: u64) -> Option<bool> {
    let dialog = state.editor_ui.rename_layers.as_mut()?;
    if dialog.editing.is_none() || c.is_control() {
        return Some(false);
    }
    let mut buf = [0u8; 4];
    dialog.input.insert_str(c.encode_utf8(&mut buf), now_ms);
    Some(true)
}

/// Backspace: deletes in the edited name, otherwise swallowed.
pub fn rename_layers_backspace(state: &mut EditorState, now_ms: u64) -> Option<bool> {
    let dialog = state.editor_ui.rename_layers.as_mut()?;
    if dialog.editing.is_none() {
        return Some(false);
    }
    dialog.input.backspace(now_ms);
    Some(true)
}

/// Enter: commits the edited name, or applies the renames.
pub fn rename_layers_enter(state: &mut EditorState) -> Option<bool> {
    state.editor_ui.rename_layers.as_ref()?;
    if state.commit_rename_layers_edit() {
        return Some(true);
    }
    state.apply_rename_layers();
    Some(true)
}

/// Escape: drops the edited name, or closes the preview.
pub fn rename_layers_escape(state: &mut EditorState) -> Option<bool> {
    state.editor_ui.rename_layers.as_ref()?;
    if !state.cancel_rename_layers_edit() {
        state.close_rename_layers();
    }
    Some(true)
}
