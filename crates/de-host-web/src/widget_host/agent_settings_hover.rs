//! Agent-settings modal hover tracking on the web `WidgetHost` — the
//! per-card / per-row hover state the modal paints its washes from.
//!
//! Split out of `cursor_input.rs` to keep every file under the repo's
//! 800-line cap.

use de_editor_ui::Point2D;

use super::WidgetHost;

impl WidgetHost {
    /// Sync every agent-settings hover flag from the cursor.
    /// Returns `true` when any hover state changed.
    pub(in crate::widget_host) fn update_agent_settings_hover(&mut self, x: f32, y: f32) -> bool {
        use de_editor_core::AgentSettingsTab;
        use de_editor_ui::widgets::agent_settings_panel::{AgentSettingsHit, AgentSettingsPanel};
        let point = Point2D::new(x, y);
        let (
            close_hover,
            server_hover,
            copy_hover,
            image_search_test_hover,
            image_search_register_link_hover,
            image_add_hover,
            image_profile_header_hover,
            image_profile_remove_hover,
            image_profile_provider_hover,
            image_profile_test_hover,
            image_provider_option_hover,
            new_hover,
            new_model_hover,
            agents_row_hover,
        ) = {
            let panel = AgentSettingsPanel::for_web_editor(&self.editor_state);
            let panel_rect = panel.rect(self.last_viewport_w, self.last_viewport_h);
            let hit = panel.hit_test(panel_rect, point);
            let is_agents = matches!(
                self.editor_state.editor_ui.agent_settings.tab,
                AgentSettingsTab::Agents
            );
            let is_images = matches!(
                self.editor_state.editor_ui.agent_settings.tab,
                AgentSettingsTab::Images
            );
            let copy_hover = matches!(
                self.editor_state.editor_ui.agent_settings.tab,
                AgentSettingsTab::Mcp
            ) && matches!(hit, AgentSettingsHit::CopyMcpClientConfig);
            let close_hover = matches!(hit, AgentSettingsHit::Close);
            let server_hover = matches!(
                self.editor_state.editor_ui.agent_settings.tab,
                AgentSettingsTab::Mcp
            ) && matches!(hit, AgentSettingsHit::ToggleMcpServer);
            let image_search_test_hover =
                is_images && panel.image_search_test_button_hover_at(panel_rect, point);
            let image_search_register_link_hover =
                is_images && matches!(hit, AgentSettingsHit::OpenImageRegisterLink);
            let image_add_hover =
                is_images && panel.image_gen_add_button_hover_at(panel_rect, point);
            let image_profile_header_hover = if is_images {
                match hit {
                    AgentSettingsHit::ToggleGenConfigEditor(index) => Some(index),
                    _ => None,
                }
            } else {
                None
            };
            let image_profile_remove_hover = if is_images {
                match hit {
                    AgentSettingsHit::RemoveGenConfig(index) => Some(index),
                    _ => None,
                }
            } else {
                None
            };
            let image_profile_provider_hover = if is_images {
                match hit {
                    AgentSettingsHit::ToggleGenProviderMenu(index) => Some(index),
                    _ => None,
                }
            } else {
                None
            };
            let image_profile_test_hover = if is_images {
                panel.image_gen_profile_test_button_hover_at(panel_rect, point)
            } else {
                None
            };
            let image_provider_option_hover = if is_images {
                match hit {
                    AgentSettingsHit::SelectGenProvider { index, provider } => {
                        Some((index, provider))
                    }
                    _ => None,
                }
            } else {
                None
            };
            let new_hover = panel.builtin_preset_hover_at(panel_rect, point);
            let new_model_hover = panel.agents_checklist_hover_at(panel_rect, point);
            let agents_row_hover = if is_agents {
                panel.agents_row_at(panel_rect, point)
            } else {
                None
            };
            (
                close_hover,
                server_hover,
                copy_hover,
                image_search_test_hover,
                image_search_register_link_hover,
                image_add_hover,
                image_profile_header_hover,
                image_profile_remove_hover,
                image_profile_provider_hover,
                image_profile_test_hover,
                image_provider_option_hover,
                new_hover,
                new_model_hover,
                agents_row_hover,
            )
        };
        let mut changed = false;
        if agents_row_hover != self.editor_state.editor_ui.agent_settings.hover_agents_row {
            self.editor_state.editor_ui.agent_settings.hover_agents_row = agents_row_hover;
            changed = true;
        }
        if close_hover
            != self
                .editor_state
                .editor_ui
                .agent_settings
                .hover_agent_settings_close
        {
            self.editor_state
                .editor_ui
                .agent_settings
                .hover_agent_settings_close = close_hover;
            changed = true;
        }
        if server_hover
            != self
                .editor_state
                .editor_ui
                .agent_settings
                .hover_mcp_server_button
        {
            self.editor_state
                .editor_ui
                .agent_settings
                .hover_mcp_server_button = server_hover;
            changed = true;
        }
        if copy_hover
            != self
                .editor_state
                .editor_ui
                .agent_settings
                .hover_mcp_client_config_copy
        {
            self.editor_state
                .editor_ui
                .agent_settings
                .hover_mcp_client_config_copy = copy_hover;
            changed = true;
        }
        if new_hover
            != self
                .editor_state
                .editor_ui
                .agent_settings
                .builtin_preset_menu_hover
        {
            self.editor_state
                .editor_ui
                .agent_settings
                .builtin_preset_menu_hover = new_hover;
            changed = true;
        }
        if new_model_hover
            != self
                .editor_state
                .editor_ui
                .agent_settings
                .agents_checklist_hover
        {
            self.editor_state
                .editor_ui
                .agent_settings
                .agents_checklist_hover = new_model_hover;
            changed = true;
        }
        if image_search_test_hover
            != self
                .editor_state
                .editor_ui
                .agent_settings
                .hover_image_search_test_button
        {
            self.editor_state
                .editor_ui
                .agent_settings
                .hover_image_search_test_button = image_search_test_hover;
            changed = true;
        }
        if image_search_register_link_hover
            != self
                .editor_state
                .editor_ui
                .agent_settings
                .hover_image_search_register_link
        {
            self.editor_state
                .editor_ui
                .agent_settings
                .hover_image_search_register_link = image_search_register_link_hover;
            changed = true;
        }
        if image_add_hover
            != self
                .editor_state
                .editor_ui
                .agent_settings
                .hover_image_gen_add_button
        {
            self.editor_state
                .editor_ui
                .agent_settings
                .hover_image_gen_add_button = image_add_hover;
            changed = true;
        }
        if image_profile_header_hover
            != self
                .editor_state
                .editor_ui
                .agent_settings
                .hover_image_gen_profile_header
        {
            self.editor_state
                .editor_ui
                .agent_settings
                .hover_image_gen_profile_header = image_profile_header_hover;
            changed = true;
        }
        if image_profile_remove_hover
            != self
                .editor_state
                .editor_ui
                .agent_settings
                .hover_image_gen_profile_remove
        {
            self.editor_state
                .editor_ui
                .agent_settings
                .hover_image_gen_profile_remove = image_profile_remove_hover;
            changed = true;
        }
        if image_profile_provider_hover
            != self
                .editor_state
                .editor_ui
                .agent_settings
                .hover_image_gen_profile_provider
        {
            self.editor_state
                .editor_ui
                .agent_settings
                .hover_image_gen_profile_provider = image_profile_provider_hover;
            changed = true;
        }
        if image_profile_test_hover
            != self
                .editor_state
                .editor_ui
                .agent_settings
                .hover_image_gen_profile_test
        {
            self.editor_state
                .editor_ui
                .agent_settings
                .hover_image_gen_profile_test = image_profile_test_hover;
            changed = true;
        }
        if image_provider_option_hover
            != self
                .editor_state
                .editor_ui
                .agent_settings
                .hover_image_gen_provider_option
        {
            self.editor_state
                .editor_ui
                .agent_settings
                .hover_image_gen_provider_option = image_provider_option_hover;
            changed = true;
        }
        let (new_missing_hover, font_picker_hover, font_import_hover) = if matches!(
            self.editor_state.editor_ui.agent_settings.tab,
            AgentSettingsTab::Fonts
        ) {
            use de_editor_ui::widgets::agent_settings_fonts::FontsHit;
            let panel = AgentSettingsPanel::for_editor(&self.editor_state);
            let panel_rect = panel.rect(self.last_viewport_w, self.last_viewport_h);
            match panel.hit_test(panel_rect, point) {
                AgentSettingsHit::Fonts(FontsHit::ChooseFont(row)) => (
                    Some(de_editor_core::missing_fonts::MissingFontsHover::ChooseFile(row)),
                    None,
                    false,
                ),
                AgentSettingsHit::Fonts(FontsHit::RemoveImportedFont(row)) => (
                    Some(de_editor_core::missing_fonts::MissingFontsHover::RemoveImported(row)),
                    None,
                    false,
                ),
                AgentSettingsHit::Fonts(FontsHit::RemoveDownloadedFont(row)) => (
                    Some(de_editor_core::missing_fonts::MissingFontsHover::RemoveDownloaded(row)),
                    None,
                    false,
                ),
                AgentSettingsHit::Fonts(FontsHit::Download(row)) => (
                    Some(de_editor_core::missing_fonts::MissingFontsHover::Download(
                        row,
                    )),
                    None,
                    false,
                ),
                AgentSettingsHit::Fonts(FontsHit::SelectFont(index)) => (None, Some(index), false),
                AgentSettingsHit::Fonts(FontsHit::ImportFont(_)) => (None, None, true),
                _ => (None, None, false),
            }
        } else {
            (None, None, false)
        };
        if new_missing_hover != self.editor_state.editor_ui.missing_fonts_hover {
            self.editor_state.editor_ui.missing_fonts_hover = new_missing_hover;
            changed = true;
        }
        if font_picker_hover != self.editor_state.editor_ui.font_picker.hover {
            self.editor_state.editor_ui.font_picker.hover = font_picker_hover;
            changed = true;
        }
        if font_import_hover != self.editor_state.editor_ui.font_picker_import_hover {
            self.editor_state.editor_ui.font_picker_import_hover = font_import_hover;
            changed = true;
        }
        if changed {
            self.mark_dirty();
        }
        changed
    }
}
