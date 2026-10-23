//! Toolbar action dispatch for web press/click paths — panel toggles
//! delegate to the shared `EditorUiState` transitions.

use super::WidgetHost;

impl WidgetHost {
    pub(in crate::widget_host) fn dispatch_toolbar_action(
        &mut self,
        action: de_editor_ui::widgets::ToolbarAction,
    ) -> bool {
        use de_editor_ui::widgets::ToolbarAction;
        match action {
            ToolbarAction::Undo => {
                let acted = self.editor_state.undo();
                if acted {
                    self.mark_dirty();
                    self.refresh_missing_fonts_after_history_change();
                }
                acted
            }
            ToolbarAction::Redo => {
                let acted = self.editor_state.redo();
                if acted {
                    self.mark_dirty();
                    self.refresh_missing_fonts_after_history_change();
                }
                acted
            }
            ToolbarAction::ToggleVariablesPanel => {
                self.editor_state.editor_ui.toggle_variables_panel();
                self.mark_dirty();
                true
            }
            ToolbarAction::ToggleDesignPanel => {
                self.editor_state.editor_ui.toggle_design_md_panel();
                self.mark_dirty();
                true
            }
            // Same file action the shape menu's import row raised: the host
            // loop owns the file picker.
            ToolbarAction::ImportImage => {
                self.editor_state.editor_ui.pending_file_action =
                    Some(de_editor_core::editor_ui_state::FileAction::ImportImageOrSvg);
                self.mark_dirty();
                true
            }
        }
    }
}
