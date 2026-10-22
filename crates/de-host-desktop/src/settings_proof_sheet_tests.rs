//! Proof sheets for the chat panel and Settings > Agents redesign (#26):
//! render their main states, with the bundled fonts, to PNGs for a visual
//! check.
//! Ignored by default because they write files; run with
//!
//!   DE_PROOF_DIR=/tmp/proof cargo test -p de-host-desktop settings_proof -- --ignored
//!
//! (without `DE_PROOF_DIR` the sheets land in the system temp directory).

use de_editor_core::agent_settings::{AgentProvider, AgentsEntry, ProviderConnectPhase};
use de_editor_core::editor_ui_state::ThemeMode;
use de_editor_core::{
    BuiltinAgentKind, BuiltinAgentPresetKey, BuiltinModelCatalogRefreshOutcome,
    BuiltinModelCatalogTarget, BuiltinModelOption, ModelEntry,
};
use de_host_native::{NativeBackend, NativeFrameBackend, WidgetHostNative};

const W: f32 = 1440.0;
const H: f32 = 900.0;
const DPI: f32 = 2.0;

fn host(theme: ThemeMode) -> WidgetHostNative {
    crate::bundled_fonts::register();
    let mut host = WidgetHostNative::new();
    let ui = &mut host.editor_state_mut().editor_ui;
    ui.locale = de_i18n::Locale::EnUs;
    ui.theme_mode = theme;
    ui.home.hide();
    ui.agent_settings_open = true;
    ui.agent_settings.tab = de_editor_core::AgentSettingsTab::Agents;
    host
}

fn shoot(host: &mut WidgetHostNative, name: &str) {
    let (pw, ph) = ((W * DPI) as i32, (H * DPI) as i32);
    let mut surface = skia_safe::surfaces::raster_n32_premul((pw, ph)).expect("raster surface");
    let mut backend = NativeBackend::with_dpi(DPI);
    {
        let canvas = surface.canvas();
        canvas.clear(skia_safe::Color::BLACK);
        canvas.scale((DPI, DPI));
        let mut frame = NativeFrameBackend::new(&mut backend, canvas);
        host.paint(&mut frame, W, H);
    }
    let data = surface
        .image_snapshot()
        .encode(None, skia_safe::EncodedImageFormat::PNG, 100)
        .expect("encode png");
    let dir = std::env::var_os("DE_PROOF_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    std::fs::create_dir_all(&dir).expect("proof dir");
    let out = dir.join(format!("{name}.png"));
    std::fs::write(&out, data.as_bytes()).expect("write png");
    println!("proof sheet written to {}", out.display());
}

fn with_gateway(host: &mut WidgetHostNative) {
    let settings = &mut host.editor_state_mut().editor_ui.agent_settings;
    let id = settings.add_builtin_agent_config(
        "SeriAI gateway",
        "sk-proof-1234567890abcdef3f9a",
        "gpt-5",
        BuiltinAgentKind::OpenAiCompat,
        "https://gateway.example/v1",
    );
    settings.builtin_agents[0].preset = BuiltinAgentPresetKey::Custom;
    settings.builtin_agents[0].toggle_model("claude-sonnet-5-5");
    settings.builtin_agents[0].toggle_model("qwen3-coder");
    let request = settings
        .begin_builtin_model_catalog_refresh(BuiltinModelCatalogTarget::Agent(id.clone()), 0)
        .expect("ready provider refreshes");
    let config = settings
        .builtin_model_catalog_config_for_request(&request)
        .expect("config");
    settings.apply_builtin_model_catalog_refresh_outcome_if_current(
        &config,
        &request,
        BuiltinModelCatalogRefreshOutcome::Success {
            models: [
                "gpt-5",
                "claude-sonnet-5-5",
                "qwen3-coder",
                "text-embedding-3",
            ]
            .iter()
            .map(|id| BuiltinModelOption::new(*id, *id))
            .collect(),
        },
    );
    settings.select_agents_entry(AgentsEntry::Builtin(id));
}

fn with_claude_connected(host: &mut WidgetHostNative) {
    let state = host.editor_state_mut();
    let index = AgentProvider::ClaudeCode.index();
    state.editor_ui.agent_settings.connected[index] = true;
    let connection = &mut state.editor_ui.agent_settings.provider_connection[index];
    connection.phase = ProviderConnectPhase::Connected;
    connection.info = Some("Signed in · checked just now".into());
    connection.version = Some("v2.1.4".into());
    state.chat.discovered_models = ["claude-opus-5-5", "claude-sonnet-5-5", "claude-haiku-5-5"]
        .iter()
        .map(|id| ModelEntry::new(AgentProvider::ClaudeCode, *id, *id))
        .collect();
    state
        .editor_ui
        .agent_settings
        .toggle_cli_model_hidden(AgentProvider::ClaudeCode, "claude-haiku-5-5");
}

#[test]
#[ignore]
fn settings_proof_sheets() {
    for (theme, suffix) in [(ThemeMode::Dark, "dark"), (ThemeMode::Light, "light")] {
        let mut h = host(theme);
        with_claude_connected(&mut h);
        with_gateway(&mut h);
        shoot(&mut h, &format!("agents-api-{suffix}"));

        h.editor_state_mut()
            .editor_ui
            .agent_settings
            .select_agents_entry(AgentsEntry::Cli(AgentProvider::ClaudeCode));
        shoot(&mut h, &format!("agents-cli-{suffix}"));

        let settings = &mut h.editor_state_mut().editor_ui.agent_settings;
        settings.begin_acp_agent_draft();
        settings.fill_acp_draft_from_preset(Some("qwen-code"));
        settings
            .acp_preset_installed
            .insert("qwen-code".to_string(), false);
        shoot(&mut h, &format!("agents-acp-{suffix}"));

        let settings = &mut h.editor_state_mut().editor_ui.agent_settings;
        settings.cancel_acp_agent_draft();
        settings.begin_builtin_agent_draft();
        settings.set_builtin_agent_draft_preset(BuiltinAgentPresetKey::Custom);
        settings.builtin_preset_menu_open =
            Some(de_editor_core::agent_settings::BuiltinAgentPresetMenuTarget::Draft);
        shoot(&mut h, &format!("agents-custom-{suffix}"));
    }
}

fn chat_host(theme: ThemeMode) -> WidgetHostNative {
    let mut h = host(theme);
    with_claude_connected(&mut h);
    with_gateway(&mut h);
    let state = h.editor_state_mut();
    state.editor_ui.agent_settings_open = false;
    state.editor_ui.slides_panel.tab = de_editor_core::editor_ui_state::LeftPanelTab::Chat;
    state.rebuild_chat_models();
    h
}

#[test]
#[ignore]
fn chat_proof_sheets() {
    use de_editor_core::chat::ChatMessage;
    use de_editor_core::{ChatActivity, ChatActivityStatus};
    for (theme, suffix) in [(ThemeMode::Dark, "dark"), (ThemeMode::Light, "light")] {
        let mut h = chat_host(theme);
        shoot(&mut h, &format!("chat-empty-{suffix}"));

        h.editor_state_mut().editor_ui.toggle_chat_plus_menu();
        shoot(&mut h, &format!("chat-plus-{suffix}"));
        h.editor_state_mut().editor_ui.toggle_chat_model_picker();
        shoot(&mut h, &format!("chat-picker-{suffix}"));
        h.editor_state_mut().editor_ui.close_chat_model_picker();

        let chat = &mut h.editor_state_mut().chat;
        chat.messages.push(ChatMessage::user(
            "Pricing page for a calm note-taking app. Three tiers, soft greens.",
        ));
        let mut reply = ChatMessage::assistant(
            "I'll set up a 1440 frame with a hero, three pricing cards and an FAQ, using a sage palette.",
        );
        reply.thinking = "Three tiers read best side by side…".into();
        reply.thinking_ms = Some(6_200);
        reply.streaming = true;
        let offset = Some(reply.content.len() as u32);
        let activity = |id: &str, title: &str, status| ChatActivity {
            id: id.into(),
            title: title.into(),
            detail: None,
            status,
            content_offset: offset,
        };
        reply.activities = vec![
            activity("frame", "Set up the 1440 frame", ChatActivityStatus::Done),
            activity(
                "tiers",
                "Lay out the pricing tiers",
                ChatActivityStatus::Running,
            ),
            activity("faq", "Write the FAQ", ChatActivityStatus::Pending),
        ];
        chat.messages.push(reply);
        shoot(&mut h, &format!("chat-conversation-{suffix}"));
    }
}
