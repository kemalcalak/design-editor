//! Press arms for the Agents tab's list and detail panes (#26): picking a
//! row, revealing the key, fetching models, re-probing a CLI agent,
//! ticking its models and shaping the ACP draft. Split out of
//! `agent_settings_press_entries.rs`, which keeps the field and
//! save/remove arms the old cards also had.

use de_editor_core::agent_settings::{
    AcpAgentField, AgentSettings, AgentsEntry, BuiltinAgentField,
};
use de_editor_core::host_settings_commit::{commit_settings_focus, SettingsCommitScope};
use de_editor_core::{BuiltinAgentPresetKey, BuiltinModelCatalogTarget, EditorState};

use crate::widgets::agent_settings_panel::AgentSettingsHit;
use crate::widgets::agent_settings_press_flow::SettingsPressOutcome;
use crate::widgets::agent_settings_press_focus::{
    clear_focus, focus_acp_agent_draft, focus_builtin_agent_draft,
};

/// Drop the drafts `keep` does not name, so the list never holds a
/// half-filled form the user walked away from.
fn cancel_other_drafts(settings: &mut AgentSettings, keep: &AgentsEntry) {
    if *keep != AgentsEntry::BuiltinDraft && settings.builtin_agent_draft.is_some() {
        settings.cancel_builtin_agent_draft();
    }
    if *keep != AgentsEntry::AcpDraft && settings.acp_agent_draft.is_some() {
        settings.cancel_acp_agent_draft();
    }
}

fn select(state: &mut EditorState, entry: AgentsEntry) {
    clear_focus(state);
    let settings = &mut state.editor_ui.agent_settings;
    cancel_other_drafts(settings, &entry);
    settings.select_agents_entry(entry);
}

/// Start (or retarget) the API draft on `preset`.
pub(crate) fn begin_api_draft(state: &mut EditorState, preset: BuiltinAgentPresetKey) {
    clear_focus(state);
    let settings = &mut state.editor_ui.agent_settings;
    cancel_other_drafts(settings, &AgentsEntry::BuiltinDraft);
    settings.begin_builtin_agent_draft();
    if settings
        .builtin_agent_draft
        .as_ref()
        .is_some_and(|draft| draft.preset != preset)
    {
        settings.set_builtin_agent_draft_preset(preset);
    }
}

/// Start the ACP draft.
pub(crate) fn begin_acp_draft(state: &mut EditorState) {
    clear_focus(state);
    let settings = &mut state.editor_ui.agent_settings;
    cancel_other_drafts(settings, &AgentsEntry::AcpDraft);
    settings.begin_acp_agent_draft();
}

/// The arms this module owns; `None` for every other hit.
pub(crate) fn apply_agents_hit(
    state: &mut EditorState,
    hit: AgentSettingsHit,
    scope: SettingsCommitScope,
    now_ms: u64,
) -> Option<SettingsPressOutcome> {
    let commit = |state: &mut EditorState| {
        commit_settings_focus(state, scope, now_ms);
    };
    match hit {
        AgentSettingsHit::SelectBuiltinEntry(index) => {
            commit(state);
            let id = state
                .editor_ui
                .agent_settings
                .builtin_agents
                .get(index)
                .map(|agent| agent.id.clone());
            if let Some(id) = id {
                select(state, AgentsEntry::Builtin(id));
            }
        }
        AgentSettingsHit::SelectApiPreset(preset) => {
            commit(state);
            begin_api_draft(state, preset);
            focus_builtin_agent_draft(state, BuiltinAgentField::ApiKey, now_ms);
        }
        AgentSettingsHit::SelectCliEntry(provider) => {
            commit(state);
            select(state, AgentsEntry::Cli(provider));
        }
        AgentSettingsHit::SelectAcpEntry(index) => {
            commit(state);
            let id = state
                .editor_ui
                .agent_settings
                .acp_agents
                .get(index)
                .map(|agent| agent.id.clone());
            if let Some(id) = id {
                select(state, AgentsEntry::Acp(id));
            }
        }
        AgentSettingsHit::ToggleApiKeyReveal => {
            let settings = &mut state.editor_ui.agent_settings;
            settings.api_key_revealed = !settings.api_key_revealed;
        }
        AgentSettingsHit::FetchBuiltinModels(index) => {
            commit(state);
            let settings = &mut state.editor_ui.agent_settings;
            let target = match index {
                Some(index) => settings
                    .builtin_agents
                    .get(index)
                    .map(|agent| BuiltinModelCatalogTarget::Agent(agent.id.clone())),
                None => Some(BuiltinModelCatalogTarget::Draft),
            };
            if let Some(target) = target {
                let _ = settings.force_builtin_model_catalog_refresh(target, now_ms);
            }
        }
        AgentSettingsHit::RecheckProvider(provider) => {
            commit(state);
            let settings = &mut state.editor_ui.agent_settings;
            if !settings.provider_probe_in_flight(provider) {
                settings.begin_provider_connect(provider);
            }
            state.rebuild_chat_models();
        }
        AgentSettingsHit::ToggleCliModel { provider, row } => {
            commit(state);
            let model = crate::widgets::agent_settings_cli_detail::cli_model_rows(
                &state.chat.discovered_models,
                provider,
            )
            .get(row)
            .map(|model| model.to_string());
            if let Some(model) = model {
                state
                    .editor_ui
                    .agent_settings
                    .toggle_cli_model_hidden(provider, &model);
                state.rebuild_chat_models();
            }
        }
        AgentSettingsHit::SelectAcpPreset(row) => {
            commit(state);
            clear_focus(state);
            let preset = row
                .and_then(|row| de_editor_core::acp_agent_presets::ACP_AGENT_PRESETS.get(row))
                .map(|preset| preset.id);
            state
                .editor_ui
                .agent_settings
                .fill_acp_draft_from_preset(preset);
        }
        AgentSettingsHit::SetAcpConnection { index, kind } => {
            commit(state);
            clear_focus(state);
            state
                .editor_ui
                .agent_settings
                .set_acp_connection_type(index, kind);
            state.rebuild_chat_models();
            if index.is_none() {
                let field = crate::widgets::agent_settings_press_focus::transport_field(kind);
                focus_acp_agent_draft(state, field, now_ms);
            }
        }
        AgentSettingsHit::AddProvider => {
            commit(state);
            begin_api_draft(state, BuiltinAgentPresetKey::Custom);
            focus_builtin_agent_draft(state, BuiltinAgentField::BaseUrl, now_ms);
        }
        AgentSettingsHit::AddAcpAgent => {
            commit(state);
            begin_acp_draft(state);
            focus_acp_agent_draft(state, AcpAgentField::Command, now_ms);
        }
        _ => return None,
    }
    Some(SettingsPressOutcome::handled())
}

#[cfg(test)]
mod tests {
    use super::*;
    use de_editor_core::agent_settings::AgentProvider;

    fn press(state: &mut EditorState, hit: AgentSettingsHit) {
        let _ = apply_agents_hit(state, hit, SettingsCommitScope::Browser, 10);
    }

    #[test]
    fn a_featured_preset_row_opens_a_draft_on_that_preset() {
        let mut state = EditorState::new();
        press(
            &mut state,
            AgentSettingsHit::SelectApiPreset(BuiltinAgentPresetKey::DeepSeek),
        );
        let settings = &state.editor_ui.agent_settings;
        assert_eq!(settings.agents_selected, Some(AgentsEntry::BuiltinDraft));
        assert_eq!(
            settings.builtin_agent_draft.as_ref().unwrap().preset,
            BuiltinAgentPresetKey::DeepSeek
        );
        assert_eq!(
            settings.focus,
            Some(
                de_editor_core::agent_settings::SettingsFocus::BuiltinAgentDraft(
                    BuiltinAgentField::ApiKey
                )
            )
        );
    }

    #[test]
    fn selecting_another_row_drops_the_drafts() {
        let mut state = EditorState::new();
        press(&mut state, AgentSettingsHit::AddProvider);
        assert_eq!(
            state
                .editor_ui
                .agent_settings
                .builtin_agent_draft
                .as_ref()
                .unwrap()
                .preset,
            BuiltinAgentPresetKey::Custom
        );
        press(
            &mut state,
            AgentSettingsHit::SelectCliEntry(AgentProvider::CodexCli),
        );
        let settings = &state.editor_ui.agent_settings;
        assert!(settings.builtin_agent_draft.is_none());
        assert!(settings.focus.is_none());
        assert_eq!(
            settings.agents_selected,
            Some(AgentsEntry::Cli(AgentProvider::CodexCli))
        );
    }

    #[test]
    fn unticking_a_cli_model_hides_it_from_the_picker() {
        let mut state = EditorState::new();
        state.chat.discovered_models = vec![de_editor_core::ModelEntry::new(
            AgentProvider::CodexCli,
            "gpt-5.5",
            "GPT-5.5",
        )];
        press(
            &mut state,
            AgentSettingsHit::ToggleCliModel {
                provider: AgentProvider::CodexCli,
                row: 0,
            },
        );
        assert!(state
            .editor_ui
            .agent_settings
            .cli_model_hidden(AgentProvider::CodexCli, "gpt-5.5"));
    }

    #[test]
    fn fetch_models_queues_a_refresh_for_a_ready_provider() {
        let mut state = EditorState::new();
        state
            .editor_ui
            .agent_settings
            .add_builtin_agent_with_defaults("A", "sk-a", "m");
        let _ = state
            .editor_ui
            .agent_settings
            .take_pending_builtin_model_catalog_refresh();
        press(&mut state, AgentSettingsHit::FetchBuiltinModels(Some(0)));
        assert!(state
            .editor_ui
            .agent_settings
            .take_pending_builtin_model_catalog_refresh()
            .is_some());
    }

    #[test]
    fn the_acp_draft_takes_a_preset_and_the_key_toggle_flips() {
        let mut state = EditorState::new();
        press(&mut state, AgentSettingsHit::AddAcpAgent);
        press(&mut state, AgentSettingsHit::SelectAcpPreset(Some(1)));
        assert_eq!(
            state
                .editor_ui
                .agent_settings
                .acp_agent_draft
                .as_ref()
                .unwrap()
                .command,
            "gemini"
        );
        press(&mut state, AgentSettingsHit::ToggleApiKeyReveal);
        assert!(state.editor_ui.agent_settings.api_key_revealed);
    }
}
