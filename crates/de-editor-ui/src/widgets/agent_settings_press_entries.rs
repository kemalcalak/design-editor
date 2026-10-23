//! Built-in-agent / ACP-agent / image-gen-profile press arms of the
//! agent-settings modal — the list-entry half of
//! [`super::agent_settings_press_flow::apply_agent_settings_hit`].
//!
//! Split out of `agent_settings_press_flow.rs` to keep every file under
//! the repo's 800-line cap.

use de_editor_core::agent_settings::{
    AcpAgentField, AgentsEntry, BuiltinAgentField, BuiltinAgentPresetMenuTarget, ImageGenField,
    ImageTestStatus, SettingsFocus,
};
use de_editor_core::host_settings_commit::{commit_settings_focus, SettingsCommitScope};
use de_editor_core::EditorState;

use crate::widgets::agent_settings_panel::AgentSettingsHit;
use crate::widgets::agent_settings_press_flow::SettingsPressOutcome;
use crate::widgets::agent_settings_press_focus::*;

/// Built-in-agent / ACP-agent / image-gen-profile arms — the list-entry
/// half of the match, split out to keep both functions readable.
pub(crate) fn apply_entry_hit(
    state: &mut EditorState,
    hit: AgentSettingsHit,
    scope: SettingsCommitScope,
    now_ms: u64,
) -> SettingsPressOutcome {
    let commit = |state: &mut EditorState| {
        commit_settings_focus(state, scope, now_ms);
    };
    let take_over = matches!(scope, SettingsCommitScope::Operator);
    match hit {
        // ─── Image-generation profiles ─────────────────────────────
        AgentSettingsHit::SetActiveGenConfig(index) => {
            commit(state);
            if let Some(id) = image_gen_profile_id(state, index) {
                state
                    .editor_ui
                    .agent_settings
                    .set_active_image_gen_profile(&id);
            }
            SettingsPressOutcome::handled()
        }
        AgentSettingsHit::RemoveGenConfig(index) => {
            commit(state);
            if let Some(id) = image_gen_profile_id(state, index) {
                state.editor_ui.agent_settings.remove_image_gen_profile(&id);
            }
            SettingsPressOutcome::handled()
        }
        AgentSettingsHit::TestGenConfig(index) => {
            commit(state);
            if let Some(profile) = state
                .editor_ui
                .agent_settings
                .image_gen_profiles
                .get_mut(index)
            {
                profile.test_status = if profile.api_key.trim().is_empty() {
                    ImageTestStatus::Invalid
                } else {
                    ImageTestStatus::Testing
                };
            }
            SettingsPressOutcome::handled()
        }
        AgentSettingsHit::ToggleGenConfigEditor(index) => {
            let was_editing = matches!(
                state.editor_ui.agent_settings.focus,
                Some(SettingsFocus::ImageGenProfile { index: focused, .. }) if focused == index
            );
            commit(state);
            if !was_editing {
                focus_image_gen_profile(state, index, ImageGenField::Name, now_ms);
            }
            SettingsPressOutcome::handled()
        }
        AgentSettingsHit::AddGenConfig => {
            commit(state);
            let id = state.editor_ui.agent_settings.add_image_gen_profile();
            let index = state
                .editor_ui
                .agent_settings
                .image_gen_profiles
                .iter()
                .position(|profile| profile.id == id)
                .unwrap_or(0);
            focus_image_gen_profile(state, index, ImageGenField::Name, now_ms);
            SettingsPressOutcome::handled()
        }
        AgentSettingsHit::ToggleGenProviderMenu(index) => {
            commit(state);
            let settings = &mut state.editor_ui.agent_settings;
            settings.image_gen_provider_menu_open =
                (settings.image_gen_provider_menu_open != Some(index)).then_some(index);
            focus_image_gen_profile(state, index, ImageGenField::Name, now_ms);
            SettingsPressOutcome::handled()
        }
        AgentSettingsHit::SelectGenProvider { index, provider: _ } => {
            commit(state);
            focus_image_gen_profile(state, index, ImageGenField::Name, now_ms);
            SettingsPressOutcome::handled()
        }
        AgentSettingsHit::FocusGenConfig { index, field } => {
            commit(state);
            state.editor_ui.agent_settings.image_gen_provider_menu_open = None;
            focus_image_gen_profile(state, index, field, now_ms);
            SettingsPressOutcome::handled()
        }
        // ─── Built-in (API-key) agents ─────────────────────────────
        AgentSettingsHit::FocusBuiltinAgent { index, field } => {
            commit(state);
            focus_builtin_agent(state, index, field, now_ms);
            SettingsPressOutcome::handled()
        }
        AgentSettingsHit::FocusBuiltinAgentDraft(field) => {
            commit(state);
            focus_builtin_agent_draft(state, field, now_ms);
            SettingsPressOutcome::handled()
        }
        AgentSettingsHit::ToggleBuiltinAgentKind(index) => {
            commit(state);
            if take_over {
                state
                    .editor_ui
                    .agent_settings
                    .take_over_browser_builtin_agent(index);
            }
            if let Some(agent) = state.editor_ui.agent_settings.builtin_agents.get_mut(index) {
                agent.toggle_kind_for_preset();
                state.rebuild_chat_models();
            }
            queue_builtin_discovery(state, index, now_ms);
            SettingsPressOutcome::handled()
        }
        AgentSettingsHit::ToggleBuiltinAgentDraftKind => {
            commit(state);
            if let Some(agent) = state.editor_ui.agent_settings.builtin_agent_draft.as_mut() {
                agent.toggle_kind_for_preset();
            }
            SettingsPressOutcome::handled()
        }
        AgentSettingsHit::ToggleBuiltinModel { index, row } => {
            // Resolve the pressed row against the same rows the checklist
            // painted — hit-test and press run on the same frame, so the
            // row index is stable. Resolve before commit because an operator
            // takeover may rotate a browser-owned provider id and invalidate
            // its old runtime catalog.
            let model_id = {
                let settings = &state.editor_ui.agent_settings;
                let entry = match index {
                    Some(index) => settings
                        .builtin_agents
                        .get(index)
                        .map(|agent| AgentsEntry::Builtin(agent.id.clone())),
                    None => Some(AgentsEntry::BuiltinDraft),
                };
                entry
                    .and_then(|entry| {
                        crate::widgets::agent_settings_api_detail::api_target(settings, &entry)
                    })
                    .and_then(|target| {
                        crate::widgets::agent_settings_api_detail::model_rows(settings, target)
                            .into_iter()
                            .nth(row)
                            .map(str::to_string)
                    })
            };
            commit(state);
            if let Some(model_id) = model_id {
                let settings = &mut state.editor_ui.agent_settings;
                let agent = match index {
                    Some(index) => settings.builtin_agents.get_mut(index),
                    None => settings.builtin_agent_draft.as_mut(),
                };
                if let Some(agent) = agent {
                    agent.toggle_model(&model_id);
                }
                if index.is_some() {
                    state.rebuild_chat_models();
                }
            }
            SettingsPressOutcome::handled()
        }
        AgentSettingsHit::ToggleBuiltinAgentPresetMenu(index) => {
            commit(state);
            let target = match index {
                Some(index) => BuiltinAgentPresetMenuTarget::Agent(index),
                None => BuiltinAgentPresetMenuTarget::Draft,
            };
            let settings = &mut state.editor_ui.agent_settings;
            settings.agents_checklist_scroll.offset = 0.0;
            settings.agents_checklist_hover = None;
            settings.builtin_preset_menu_open =
                (settings.builtin_preset_menu_open != Some(target)).then_some(target);
            settings.builtin_preset_menu_scroll.offset = 0.0;
            settings.builtin_preset_menu_hover = None;
            SettingsPressOutcome::handled()
        }
        AgentSettingsHit::SelectBuiltinAgentPreset { index, preset } => {
            commit(state);
            match index {
                Some(index) => {
                    if take_over {
                        state
                            .editor_ui
                            .agent_settings
                            .take_over_browser_builtin_agent(index);
                    }
                    state
                        .editor_ui
                        .agent_settings
                        .set_builtin_agent_preset(index, preset);
                    state.rebuild_chat_models();
                    queue_builtin_discovery(state, index, now_ms);
                }
                None => state
                    .editor_ui
                    .agent_settings
                    .set_builtin_agent_draft_preset(preset),
            }
            let settings = &mut state.editor_ui.agent_settings;
            settings.builtin_preset_menu_open = None;
            settings.builtin_preset_menu_scroll.offset = 0.0;
            settings.builtin_preset_menu_hover = None;
            SettingsPressOutcome::handled()
        }
        AgentSettingsHit::ToggleBuiltinAgentEnabled(index) => {
            commit(state);
            if take_over {
                state
                    .editor_ui
                    .agent_settings
                    .take_over_browser_builtin_agent(index);
            }
            if let Some(agent) = state.editor_ui.agent_settings.builtin_agents.get_mut(index) {
                agent.enabled = !agent.enabled;
                state.rebuild_chat_models();
            }
            queue_builtin_discovery(state, index, now_ms);
            SettingsPressOutcome::handled()
        }
        AgentSettingsHit::SaveBuiltinAgentEditing(index) => {
            // Commit any focused field, then drop focus; a fresh key or
            // base URL warrants a model catalog refresh, mirroring the
            // draft save.
            commit(state);
            queue_builtin_discovery(state, index, now_ms);
            clear_focus(state);
            state.rebuild_chat_models();
            SettingsPressOutcome::handled()
        }
        AgentSettingsHit::RemoveBuiltinAgent(index) => {
            commit(state);
            if state
                .editor_ui
                .agent_settings
                .remove_builtin_agent(index)
                .is_some()
            {
                clear_focus(state);
                state.rebuild_chat_models();
            }
            SettingsPressOutcome::handled()
        }
        AgentSettingsHit::SaveBuiltinAgentDraft => {
            commit(state);
            if let Some(id) = state.editor_ui.agent_settings.save_builtin_agent_draft() {
                let _ = state
                    .editor_ui
                    .agent_settings
                    .begin_builtin_model_catalog_refresh(
                        de_editor_core::BuiltinModelCatalogTarget::Agent(id),
                        now_ms,
                    );
                clear_focus(state);
                state.rebuild_chat_models();
            } else {
                // Rejected draft keeps the form open, parked on the field
                // that must be filled in.
                focus_builtin_agent_draft(state, BuiltinAgentField::ApiKey, now_ms);
            }
            SettingsPressOutcome::handled()
        }
        AgentSettingsHit::CancelBuiltinAgentDraft => {
            state.editor_ui.agent_settings.cancel_builtin_agent_draft();
            clear_focus(state);
            SettingsPressOutcome::handled()
        }
        // ─── ACP agents ────────────────────────────────────────────
        AgentSettingsHit::FocusAcpAgent { index, field } => {
            commit(state);
            focus_acp_agent(state, index, field, now_ms);
            SettingsPressOutcome::handled()
        }
        AgentSettingsHit::FocusAcpAgentDraft(field) => {
            commit(state);
            focus_acp_agent_draft(state, field, now_ms);
            SettingsPressOutcome::handled()
        }
        AgentSettingsHit::RemoveAcpAgent(index) => {
            commit(state);
            let id = state
                .editor_ui
                .agent_settings
                .acp_agents
                .get(index)
                .map(|agent| agent.id.clone());
            if let Some(id) = id {
                state.editor_ui.agent_settings.remove_acp_agent(&id);
                clear_focus(state);
                state.rebuild_chat_models();
            }
            SettingsPressOutcome::handled()
        }
        AgentSettingsHit::ToggleAcpConnected(index) => {
            commit(state);
            let settings = &state.editor_ui.agent_settings;
            // An agent that was never configured focuses its transport
            // field instead of starting a doomed probe.
            let needs_config_focus = settings.acp_agents.get(index).is_some_and(|agent| {
                !settings.acp_agent_verified_connected(&agent.id) && !agent.ready()
            });
            if needs_config_focus {
                let field = state
                    .editor_ui
                    .agent_settings
                    .acp_agents
                    .get(index)
                    .map(|agent| transport_field(agent.connection_type));
                if let Some(field) = field {
                    focus_acp_agent(state, index, field, now_ms);
                }
            } else if state
                .editor_ui
                .agent_settings
                .acp_agent_verified_connected_at(index)
            {
                state.editor_ui.agent_settings.disconnect_acp_agent(index);
                state.rebuild_chat_models();
            } else if state
                .editor_ui
                .agent_settings
                .begin_acp_agent_connect(index)
                .is_some()
            {
                state.rebuild_chat_models();
            }
            SettingsPressOutcome::handled()
        }
        AgentSettingsHit::SaveAcpAgentDraft => {
            commit(state);
            if state
                .editor_ui
                .agent_settings
                .save_acp_agent_draft()
                .is_some()
            {
                clear_focus(state);
                // "Add agent" goes straight into the handshake, like the
                // old quick-add rows did.
                let index = state.editor_ui.agent_settings.acp_agents.len() - 1;
                if state
                    .editor_ui
                    .agent_settings
                    .begin_acp_agent_connect(index)
                    .is_some()
                {
                    state.rebuild_chat_models();
                }
            } else {
                let field = state
                    .editor_ui
                    .agent_settings
                    .acp_agent_draft
                    .as_ref()
                    .map(|agent| transport_field(agent.connection_type))
                    .unwrap_or(AcpAgentField::Command);
                focus_acp_agent_draft(state, field, now_ms);
            }
            SettingsPressOutcome::handled()
        }
        AgentSettingsHit::CancelAcpAgentDraft => {
            state.editor_ui.agent_settings.cancel_acp_agent_draft();
            clear_focus(state);
            SettingsPressOutcome::handled()
        }
        // Every remaining variant is handled by the caller. Spelled out
        // rather than `_` so a new hit variant is a compile error in one
        // of the two halves instead of a silent no-op.
        // The list and detail arms (#26).
        AgentSettingsHit::AddProvider
        | AgentSettingsHit::AddAcpAgent
        | AgentSettingsHit::SelectBuiltinEntry(_)
        | AgentSettingsHit::SelectApiPreset(_)
        | AgentSettingsHit::SelectCliEntry(_)
        | AgentSettingsHit::SelectAcpEntry(_)
        | AgentSettingsHit::ToggleApiKeyReveal
        | AgentSettingsHit::FetchBuiltinModels(_)
        | AgentSettingsHit::RecheckProvider(_)
        | AgentSettingsHit::ToggleCliModel { .. }
        | AgentSettingsHit::SelectAcpPreset(_)
        | AgentSettingsHit::SetAcpConnection { .. } => {
            crate::widgets::agent_settings_press_agents::apply_agents_hit(state, hit, scope, now_ms)
                .unwrap_or_else(SettingsPressOutcome::handled)
        }
        AgentSettingsHit::Close
        | AgentSettingsHit::Outside
        | AgentSettingsHit::Inside
        | AgentSettingsHit::SelectTab(_)
        | AgentSettingsHit::Connect(_)
        | AgentSettingsHit::ToggleMcpServer
        | AgentSettingsHit::ToggleMcpCli(_)
        | AgentSettingsHit::CopyMcpClientConfig
        | AgentSettingsHit::ExpandMcpCli(_)
        | AgentSettingsHit::CopyMcpSnippet(_)
        | AgentSettingsHit::ToggleMcpShowAll
        | AgentSettingsHit::Fonts(_)
        | AgentSettingsHit::ToggleImagesAdvanced
        | AgentSettingsHit::FocusSearchField(_)
        | AgentSettingsHit::OpenImageRegisterLink
        | AgentSettingsHit::TestImageSearch
        | AgentSettingsHit::ToggleAutoUpdate
        | AgentSettingsHit::UpdateButton
        | AgentSettingsHit::SelectPencilCursor(_)
        | AgentSettingsHit::SelectThemeMode(_)
        | AgentSettingsHit::ToggleExperimental
        | AgentSettingsHit::FocusMcpPort => {
            debug_assert!(false, "handled by apply_agent_settings_hit");
            SettingsPressOutcome::handled()
        }
    }
}

fn queue_builtin_discovery(state: &mut EditorState, index: usize, now_ms: u64) {
    let target = state
        .editor_ui
        .agent_settings
        .builtin_agents
        .get(index)
        .map(|agent| de_editor_core::BuiltinModelCatalogTarget::Agent(agent.id.clone()));
    if let Some(target) = target {
        let _ = state
            .editor_ui
            .agent_settings
            .begin_builtin_model_catalog_refresh(target, now_ms);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saving_the_editing_form_commits_the_draft_and_collapses_it() {
        let mut state = EditorState::new();
        state.editor_ui.agent_settings.begin_builtin_agent_draft();
        state
            .editor_ui
            .agent_settings
            .builtin_agent_draft
            .as_mut()
            .expect("draft")
            .api_key = "sk-old".into();
        let _ = apply_entry_hit(
            &mut state,
            AgentSettingsHit::SaveBuiltinAgentDraft,
            SettingsCommitScope::Browser,
            55,
        );
        // Type a new key into the saved provider.
        focus_builtin_agent(&mut state, 0, BuiltinAgentField::ApiKey, 57);
        state.editor_ui.settings_input.set_text("sk-rotated");

        let outcome = apply_entry_hit(
            &mut state,
            AgentSettingsHit::SaveBuiltinAgentEditing(0),
            SettingsCommitScope::Browser,
            58,
        );

        assert_eq!(outcome, SettingsPressOutcome::handled());
        assert_eq!(
            state.editor_ui.agent_settings.builtin_agents[0].api_key,
            "sk-rotated"
        );
        // Save drops the field focus.
        assert!(state.editor_ui.agent_settings.focus.is_none());
    }

    #[test]
    fn saving_a_ready_builtin_draft_queues_its_first_model_discovery() {
        let mut state = EditorState::new();
        state.editor_ui.agent_settings.begin_builtin_agent_draft();
        state
            .editor_ui
            .agent_settings
            .builtin_agent_draft
            .as_mut()
            .expect("draft")
            .api_key = "sk-new".into();

        let outcome = apply_entry_hit(
            &mut state,
            AgentSettingsHit::SaveBuiltinAgentDraft,
            SettingsCommitScope::Browser,
            55,
        );

        assert_eq!(outcome, SettingsPressOutcome::handled());
        let saved_id = state
            .editor_ui
            .agent_settings
            .builtin_agents
            .first()
            .expect("saved provider")
            .id
            .clone();
        let request = state
            .editor_ui
            .agent_settings
            .take_pending_builtin_model_catalog_refresh()
            .expect("save queues discovery");
        assert_eq!(
            request.target,
            de_editor_core::BuiltinModelCatalogTarget::Agent(saved_id)
        );
    }

    #[test]
    fn key_commit_followed_by_kind_toggle_requeues_for_the_new_endpoint() {
        let mut state = EditorState::new();
        let id = state.editor_ui.agent_settings.add_builtin_agent_config(
            "MiniMax",
            "",
            "MiniMax-M3",
            de_editor_core::BuiltinAgentKind::Anthropic,
            "https://api.minimaxi.com/anthropic",
        );
        state.editor_ui.agent_settings.focus = Some(SettingsFocus::BuiltinAgent {
            index: 0,
            field: BuiltinAgentField::ApiKey,
        });
        state.editor_ui.settings_input.set_text("sk-new");

        apply_entry_hit(
            &mut state,
            AgentSettingsHit::ToggleBuiltinAgentKind(0),
            SettingsCommitScope::Browser,
            77,
        );

        let request = state
            .editor_ui
            .agent_settings
            .take_pending_builtin_model_catalog_refresh()
            .expect("new kind must retain an immediate discovery request");
        assert_eq!(
            request.target,
            de_editor_core::BuiltinModelCatalogTarget::Agent(id)
        );
        let config = state
            .editor_ui
            .agent_settings
            .builtin_model_catalog_config_for_request(&request)
            .expect("request matches the post-toggle configuration");
        assert_eq!(config.kind, de_editor_core::BuiltinAgentKind::OpenAiCompat);
    }
}
