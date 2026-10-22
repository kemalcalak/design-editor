//! VariablesPanel draft commits — theme/variant header renames and the
//! per-row cell drafts (Name / Number / String / inline Color hex).
//!
//! The whole walk lives in the shared
//! `de_editor_core::host_variables_commit` (the web twin drives the same
//! functions); this file is only the `mark_dirty()` tail.

use super::WidgetHostNative;
use de_editor_core::host_variables_commit as vars_commit;

impl WidgetHostNative {
    /// Commit any pending VariablesPanel theme/variant header rename.
    pub(in crate::widget_host) fn commit_variables_panel_header_focus_if_any(&mut self) {
        if vars_commit::commit_header_focus(&mut self.editor_state) {
            self.mark_dirty();
        }
    }

    pub(in crate::widget_host) fn variable_axis_value_for_variant(
        &self,
        variant: usize,
    ) -> Option<(String, String)> {
        vars_commit::variable_axis_value_for_variant(&self.editor_state, variant)
    }

    /// Commit any pending VariablesPanel row edit (Number / String).
    pub(in crate::widget_host) fn commit_variable_row_focus_if_any(&mut self) {
        if vars_commit::commit_row_focus(&mut self.editor_state) {
            self.mark_dirty();
        }
    }
}
