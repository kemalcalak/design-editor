//! Toolbar action dispatch for native press/click paths — panel toggles
//! delegate to the shared `EditorUiState` transitions.

use super::WidgetHostNative;

impl WidgetHostNative {
    pub(in crate::widget_host) fn dispatch_toolbar_action(
        &mut self,
        action: de_editor_ui::widgets::ToolbarAction,
    ) -> bool {
        use de_editor_ui::widgets::ToolbarAction;
        match action {
            ToolbarAction::Undo => self.apply_undo(),
            ToolbarAction::Redo => self.apply_redo(),
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
