use super::WidgetHost;
use de_editor_core::agent_settings::{
    AgentSettingsTab, BuiltinAgentField, ImageGenField, ImageGenProvider, ImageTestStatus,
    SettingsFocus,
};
use de_editor_core::{AgentProvider, AgentSettingsButton, ButtonPressTarget, PencilCursorStyle};
use de_editor_ui::widgets::agent_settings_panel::{AgentSettingsHit, AgentSettingsPanel};
use de_editor_ui::Point2D;

#[test]
fn close_press_sets_and_release_clears_agent_settings_button() {
    let mut host = WidgetHost::new();
    let panel = AgentSettingsPanel::for_web_editor(&host.editor_state);
    let rect = panel.rect(1200.0, 800.0);
    let close = de_editor_ui::widgets::agent_settings_panel::close_button_rect(rect);
    let close_x = close.origin.x + close.size.x / 2.0;
    let close_y = close.origin.y + close.size.y / 2.0;

    assert!(host.dispatch_agent_settings_press(close_x, close_y, 1200.0, 800.0));
    assert_eq!(
        host.editor_state.editor_ui.pressed_button,
        Some(ButtonPressTarget::AgentSettings(AgentSettingsButton::Close))
    );

    assert!(host.apply_release_with_viewport(1200.0, 800.0));
    assert_eq!(host.editor_state.editor_ui.pressed_button, None);
}

/// Press wherever the web panel resolves to `hit`, scrolling the body
/// down the way a user would. Panics when no scroll offset shows it.
pub(super) fn press_hit(host: &mut WidgetHost, hit: AgentSettingsHit) -> bool {
    let mut scroll = 0.0_f32;
    loop {
        host.editor_state.editor_ui.agent_settings.scroll_y.offset = scroll;
        let (point, max) = {
            let panel = AgentSettingsPanel::for_web_editor(&host.editor_state);
            let rect = panel.rect(1200.0, 800.0);
            (panel.point_for_hit(rect, hit), panel.max_scroll(rect))
        };
        if let Some(point) = point {
            return host.dispatch_agent_settings_press(point.x, point.y, 1200.0, 800.0);
        }
        if scroll >= max {
            panic!("no scroll offset shows a control that resolves to {hit:?}");
        }
        scroll = (scroll + 24.0).min(max);
    }
}

/// Every hit reachable anywhere in the web panel's body, across scroll.
pub(super) fn reachable_hits(host: &mut WidgetHost) -> Vec<AgentSettingsHit> {
    let mut hits = Vec::new();
    let mut scroll = 0.0_f32;
    loop {
        host.editor_state.editor_ui.agent_settings.scroll_y.offset = scroll;
        let panel = AgentSettingsPanel::for_web_editor(&host.editor_state);
        let rect = panel.rect(1200.0, 800.0);
        let content = panel.resolved_content_viewport(rect);
        let mut y = content.origin.y + 2.0;
        while y < content.origin.y + content.size.y {
            let mut x = content.origin.x + 2.0;
            while x < content.origin.x + content.size.x {
                let hit = panel.hit_test(rect, Point2D::new(x, y));
                if !hits.contains(&hit) {
                    hits.push(hit);
                }
                x += 6.0;
            }
            y += 6.0;
        }
        let max = panel.max_scroll(rect);
        if scroll >= max {
            break;
        }
        scroll = (scroll + 24.0).min(max);
    }
    host.editor_state.editor_ui.agent_settings.scroll_y.offset = 0.0;
    hits
}

fn first_hit_point(
    panel: &AgentSettingsPanel<'_>,
    rect: de_editor_ui::Rect,
    hit: AgentSettingsHit,
) -> Option<Point2D> {
    let mut y = rect.origin.y;
    while y < rect.origin.y + rect.size.y {
        let mut x = rect.origin.x;
        while x < rect.origin.x + rect.size.x {
            let p = Point2D::new(x, y);
            if panel.hit_test(rect, p) == hit {
                return Some(p);
            }
            x += 4.0;
        }
        y += 4.0;
    }
    None
}

#[test]
fn web_system_pencil_cursor_selection_updates_style() {
    let mut host = WidgetHost::new();
    host.editor_state.editor_ui.agent_settings.tab = AgentSettingsTab::System;
    assert_eq!(
        host.editor_state.editor_ui.pencil_cursor_style,
        PencilCursorStyle::Rounded
    );

    let panel = AgentSettingsPanel::for_web_editor(&host.editor_state);
    let rect = panel.rect(1200.0, 800.0);
    let point = first_hit_point(
        &panel,
        rect,
        AgentSettingsHit::SelectPencilCursor(PencilCursorStyle::Classic),
    )
    .expect("classic pencil cursor swatch should be clickable");

    assert!(host.dispatch_agent_settings_press(point.x, point.y, 1200.0, 800.0));
    assert_eq!(
        host.editor_state.editor_ui.pencil_cursor_style,
        PencilCursorStyle::Classic
    );
}

#[test]
fn web_agent_settings_hides_cli_provider_connect_targets() {
    let host = WidgetHost::new();
    let panel = AgentSettingsPanel::for_web_editor(&host.editor_state);
    let rect = panel.rect(1200.0, 800.0);

    assert!(
        first_hit_point(
            &panel,
            rect,
            AgentSettingsHit::Connect(AgentProvider::CodexCli)
        )
        .is_none(),
        "web agent settings should hide Codex CLI connect targets"
    );
}

#[test]
fn web_agent_settings_hides_mcp_tab_from_sidebar() {
    let host = WidgetHost::new();
    let panel = AgentSettingsPanel::for_web_editor(&host.editor_state);
    let rect = panel.rect(1200.0, 800.0);
    let mcp_nav = first_hit_point(
        &panel,
        rect,
        AgentSettingsHit::SelectTab(AgentSettingsTab::Mcp),
    );

    assert_eq!(
        mcp_nav, None,
        "web agent settings should not expose the MCP tab"
    );
    assert_eq!(
        host.editor_state.editor_ui.agent_settings.tab,
        AgentSettingsTab::Agents
    );
}

#[test]
fn toggling_builtin_kind_commits_focused_api_key_draft() {
    let mut host = WidgetHost::new();
    host.editor_state
        .editor_ui
        .agent_settings
        .add_builtin_agent_with_defaults("MINIMAX", "", "MiniMax-M2.7");
    host.editor_state
        .editor_ui
        .agent_settings
        .set_builtin_agent_preset(0, de_editor_core::BuiltinAgentPresetKey::MiniMax);
    host.editor_state.editor_ui.agent_settings.focus = Some(SettingsFocus::BuiltinAgent {
        index: 0,
        field: BuiltinAgentField::ApiKey,
    });
    host.editor_state
        .editor_ui
        .settings_input
        .set_text("sk-web");

    assert!(press_hit(
        &mut host,
        AgentSettingsHit::ToggleBuiltinAgentKind(0)
    ));

    let agent = &host.editor_state.editor_ui.agent_settings.builtin_agents[0];
    assert_eq!(agent.api_key, "sk-web");
    assert_eq!(
        agent.kind,
        de_editor_core::agent_settings::BuiltinAgentKind::OpenAiCompat
    );
    assert!(host.editor_state.editor_ui.agent_settings.focus.is_none());
}

#[test]
fn add_custom_endpoint_opens_unsaved_builtin_agent_draft() {
    let mut host = WidgetHost::new();
    host.set_now_ms(1234);

    assert!(press_hit(&mut host, AgentSettingsHit::AddProvider));
    assert_eq!(
        host.editor_state.editor_ui.pressed_button,
        Some(ButtonPressTarget::AgentSettings(
            AgentSettingsButton::AddProvider
        ))
    );

    let settings = &host.editor_state.editor_ui.agent_settings;
    assert!(settings.builtin_agents.is_empty());
    assert!(settings.builtin_agent_draft.is_some());
    assert_eq!(
        settings.focus,
        Some(SettingsFocus::BuiltinAgentDraft(BuiltinAgentField::BaseUrl))
    );
    assert_eq!(
        host.editor_state
            .editor_ui
            .settings_input
            .next_blink_flip_ms(1234),
        1734
    );

    assert!(host.apply_release_with_viewport(1200.0, 800.0));
    assert_eq!(host.editor_state.editor_ui.pressed_button, None);
}

#[test]
fn the_web_agents_tab_offers_no_acp_or_cli_controls() {
    let mut host = WidgetHost::new();
    let hits = reachable_hits(&mut host);
    assert!(
        !hits.iter().any(|hit| matches!(
            hit,
            AgentSettingsHit::AddAcpAgent
                | AgentSettingsHit::SelectCliEntry(_)
                | AgentSettingsHit::SelectAcpEntry(_)
                | AgentSettingsHit::Connect(_)
        )),
        "the browser cannot run local agents: {hits:?}"
    );
    assert!(hits.contains(&AgentSettingsHit::AddProvider));
}

#[test]
fn builtin_provider_select_retargets_the_draft_preset() {
    let mut host = WidgetHost::new();
    assert!(press_hit(&mut host, AgentSettingsHit::AddProvider));
    assert!(press_hit(
        &mut host,
        AgentSettingsHit::ToggleBuiltinAgentPresetMenu(None)
    ));
    assert!(press_hit(
        &mut host,
        AgentSettingsHit::SelectBuiltinAgentPreset {
            index: None,
            preset: de_editor_core::BuiltinAgentPresetKey::MiniMax,
        }
    ));

    let draft = host
        .editor_state
        .editor_ui
        .agent_settings
        .builtin_agent_draft
        .as_ref()
        .expect("draft remains open");
    assert_eq!(draft.preset, de_editor_core::BuiltinAgentPresetKey::MiniMax);
    assert_eq!(draft.display_name, "MiniMax");
    assert_eq!(draft.models, ["MiniMax-M3"]);
    assert_eq!(draft.base_url, "https://api.minimaxi.com/anthropic");
}

#[test]
fn save_builtin_agent_draft_persists_provider() {
    let mut host = WidgetHost::new();
    assert!(press_hit(
        &mut host,
        AgentSettingsHit::SelectApiPreset(de_editor_core::BuiltinAgentPresetKey::Anthropic)
    ));
    for c in "sk-web".chars() {
        assert!(host.apply_text(c));
    }
    assert!(press_hit(
        &mut host,
        AgentSettingsHit::SaveBuiltinAgentDraft
    ));

    let settings = &host.editor_state.editor_ui.agent_settings;
    assert_eq!(settings.builtin_agents.len(), 1);
    assert!(settings.builtin_agent_draft.is_none());
    assert_eq!(settings.builtin_agents[0].api_key, "sk-web");
    assert!(settings.focus.is_none());
}

#[test]
fn image_generation_add_press_sets_and_release_clears_agent_settings_button() {
    let mut host = WidgetHost::new();
    host.editor_state.editor_ui.agent_settings.tab = AgentSettingsTab::Images;

    let panel = AgentSettingsPanel::for_web_editor(&host.editor_state);
    let rect = panel.rect(1200.0, 800.0);
    let content_x = de_editor_ui::widgets::agent_settings_panel::secondary_tab_body(rect)
        .origin
        .x;
    let content_y = de_editor_ui::widgets::agent_settings_panel::secondary_tab_body(rect)
        .origin
        .y;
    let content_w = de_editor_ui::widgets::agent_settings_panel::secondary_tab_body(rect)
        .size
        .x;
    let gen_top = content_y + 36.0 + 24.0 + 28.0;
    let add_x = content_x + content_w - 36.0;
    let add_y = gen_top + 18.0;

    assert!(host.dispatch_agent_settings_press(add_x, add_y, 1200.0, 800.0));
    assert_eq!(
        host.editor_state.editor_ui.pressed_button,
        Some(ButtonPressTarget::AgentSettings(
            AgentSettingsButton::ImageGenAdd
        ))
    );

    assert!(host.apply_release_with_viewport(1200.0, 800.0));
    assert_eq!(host.editor_state.editor_ui.pressed_button, None);
}

#[test]
fn image_search_test_press_sets_and_release_clears_agent_settings_button() {
    let mut host = WidgetHost::new();
    host.editor_state.editor_ui.agent_settings.tab = AgentSettingsTab::Images;
    host.editor_state
        .editor_ui
        .agent_settings
        .images_advanced_open = true;
    host.editor_state
        .editor_ui
        .agent_settings
        .openverse_client_id = "client".into();
    host.editor_state
        .editor_ui
        .agent_settings
        .openverse_client_secret = "secret".into();

    let panel = AgentSettingsPanel::for_web_editor(&host.editor_state);
    let rect = panel.rect(1200.0, 800.0);
    let content_y = de_editor_ui::widgets::agent_settings_panel::secondary_tab_body(rect)
        .origin
        .y;
    let content_w = de_editor_ui::widgets::agent_settings_panel::secondary_tab_body(rect)
        .size
        .x;
    let x = de_editor_ui::widgets::agent_settings_panel::secondary_tab_body(rect)
        .origin
        .x
        + content_w
        - 28.0;
    let y = content_y + 36.0 + 24.0 + 22.0 + 36.0 + 10.0 + 36.0 + 14.0 + 18.0;

    assert!(host.dispatch_agent_settings_press(x, y, 1200.0, 800.0));
    assert_eq!(
        host.editor_state.editor_ui.pressed_button,
        Some(ButtonPressTarget::AgentSettings(
            AgentSettingsButton::ImageSearchTest
        ))
    );

    assert!(host.apply_release_with_viewport(1200.0, 800.0));
    assert_eq!(host.editor_state.editor_ui.pressed_button, None);
}

#[test]
fn image_generation_profile_test_tracks_testing_status_like_ts() {
    let mut host = WidgetHost::new();
    host.editor_state.editor_ui.agent_settings.tab = AgentSettingsTab::Images;
    host.editor_state
        .editor_ui
        .agent_settings
        .add_image_gen_profile();
    host.editor_state
        .editor_ui
        .agent_settings
        .image_gen_profiles[0]
        .api_key = "sk-test".into();
    host.editor_state.editor_ui.agent_settings.focus = Some(SettingsFocus::ImageGenProfile {
        index: 0,
        field: ImageGenField::Name,
    });

    let panel = AgentSettingsPanel::for_web_editor(&host.editor_state);
    let rect = panel.rect(1200.0, 800.0);
    let content_x = de_editor_ui::widgets::agent_settings_panel::secondary_tab_body(rect)
        .origin
        .x;
    let content_y = de_editor_ui::widgets::agent_settings_panel::secondary_tab_body(rect)
        .origin
        .y;
    let content_w = de_editor_ui::widgets::agent_settings_panel::secondary_tab_body(rect)
        .size
        .x;
    let gen_top = content_y + 36.0 + 24.0 + 28.0;
    let row_y = gen_top + 36.0 + 8.0;
    let api_field_y = row_y + 32.0 + 8.0 + 36.0 * 2.0;

    assert!(host.dispatch_agent_settings_press(
        content_x + content_w - 40.0,
        api_field_y + 12.0,
        1200.0,
        800.0
    ));

    let profile = &host
        .editor_state
        .editor_ui
        .agent_settings
        .image_gen_profiles[0];
    assert_eq!(profile.test_status, ImageTestStatus::Testing);
    assert_eq!(
        host.editor_state.editor_ui.pressed_button,
        Some(ButtonPressTarget::AgentSettings(
            AgentSettingsButton::ImageProfileTest(0)
        ))
    );

    assert!(host.apply_release_with_viewport(1200.0, 800.0));
    assert_eq!(host.editor_state.editor_ui.pressed_button, None);
}

#[test]
fn image_generation_provider_select_commits_and_closes_menu() {
    let mut host = WidgetHost::new();
    host.editor_state.editor_ui.agent_settings.tab = AgentSettingsTab::Images;
    host.editor_state
        .editor_ui
        .agent_settings
        .add_image_gen_profile();
    host.editor_state
        .editor_ui
        .agent_settings
        .image_gen_profiles[0]
        .model = "dall-e-3".into();
    host.editor_state.editor_ui.agent_settings.focus = Some(SettingsFocus::ImageGenProfile {
        index: 0,
        field: ImageGenField::Name,
    });

    let panel = AgentSettingsPanel::for_web_editor(&host.editor_state);
    let rect = panel.rect(1200.0, 800.0);
    let content_x = de_editor_ui::widgets::agent_settings_panel::secondary_tab_body(rect)
        .origin
        .x;
    let content_y = de_editor_ui::widgets::agent_settings_panel::secondary_tab_body(rect)
        .origin
        .y;
    let gen_top = content_y + 36.0 + 24.0 + 28.0;
    let row_y = gen_top + 36.0 + 8.0;
    let provider_y = row_y + 32.0 + 8.0 + 36.0;

    assert!(host.dispatch_agent_settings_press(
        content_x + 110.0 + 20.0,
        provider_y + 12.0,
        1200.0,
        800.0
    ));
    assert_eq!(
        host.editor_state
            .editor_ui
            .agent_settings
            .image_gen_provider_menu_open,
        Some(0)
    );
    assert_eq!(
        host.editor_state.editor_ui.agent_settings.focus,
        Some(SettingsFocus::ImageGenProfile {
            index: 0,
            field: ImageGenField::Name,
        })
    );
    assert_eq!(
        host.editor_state.editor_ui.pressed_button,
        Some(ButtonPressTarget::AgentSettings(
            AgentSettingsButton::ImageProfileProvider(0)
        ))
    );
    assert!(host.apply_release_with_viewport(1200.0, 800.0));
    assert_eq!(host.editor_state.editor_ui.pressed_button, None);

    assert!(host.dispatch_agent_settings_press(
        content_x + 110.0 + 20.0,
        provider_y + 60.0,
        1200.0,
        800.0
    ));

    let settings = &host.editor_state.editor_ui.agent_settings;
    let profile = &settings.image_gen_profiles[0];
    assert_eq!(profile.provider, ImageGenProvider::OpenAi);
    assert_eq!(profile.model, "dall-e-3");
    assert_eq!(settings.image_gen_provider_menu_open, Some(0));
    assert_eq!(
        host.editor_state.editor_ui.pressed_button,
        Some(ButtonPressTarget::AgentSettings(
            AgentSettingsButton::ImageProviderOption {
                index: 0,
                provider: ImageGenProvider::Gemini,
            },
        ))
    );

    assert!(host.apply_release_with_viewport(1200.0, 800.0));

    let settings = &host.editor_state.editor_ui.agent_settings;
    let profile = &settings.image_gen_profiles[0];
    assert_eq!(profile.provider, ImageGenProvider::Gemini);
    assert!(profile.model.is_empty());
    assert!(settings.image_gen_provider_menu_open.is_none());
    assert_eq!(host.editor_state.editor_ui.pressed_button, None);
    assert_eq!(
        settings.focus,
        Some(SettingsFocus::ImageGenProfile {
            index: 0,
            field: ImageGenField::Name,
        })
    );
}

#[test]
fn image_generation_profile_header_click_toggles_editor_closed() {
    let mut host = WidgetHost::new();
    host.editor_state.editor_ui.agent_settings.tab = AgentSettingsTab::Images;
    host.editor_state
        .editor_ui
        .agent_settings
        .add_image_gen_profile();
    host.editor_state.editor_ui.agent_settings.focus = Some(SettingsFocus::ImageGenProfile {
        index: 0,
        field: ImageGenField::Name,
    });
    host.editor_state
        .editor_ui
        .settings_input
        .set_text("Config 1");

    let panel = AgentSettingsPanel::for_web_editor(&host.editor_state);
    let rect = panel.rect(1200.0, 800.0);
    let content_x = de_editor_ui::widgets::agent_settings_panel::secondary_tab_body(rect)
        .origin
        .x;
    let content_y = de_editor_ui::widgets::agent_settings_panel::secondary_tab_body(rect)
        .origin
        .y;
    let gen_top = content_y + 36.0 + 24.0 + 28.0;
    let row_y = gen_top + 36.0 + 8.0;

    assert!(host.dispatch_agent_settings_press(content_x + 72.0, row_y + 16.0, 1200.0, 800.0));
    assert_eq!(
        host.editor_state.editor_ui.pressed_button,
        Some(ButtonPressTarget::AgentSettings(
            AgentSettingsButton::ImageProfileHeader(0)
        ))
    );

    assert_eq!(host.editor_state.editor_ui.agent_settings.focus, None);
    assert!(host.editor_state.editor_ui.settings_input.text().is_empty());
    assert!(host.apply_release_with_viewport(1200.0, 800.0));
    assert_eq!(host.editor_state.editor_ui.pressed_button, None);
}

#[test]
fn web_mcp_server_button_is_hidden_when_mcp_tab_is_persisted() {
    let mut host = WidgetHost::new();
    host.editor_state.editor_ui.agent_settings.tab = AgentSettingsTab::Mcp;
    let panel = AgentSettingsPanel::for_web_editor(&host.editor_state);
    let rect = panel.rect(1200.0, 800.0);
    // The web tab set has no MCP entry, so a persisted `tab = Mcp` falls
    // back to Agents. Press exactly where the MCP tab would have put its
    // Start/Stop button: whatever the Agents tab paints there, it must
    // never be the server toggle. (The old fixture asserted `Inside` at a
    // hand-copied offset, which only held while the fallback tab happened
    // to be empty there — an incidental property, not the contract this
    // test is named for.)
    let button = de_editor_ui::widgets::agent_settings_panel::mcp_server_button(rect);
    let x = button.origin.x + button.size.x / 2.0;
    let y = button.origin.y + button.size.y / 2.0;

    assert_ne!(
        panel.hit_test(rect, Point2D::new(x, y)),
        AgentSettingsHit::ToggleMcpServer
    );
    assert!(host.dispatch_agent_settings_press(x, y, 1200.0, 800.0));
    assert!(
        !host
            .editor_state
            .editor_ui
            .agent_settings
            .mcp_server
            .running
    );
    assert_eq!(host.editor_state.editor_ui.pressed_button, None);
}

#[test]
fn web_mcp_client_config_copy_is_hidden_when_mcp_tab_is_persisted() {
    let mut host = WidgetHost::new();
    host.set_now_ms(4_321);
    host.editor_state.editor_ui.agent_settings.tab = AgentSettingsTab::Mcp;
    host.editor_state
        .editor_ui
        .agent_settings
        .mcp_server
        .running = true;

    let panel = AgentSettingsPanel::for_web_editor(&host.editor_state);
    let rect = panel.rect(1200.0, 800.0);
    let content = de_editor_ui::widgets::agent_settings_panel::content_viewport(rect);
    // The web tab set has no MCP entry, so a persisted `tab = Mcp` falls
    // back to Agents. Press exactly where the MCP tab would have put its
    // copy action: whatever the Agents tab paints there, it must never be
    // the copy. (The old fixture asserted `Inside` at that point, which
    // only held while the fallback tab happened to be empty there — an
    // incidental property, not the contract this test is named for.)
    let copy = de_editor_ui::widgets::agent_settings_panel::mcp_copy_config_button(
        rect,
        &host.editor_state.editor_ui.agent_settings,
        host.editor_state.editor_ui.effective_locale(),
    );
    let scroll = copy.origin.y - content.origin.y;
    host.editor_state.editor_ui.agent_settings.scroll_y.offset = scroll;
    let panel = AgentSettingsPanel::for_web_editor(&host.editor_state);
    let x = copy.origin.x + copy.size.x / 2.0;
    let y = copy.origin.y + copy.size.y / 2.0 - scroll;

    assert_ne!(
        panel.hit_test(rect, Point2D::new(x, y)),
        AgentSettingsHit::CopyMcpClientConfig
    );
    assert!(host.dispatch_agent_settings_press(x, y, 1200.0, 800.0));

    assert_eq!(
        host.editor_state
            .editor_ui
            .agent_settings
            .mcp_client_config_copied_at_ms,
        None
    );
    assert_eq!(host.editor_state.chat.pending_copy_text.as_deref(), None);
    assert_ne!(
        host.editor_state.editor_ui.pressed_button,
        Some(ButtonPressTarget::AgentSettings(
            AgentSettingsButton::McpClientConfigCopy
        ))
    );
}

#[test]
fn web_mcp_server_button_hover_is_hidden_when_mcp_tab_is_persisted() {
    let mut host = WidgetHost::new();
    host.last_viewport_w = 1200.0;
    host.last_viewport_h = 800.0;
    host.editor_state.editor_ui.agent_settings_open = true;
    host.editor_state.editor_ui.agent_settings.tab = AgentSettingsTab::Mcp;
    host.editor_state
        .editor_ui
        .agent_settings
        .mcp_server
        .running = true;

    let panel = AgentSettingsPanel::for_web_editor(&host.editor_state);
    let rect = panel.rect(1200.0, 800.0);
    let content_x = de_editor_ui::widgets::agent_settings_panel::content_viewport(rect)
        .origin
        .x;
    let content_y = de_editor_ui::widgets::agent_settings_panel::content_viewport(rect)
        .origin
        .y;
    let content_w = de_editor_ui::widgets::agent_settings_panel::content_viewport(rect)
        .size
        .x;
    let server_card_y = content_y + 36.0;
    let button_x = content_x + content_w - 16.0 - 72.0;

    assert!(!host.update_agent_settings_hover(button_x + 36.0, server_card_y + 26.0,));
    assert!(
        !host
            .editor_state
            .editor_ui
            .agent_settings
            .hover_mcp_server_button
    );
}

#[test]
fn image_settings_button_hover_tracks_cursor() {
    let mut host = WidgetHost::new();
    host.last_viewport_w = 1200.0;
    host.last_viewport_h = 800.0;
    host.editor_state.editor_ui.agent_settings_open = true;
    host.editor_state.editor_ui.agent_settings.tab = AgentSettingsTab::Images;
    host.editor_state
        .editor_ui
        .agent_settings
        .images_advanced_open = true;

    let panel = AgentSettingsPanel::for_web_editor(&host.editor_state);
    let rect = panel.rect(1200.0, 800.0);
    let content_x = de_editor_ui::widgets::agent_settings_panel::secondary_tab_body(rect)
        .origin
        .x;
    let content_y = de_editor_ui::widgets::agent_settings_panel::secondary_tab_body(rect)
        .origin
        .y;
    let content_w = de_editor_ui::widgets::agent_settings_panel::secondary_tab_body(rect)
        .size
        .x;

    assert!(host.update_agent_settings_hover(content_x + content_w - 28.0, content_y + 196.0,));
    assert!(
        host.editor_state
            .editor_ui
            .agent_settings
            .hover_image_search_test_button
    );

    assert!(host.update_agent_settings_hover(content_x + content_w - 36.0, content_y + 260.0,));
    assert!(
        host.editor_state
            .editor_ui
            .agent_settings
            .hover_image_gen_add_button
    );
    assert!(
        !host
            .editor_state
            .editor_ui
            .agent_settings
            .hover_image_search_test_button
    );
}
