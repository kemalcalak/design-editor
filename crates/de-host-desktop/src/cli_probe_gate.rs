//! On-demand start of the desktop's agent-CLI probes.
//!
//! Two background jobs spawn the user's agent CLIs (`codex app-server`,
//! `claude`, …), and those CLIs may call their own vendors' APIs: model
//! discovery (`ModelProbe`) and the silent reconnect replay for providers
//! remembered in `agents.json`. Neither runs at launch. Both start once per
//! session, from the redraw pump, the first time the user opens a surface
//! that shows or uses CLI provider state — the chat model picker, Settings →
//! Agents, or the chat composer
//! ([`de_editor_core::EditorState::wants_cli_provider_state`]). A session
//! that never touches agents or chat never spawns a CLI.

use de_editor_core::agent_settings::{AgentSettings, ProviderConnectPhase};
use de_editor_core::AgentProvider;

use crate::DesktopApp;

impl DesktopApp {
    /// Start model discovery and the reconnect replay if this is the first
    /// frame that shows CLI provider state. Returns true when it started
    /// them on this call.
    pub(crate) fn start_cli_probes_on_demand(&mut self) -> bool {
        if self.cli_probes_started || !self.host.editor_state().wants_cli_provider_state() {
            return false;
        }
        self.cli_probes_started = true;
        // Spawn discovery BEFORE the replay: `begin_provider_connect`
        // clears the provider's `connected` flag while its probe runs, and
        // discovery must see last session's mask. Unit tests never spawn
        // real CLIs, and a probe that is somehow already running is kept.
        if !cfg!(test) && !self.model_probe.is_pending() {
            let connected = self.host.editor_state().editor_ui.agent_settings.connected;
            self.model_probe =
                de_host_services::model_discovery::ModelProbe::spawn_for_connected(connected);
        }
        self.restore_remembered_connections();
        true
    }

    /// Replay one silent connect probe per provider remembered as
    /// connected last session. The first probe starts immediately; the
    /// rest ride the landing hook in `provider_probe_host`, one at a time.
    ///
    /// A provider whose card is no longer `Idle` (the user already pressed
    /// Connect on it) is skipped, and when an explicit probe is already
    /// requested or running every remembered provider is queued behind it
    /// instead — `begin_provider_connect` would otherwise overwrite the
    /// single request seam and strand the user's card in `Probing`.
    pub(crate) fn restore_remembered_connections(&mut self) {
        let settings = &self.host.editor_state().editor_ui.agent_settings;
        let mut remembered: Vec<AgentProvider> = AgentProvider::ALL
            .iter()
            .copied()
            .filter(|provider| {
                let index = AgentSettings::provider_index(*provider);
                self.remembered_connections[index]
                    && settings.provider_connection[index].phase == ProviderConnectPhase::Idle
            })
            .collect();
        if remembered.is_empty() {
            return;
        }
        if self.provider_connect_pending() {
            remembered.reverse();
            self.provider_reconnect_queue = remembered;
            return;
        }
        let first = remembered.remove(0);
        remembered.reverse();
        self.provider_reconnect_queue = remembered;
        // `begin_provider_connect`, not a bare write to the request seam:
        // the landing hook drops any outcome whose card is not in the
        // Probing phase, so a seam-only request ran a real probe whose
        // result was then discarded — and with it the hook that advances
        // this queue, stranding every provider after the first.
        self.host
            .editor_state_mut()
            .editor_ui
            .agent_settings
            .begin_provider_connect(first);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use de_editor_core::agent_settings::AgentSettingsTab;

    fn fresh_app() -> DesktopApp {
        let mut app = DesktopApp::new(None);
        let es = app.host.editor_state_mut();
        es.editor_ui.agent_settings.connected = [false; 7];
        es.editor_ui.agent_settings.provider_connection = Default::default();
        es.editor_ui.agent_settings.pending_provider_connect = None;
        es.editor_ui.agent_settings_open = false;
        es.editor_ui.close_chat_model_picker();
        app.remembered_connections = [false; 7];
        app
    }

    fn remember(app: &mut DesktopApp, provider: AgentProvider) {
        app.remembered_connections[AgentSettings::provider_index(provider)] = true;
    }

    fn phase(app: &DesktopApp, provider: AgentProvider) -> ProviderConnectPhase {
        app.host
            .editor_state()
            .editor_ui
            .agent_settings
            .provider_connection[AgentSettings::provider_index(provider)]
        .phase
    }

    fn open_agents_tab(app: &mut DesktopApp) {
        let ui = &mut app.host.editor_state_mut().editor_ui;
        ui.agent_settings.tab = AgentSettingsTab::Agents;
        ui.agent_settings_open = true;
    }

    #[test]
    fn launch_starts_no_cli_probe() {
        let mut app = fresh_app();
        remember(&mut app, AgentProvider::ClaudeCode);

        assert!(!app.model_probe.is_pending(), "discovery is idle at launch");
        assert!(!app.start_cli_probes_on_demand());
        assert!(!app.cli_probes_started);
        assert_eq!(
            phase(&app, AgentProvider::ClaudeCode),
            ProviderConnectPhase::Idle
        );
        assert!(!app.provider_connect_pending());
    }

    #[test]
    fn opening_the_agents_tab_starts_the_replay_exactly_once() {
        let mut app = fresh_app();
        remember(&mut app, AgentProvider::ClaudeCode);
        remember(&mut app, AgentProvider::OpenCode);
        open_agents_tab(&mut app);

        assert!(app.start_cli_probes_on_demand());
        assert!(app.cli_probes_started);
        let settings = &app.host.editor_state().editor_ui.agent_settings;
        assert_eq!(
            settings.pending_provider_connect,
            Some(AgentProvider::ClaudeCode)
        );
        assert_eq!(
            phase(&app, AgentProvider::ClaudeCode),
            ProviderConnectPhase::Probing
        );
        assert_eq!(app.provider_reconnect_queue, vec![AgentProvider::OpenCode]);

        // Closing and reopening must not replay again this session.
        app.host.editor_state_mut().editor_ui.agent_settings_open = false;
        open_agents_tab(&mut app);
        assert!(!app.start_cli_probes_on_demand());
        assert_eq!(app.provider_reconnect_queue, vec![AgentProvider::OpenCode]);
    }

    #[test]
    fn opening_the_chat_model_picker_also_starts_the_replay() {
        let mut app = fresh_app();
        remember(&mut app, AgentProvider::CodexCli);
        app.host
            .editor_state_mut()
            .editor_ui
            .toggle_chat_model_picker();

        assert!(app.start_cli_probes_on_demand());
        assert_eq!(
            phase(&app, AgentProvider::CodexCli),
            ProviderConnectPhase::Probing
        );
    }

    #[test]
    fn an_explicit_connect_is_neither_replaced_nor_probed_twice() {
        let mut app = fresh_app();
        remember(&mut app, AgentProvider::ClaudeCode);
        remember(&mut app, AgentProvider::CodexCli);
        // The user pressed Connect on Claude Code before the gate's frame.
        app.host
            .editor_state_mut()
            .editor_ui
            .agent_settings
            .begin_provider_connect(AgentProvider::ClaudeCode);
        open_agents_tab(&mut app);

        assert!(app.start_cli_probes_on_demand());
        let settings = &app.host.editor_state().editor_ui.agent_settings;
        assert_eq!(
            settings.pending_provider_connect,
            Some(AgentProvider::ClaudeCode),
            "the user's own request keeps the seam"
        );
        assert_eq!(
            app.provider_reconnect_queue,
            vec![AgentProvider::CodexCli],
            "Claude Code is already probing, so only Codex queues behind it"
        );
        assert_eq!(
            phase(&app, AgentProvider::CodexCli),
            ProviderConnectPhase::Idle
        );
    }
}
