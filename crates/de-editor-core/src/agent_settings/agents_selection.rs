//! The Agents tab's list/detail selection and the per-CLI-agent hidden
//! model sets (#26). The tab lists API providers, the CLI agents on this
//! machine and ACP agents on the left; the right pane edits whichever
//! entry is selected here.

use super::{AcpConnectionType, AgentSettings};
use crate::acp_agent_presets::acp_agent_preset;
use crate::chat::AgentProvider;

/// One row of the Agents tab list. Built-in and ACP rows are keyed by id,
/// not index, so removing an earlier row never moves the selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentsEntry {
    Builtin(String),
    BuiltinDraft,
    Cli(AgentProvider),
    Acp(String),
    AcpDraft,
}

impl AgentSettings {
    /// Whether `entry` still names something on the list. CLI and ACP rows
    /// exist only where the tab shows external agents.
    fn agents_entry_exists(&self, entry: &AgentsEntry, external: bool) -> bool {
        match entry {
            AgentsEntry::Builtin(id) => self.builtin_agents.iter().any(|agent| agent.id == *id),
            AgentsEntry::BuiltinDraft => self.builtin_agent_draft.is_some(),
            AgentsEntry::Cli(_) => external,
            AgentsEntry::Acp(id) => external && self.acp_agents.iter().any(|agent| agent.id == *id),
            AgentsEntry::AcpDraft => external && self.acp_agent_draft.is_some(),
        }
    }

    /// The entry the detail pane shows: the stored selection while it still
    /// exists, else the first row (API providers, then CLI agents). `None`
    /// only on a list with nothing in it.
    pub fn resolved_agents_entry(&self, external: bool) -> Option<AgentsEntry> {
        if let Some(entry) = self
            .agents_selected
            .as_ref()
            .filter(|entry| self.agents_entry_exists(entry, external))
        {
            return Some(entry.clone());
        }
        if let Some(agent) = self.builtin_agents.first() {
            return Some(AgentsEntry::Builtin(agent.id.clone()));
        }
        if self.builtin_agent_draft.is_some() {
            return Some(AgentsEntry::BuiltinDraft);
        }
        external.then_some(AgentsEntry::Cli(AgentProvider::DISPLAY[0]))
    }

    /// Show `entry` in the detail pane. The key goes back under its mask
    /// and the old entry's dropdowns and checklist scroll reset with it.
    pub fn select_agents_entry(&mut self, entry: AgentsEntry) {
        self.agents_selected = Some(entry);
        self.api_key_revealed = false;
        self.builtin_model_menu_scroll.offset = 0.0;
        self.builtin_preset_menu_open = None;
        self.builtin_preset_menu_hover = None;
        self.builtin_model_menu_open = None;
        self.builtin_model_menu_hover = None;
    }

    /// Fill the ACP draft from a quick-add preset, or clear it back to a
    /// blank local command for `None` ("Custom"). The draft stays a draft:
    /// saving it goes through the ordinary add path.
    pub fn fill_acp_draft_from_preset(&mut self, preset_id: Option<&str>) {
        let preset = preset_id.and_then(acp_agent_preset);
        let blank_name = format!("ACP Agent {}", self.next_acp_agent_id.max(1));
        let Some(draft) = self.acp_agent_draft.as_mut() else {
            return;
        };
        draft.connection_type = AcpConnectionType::Local;
        draft.url = None;
        match preset {
            Some(preset) => {
                draft.display_name = preset.display_name.to_string();
                draft.command = preset.command.to_string();
                draft.args = preset.args.iter().map(|arg| arg.to_string()).collect();
            }
            None => {
                draft.display_name = blank_name;
                draft.command.clear();
                draft.args.clear();
            }
        }
    }

    /// Switch an ACP agent (`Some(index)`) or the draft between a local
    /// command and a remote URL. A saved agent drops its live connection,
    /// which belonged to the old transport.
    pub fn set_acp_connection_type(&mut self, index: Option<usize>, kind: AcpConnectionType) {
        let agent = match index {
            Some(index) => self.acp_agents.get_mut(index),
            None => self.acp_agent_draft.as_mut(),
        };
        let Some(agent) = agent else { return };
        if agent.connection_type == kind {
            return;
        }
        agent.connection_type = kind;
        let id = agent.id.clone();
        if index.is_some() {
            self.invalidate_acp_agent_connection(&id);
        }
    }

    /// Whether the user unticked `model` for a CLI agent (D6).
    pub fn cli_model_hidden(&self, provider: AgentProvider, model: &str) -> bool {
        self.cli_hidden_models[provider.index()].contains(model)
    }

    /// Tick or untick one of a CLI agent's models. Every model starts
    /// visible; only the unticked ones are stored.
    pub fn toggle_cli_model_hidden(&mut self, provider: AgentProvider, model: &str) {
        let hidden = &mut self.cli_hidden_models[provider.index()];
        if !hidden.remove(model) {
            hidden.insert(model.to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_builtin(settings: &mut AgentSettings, name: &str) -> String {
        settings.add_builtin_agent_with_defaults(name, "sk-test", "model-a")
    }

    #[test]
    fn an_empty_selection_falls_back_to_the_first_row() {
        let mut settings = AgentSettings::default();
        assert_eq!(settings.resolved_agents_entry(false), None);
        assert_eq!(
            settings.resolved_agents_entry(true),
            Some(AgentsEntry::Cli(AgentProvider::ClaudeCode))
        );
        let id = with_builtin(&mut settings, "One");
        assert_eq!(
            settings.resolved_agents_entry(true),
            Some(AgentsEntry::Builtin(id))
        );
    }

    #[test]
    fn removing_the_selected_provider_moves_to_the_first_row() {
        let mut settings = AgentSettings::default();
        let first = with_builtin(&mut settings, "One");
        let second = with_builtin(&mut settings, "Two");
        settings.select_agents_entry(AgentsEntry::Builtin(second));
        settings.remove_builtin_agent(1);
        assert_eq!(
            settings.resolved_agents_entry(true),
            Some(AgentsEntry::Builtin(first))
        );
    }

    #[test]
    fn external_rows_are_not_selectable_on_the_web_list() {
        let mut settings = AgentSettings::default();
        settings.select_agents_entry(AgentsEntry::Cli(AgentProvider::CodexCli));
        assert_eq!(settings.resolved_agents_entry(false), None);
        assert_eq!(
            settings.resolved_agents_entry(true),
            Some(AgentsEntry::Cli(AgentProvider::CodexCli))
        );
    }

    #[test]
    fn drafts_select_themselves_and_their_saved_row() {
        let mut settings = AgentSettings::default();
        settings.begin_builtin_agent_draft();
        assert_eq!(settings.agents_selected, Some(AgentsEntry::BuiltinDraft));
        settings.builtin_agent_draft.as_mut().unwrap().api_key = "sk-test".into();
        let id = settings.save_builtin_agent_draft().unwrap();
        assert_eq!(settings.agents_selected, Some(AgentsEntry::Builtin(id)));

        settings.begin_acp_agent_draft();
        assert_eq!(settings.agents_selected, Some(AgentsEntry::AcpDraft));
        let index = settings.add_acp_agent_preset("gemini-cli").unwrap();
        let id = settings.acp_agents[index].id.clone();
        assert_eq!(settings.agents_selected, Some(AgentsEntry::Acp(id)));
    }

    #[test]
    fn selecting_masks_the_key_again() {
        let mut settings = AgentSettings {
            api_key_revealed: true,
            ..AgentSettings::default()
        };
        settings.select_agents_entry(AgentsEntry::BuiltinDraft);
        assert!(!settings.api_key_revealed);
    }

    #[test]
    fn the_acp_draft_fills_from_a_preset_and_clears_for_custom() {
        let mut settings = AgentSettings::default();
        settings.begin_acp_agent_draft();
        settings.fill_acp_draft_from_preset(Some("qwen-code"));
        let draft = settings.acp_agent_draft.as_ref().unwrap();
        assert_eq!(draft.command, "qwen");
        assert_eq!(draft.args, ["--acp"]);
        settings.fill_acp_draft_from_preset(None);
        let draft = settings.acp_agent_draft.as_ref().unwrap();
        assert!(draft.command.is_empty() && draft.args.is_empty());
    }

    #[test]
    fn switching_a_saved_agent_to_remote_drops_its_connection() {
        let mut settings = AgentSettings::default();
        let index = settings.add_acp_agent_preset("kimi").unwrap();
        settings.acp_agents[index].connected = true;
        settings.set_acp_connection_type(Some(index), AcpConnectionType::Remote);
        assert_eq!(
            settings.acp_agents[index].connection_type,
            AcpConnectionType::Remote
        );
        assert!(!settings.acp_agents[index].connected);
    }

    #[test]
    fn cli_models_toggle_hidden_per_agent() {
        let mut settings = AgentSettings::default();
        settings.toggle_cli_model_hidden(AgentProvider::CodexCli, "gpt-5.5");
        assert!(settings.cli_model_hidden(AgentProvider::CodexCli, "gpt-5.5"));
        assert!(!settings.cli_model_hidden(AgentProvider::ClaudeCode, "gpt-5.5"));
        settings.toggle_cli_model_hidden(AgentProvider::CodexCli, "gpt-5.5");
        assert!(!settings.cli_model_hidden(AgentProvider::CodexCli, "gpt-5.5"));
    }
}
