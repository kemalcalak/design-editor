//! Backspace / Delete / Duplicate on `WidgetHostNative` — the
//! focused-input-first ladder that keeps a keystroke from deleting the
//! selection out from under a caret.
//!
//! Split out of `keyboard.rs` to keep every file under the repo's
//! 800-line cap.

use super::WidgetHostNative;
use de_editor_core::host_keyboard_transitions as shared;
use de_editor_core::host_preset_name_draft as preset_name;

impl WidgetHostNative {
    pub fn apply_backspace(&mut self) -> bool {
        // Home's overlays own Backspace above the sheet draft (the same
        // precedence `apply_text` gives them).
        if self.editor_state.editor_ui.home.visible {
            if self.editor_state.editor_ui.chat_model_picker.open {
                return self.apply_chat_model_picker_backspace();
            }
            if !self.editor_state.editor_ui.agent_settings_open {
                if let Some(changed) = self.home_backspace() {
                    return changed;
                }
            }
            // Settings modal over Home: fall through so its own input
            // arm in the ladder below takes the key.
        }
        if let Some(changed) = shared::prompt_center_backspace(&mut self.editor_state, self.now_ms)
        {
            if changed {
                self.mark_dirty();
            }
            return true;
        }
        if let Some(changed) = shared::scene_template_backspace(&mut self.editor_state, self.now_ms)
        {
            if changed {
                self.mark_dirty();
            }
            return true;
        }
        // Preview mode: Backspace edits the focused runtime widget, not
        // the editor selection.
        if self.preview.is_some() {
            return self.preview_dispatch_key("Backspace", false);
        }
        if self.apply_image_panel_backspace() {
            return true;
        }
        // The shader source editor (#164) owns the keyboard while focused.
        if let Some(changed) =
            de_editor_core::shader_source_editor::backspace(&mut self.editor_state, self.now_ms)
        {
            if changed {
                self.mark_dirty();
            }
            return true;
        }
        if self.editor_state.color_picker_hex_focused() {
            self.editor_state.color_picker_hex_backspace(self.now_ms);
            self.mark_dirty();
            return true;
        }
        if self.editor_state.color_picker_rgb_focused() {
            self.editor_state.color_picker_rgb_backspace(self.now_ms);
            self.mark_dirty();
            return true;
        }
        if self.editor_state.editor_ui.agent_settings.focus.is_some() {
            return self.apply_settings_backspace();
        }
        if self.git_clone_input_active() {
            // Swallow Backspace whenever the wizard is open so it can
            // never delete a selected node; pop a char only from a
            // focused field that isn't mid-clone.
            if let Some(form) = self.editor_state.editor_ui.git_panel.clone_form.as_mut() {
                if !form.cloning {
                    match form.focus {
                        Some(de_editor_core::CloneField::Url) => {
                            form.url_input.backspace(self.now_ms)
                        }
                        Some(de_editor_core::CloneField::Dest) => {
                            form.dest_input.backspace(self.now_ms)
                        }
                        None => {}
                    }
                    form.error = None;
                }
            }
            self.mark_dirty();
            return true;
        }
        if self.git_commit_focus_active() {
            let panel = &mut self.editor_state.editor_ui.git_panel;
            panel.commit_input.backspace(self.now_ms);
            // Editing the message (delete or cut, not just typing) clears
            // the stale "no changes to commit" hint, matching `apply_text`.
            panel.commit_no_changes = false;
            self.mark_dirty();
            return true;
        }
        if self.git_remote_focus_active() {
            let panel = &mut self.editor_state.editor_ui.git_panel;
            panel.remote_input.backspace(self.now_ms);
            self.mark_dirty();
            return true;
        }
        if self.git_https_focus_active() {
            let panel = &mut self.editor_state.editor_ui.git_panel;
            panel.https_input.backspace(self.now_ms);
            self.mark_dirty();
            return true;
        }
        if self.git_author_focus_active() {
            let panel = &mut self.editor_state.editor_ui.git_panel;
            if panel.author_email_focused {
                panel.author_email_input.backspace(self.now_ms);
            } else {
                panel.author_name_input.backspace(self.now_ms);
            }
            self.mark_dirty();
            return true;
        }
        if self.git_branch_create_focus_active() {
            let panel = &mut self.editor_state.editor_ui.git_panel;
            panel.branch_create_input.backspace(self.now_ms);
            self.mark_dirty();
            return true;
        }
        if let Some(changed) = shared::rename_backspace(&mut self.editor_state, self.now_ms) {
            if changed {
                self.mark_dirty();
            }
            return changed;
        }
        if let Some(changed) = shared::text_edit_backspace(&mut self.editor_state, self.now_ms) {
            if changed {
                self.mark_dirty();
            }
            return changed;
        }
        // Variables-panel search filter — pop one char.
        if let Some(changed) =
            shared::variables_search_backspace(&mut self.editor_state, self.now_ms)
        {
            if changed {
                self.mark_dirty();
            }
            return changed;
        }
        if self.editor_state.editor_ui.variables_header_rename_active() {
            let changed = shared::variables_header_backspace(&mut self.editor_state, self.now_ms);
            if changed {
                self.mark_dirty();
            }
            return changed;
        }
        if let Some(changed) =
            preset_name::preset_name_backspace(&mut self.editor_state, self.now_ms)
        {
            if changed {
                self.mark_dirty();
            }
            return changed;
        }
        if self.editor_state.editor_ui.variable_row_focus.is_some() {
            let changed = shared::variable_row_backspace(&mut self.editor_state, self.now_ms);
            if changed {
                self.mark_dirty();
            }
            return changed;
        }
        if self.editor_state.ui.property_focus.is_some()
            || self.editor_state.editor_ui.effect_param_focus.is_some()
        {
            let changed = shared::property_input_backspace(&mut self.editor_state, self.now_ms);
            if changed {
                self.mark_dirty();
            }
            return changed;
        }
        // Font-family picker search box (font_picker_dispatch.rs).
        if self.apply_font_picker_backspace() {
            return true;
        }
        if self.editor_state.editor_ui.icon_picker.open {
            if self.editor_state.editor_ui.icon_picker_select_all {
                self.editor_state.editor_ui.icon_picker_search.clear();
                self.editor_state.editor_ui.icon_picker_select_all = false;
                self.editor_state.editor_ui.icon_picker.hover = None;
                self.editor_state.editor_ui.icon_picker.pressed = None;
                // Filter changed → scroll the list back to the top.
                self.editor_state.editor_ui.icon_picker.scroll.offset = 0.0;
                self.mark_dirty();
                return true;
            }
            if self
                .editor_state
                .editor_ui
                .icon_picker_search
                .pop()
                .is_some()
            {
                self.editor_state.editor_ui.icon_picker.hover = None;
                self.editor_state.editor_ui.icon_picker.pressed = None;
                // Filter changed → scroll the list back to the top.
                self.editor_state.editor_ui.icon_picker.scroll.offset = 0.0;
                self.mark_dirty();
                return true;
            }
            return false;
        }
        if self.editor_state.editor_ui.chat_model_picker.open {
            return self.apply_chat_model_picker_backspace();
        }
        if self.editor_state.editor_ui.component_browser_search_focused {
            if self.editor_state.editor_ui.component_browser_select_all {
                self.editor_state.editor_ui.component_browser_search.clear();
                self.editor_state.editor_ui.component_browser_select_all = false;
                self.mark_dirty();
                return true;
            }
            if self
                .editor_state
                .editor_ui
                .component_browser_search
                .pop()
                .is_some()
            {
                self.mark_dirty();
                return true;
            }
            return false;
        }
        if let Some(changed) = shared::chat_input_backspace(&mut self.editor_state, self.now_ms) {
            if changed {
                self.mark_dirty();
            }
            return changed;
        }
        // Pen authoring: Backspace pops the last anchor (`pen_press.rs`).
        if self.apply_pen_backspace() {
            return true;
        }
        if shared::delete_selection_with_history(&mut self.editor_state) {
            self.mark_dirty();
            return true;
        }
        false
    }

    /// Delete — pops a char from rename / text-edit when active;
    /// otherwise deletes the selected node.
    pub fn apply_delete(&mut self) -> bool {
        if let Some(changed) = self.home_delete() {
            return changed;
        }
        if let Some(changed) =
            shared::prompt_center_delete_forward(&mut self.editor_state, self.now_ms)
        {
            if changed {
                self.mark_dirty();
            }
            return true;
        }
        if let Some(changed) =
            shared::scene_template_delete_forward(&mut self.editor_state, self.now_ms)
        {
            if changed {
                self.mark_dirty();
            }
            return true;
        }
        // Preview mode: Delete edits the focused runtime widget, never
        // the editor selection.
        if self.preview.is_some() {
            return self.preview_dispatch_key("Delete", false);
        }
        if self.apply_image_panel_delete() {
            return true;
        }
        // The shader source editor (#164) owns the keyboard while focused.
        if let Some(changed) = de_editor_core::shader_source_editor::delete_forward(
            &mut self.editor_state,
            self.now_ms,
        ) {
            if changed {
                self.mark_dirty();
            }
            return true;
        }
        // The open font picker owns Delete. Its search draft handles
        // Backspace separately; forward-delete must never reach the canvas
        // selection behind the overlay.
        if self.editor_state.editor_ui.font_picker.open {
            return true;
        }
        // Settings-modal input owns Delete while focused (forward
        // deletion at the caret) — without this arm the keystroke fell
        // through and removed the selected node behind the modal.
        if self.editor_state.editor_ui.agent_settings.focus.is_some() {
            return self.apply_settings_delete_forward();
        }
        if self.editor_state.editor_ui.variables_header_rename_active() {
            let changed =
                shared::variables_header_delete_forward(&mut self.editor_state, self.now_ms);
            if changed {
                self.mark_dirty();
            }
            return changed;
        }
        if let Some(changed) =
            preset_name::preset_name_delete_forward(&mut self.editor_state, self.now_ms)
        {
            if changed {
                self.mark_dirty();
            }
            return changed;
        }
        if self.editor_state.editor_ui.variable_row_focus.is_some() {
            let changed = shared::variable_row_delete_forward(&mut self.editor_state, self.now_ms);
            if changed {
                self.mark_dirty();
            }
            return changed;
        }
        // The rename draft has no forward deletion — Delete pops the
        // char before the caret, same as Backspace.
        if let Some(changed) = shared::rename_backspace(&mut self.editor_state, self.now_ms) {
            if changed {
                self.mark_dirty();
            }
            return changed;
        }
        // Delete is FORWARD deletion at the caret (or removes the
        // active selection) — textarea parity.
        if let Some(changed) = shared::text_edit_delete_forward(&mut self.editor_state, self.now_ms)
        {
            if changed {
                self.mark_dirty();
            }
            return changed;
        }
        if self.editor_state.ui.property_focus.is_some()
            || self.editor_state.editor_ui.effect_param_focus.is_some()
        {
            let changed =
                shared::property_input_delete_forward(&mut self.editor_state, self.now_ms);
            if changed {
                self.mark_dirty();
            }
            return changed;
        }
        if self.editor_state.chat.focused
            && self.editor_state.chat.delete_input_selection(self.now_ms)
        {
            self.mark_dirty();
            return true;
        }
        // Don't delete the selected node when a chrome text input or
        // search overlay owns the keyboard. Those branches ran above
        // (or deliberately swallow Delete); falling through here would
        // silently drop the node behind the focused field.
        if shared::delete_owned_by_chrome_input(&self.editor_state) {
            return false;
        }
        if shared::delete_selection_with_history(&mut self.editor_state) {
            self.mark_dirty();
            return true;
        }
        false
    }

    /// Cmd-D — duplicate selection as a sibling at +10 doc px.
    pub fn apply_duplicate(&mut self) -> bool {
        if self.input_active() {
            return false;
        }
        let dup = shared::duplicate_selection(&mut self.editor_state, &mut self.next_node_id);
        if dup {
            self.mark_dirty();
        }
        dup
    }
}
