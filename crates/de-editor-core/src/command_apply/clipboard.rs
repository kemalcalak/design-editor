//! Clipboard arms of [`EditorState::apply_with_allocator`] (copy / cut /
//! paste). Carved out of the `command_apply` spine as pure code motion to
//! keep it under the repo's 800-line cap.

use super::*;

impl EditorState {
    /// Apply `CopySelected` / `CutSelected` / `PasteClipboard`; the spine
    /// routes only those three here.
    pub(super) fn apply_clipboard_command(
        &mut self,
        cmd: EditorCommand,
        allocator: &mut dyn IdAllocator,
    ) -> Result<bool, IdAllocError> {
        Ok(match cmd {
            EditorCommand::CopySelected => self.copy_selected(),
            EditorCommand::CutSelected => {
                let snap = self.snapshot_for_history();
                if self.cut_selected() {
                    self.history_push_past(snap);
                    true
                } else {
                    false
                }
            }
            EditorCommand::PasteClipboard { offset_px } => {
                let snap = self.snapshot_for_history();
                let new_ids = self.paste_clipboard_with_allocator(allocator, offset_px as f64)?;
                if new_ids.is_empty() {
                    return Ok(false);
                }
                self.history_push_past(snap);
                true
            }
            _ => unreachable!("apply_clipboard_command only takes clipboard commands"),
        })
    }
}
