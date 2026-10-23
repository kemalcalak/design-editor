//! gl-host tests for Home press routing: each target lands on the
//! editor action behind it.

use super::*;
use de_editor_core::agent_settings::McpCli;
use de_editor_core::{HomeFamily, HomeSize, RecentFile, RecentView};
use de_editor_ui::Rect;

const W: f32 = 1440.0;
const H: f32 = 900.0;

fn host_with_home() -> WidgetHostNative {
    let mut host = WidgetHostNative::new();
    host.editor_state_mut().editor_ui.home.show(false);
    host
}

fn center(rect: Rect) -> Point2D {
    Point2D::new(
        rect.origin.x + rect.size.x / 2.0,
        rect.origin.y + rect.size.y / 2.0,
    )
}

fn layout(host: &WidgetHostNative) -> de_editor_ui::widgets::HomeLayout {
    HomeSurface::for_editor(host.editor_state())
        .expect("home visible")
        .layout(W, H)
}

fn press(host: &mut WidgetHostNative, rect: Rect) {
    let point = center(rect);
    assert!(host.apply_press(point.x, point.y, W, H));
    host.apply_release_with_viewport(W, H);
}

#[test]
fn typing_lands_in_the_active_kind_draft_and_switching_kinds_keeps_it() {
    let mut host = host_with_home();
    for character in "a bakery".chars() {
        assert!(host.apply_text(character));
    }
    let slides = layout(&host).chips[3];
    press(&mut host, slides);
    assert_eq!(host.editor_state().editor_ui.home.task, HomeFamily::Slides);
    assert!(host.editor_state().editor_ui.home.draft.is_empty());
    let landing = layout(&host).chips[0];
    press(&mut host, landing);
    assert_eq!(host.editor_state().editor_ui.home.draft, "a bakery");
}

#[test]
fn the_size_menu_opens_picks_and_closes_on_an_outside_press() {
    let mut host = host_with_home();
    let chip = layout(&host).size_chip;
    press(&mut host, chip);
    assert!(host.editor_state().editor_ui.home.size_menu_open);
    let row = layout(&host).size_rows[1];
    press(&mut host, row);
    assert!(!host.editor_state().editor_ui.home.size_menu_open);
    assert_eq!(
        host.editor_state().editor_ui.home.task_draft().size,
        HomeSize::Tablet768
    );

    press(&mut host, chip);
    let figma = layout(&host).imports[0];
    press(&mut host, figma);
    assert!(
        !host.editor_state().editor_ui.home.size_menu_open,
        "an outside press closes the menu"
    );
    assert!(
        host.editor_state().editor_ui.pending_file_action.is_none(),
        "and does nothing else"
    );
}

#[test]
fn import_cards_route_to_the_existing_file_actions() {
    let mut host = host_with_home();
    let imports = layout(&host).imports;
    press(&mut host, imports[0]);
    assert_eq!(
        host.editor_state_mut().editor_ui.pending_file_action.take(),
        Some(FileAction::ImportFigma)
    );
    press(&mut host, imports[1]);
    assert_eq!(
        host.editor_state_mut().editor_ui.pending_file_action.take(),
        Some(FileAction::ImportHtml)
    );
    press(&mut host, imports[3]);
    assert_eq!(
        host.editor_state_mut().editor_ui.pending_file_action.take(),
        Some(FileAction::ImportPenDev)
    );
    press(&mut host, imports[2]);
    assert!(host.editor_state().chat.pending_attachment_pick);
    assert!(
        !host.editor_state().editor_ui.home.draft.is_empty(),
        "the screenshot card fills an empty brief"
    );
}

#[test]
fn top_bar_and_sidebar_ask_for_files_and_templates() {
    let mut host = host_with_home();
    let l = layout(&host);
    press(&mut host, l.new_file);
    assert_eq!(
        host.editor_state_mut().editor_ui.pending_file_action.take(),
        Some(FileAction::New)
    );
    press(&mut host, l.open_file);
    assert_eq!(
        host.editor_state_mut().editor_ui.pending_file_action.take(),
        Some(FileAction::Open)
    );
    press(&mut host, l.nav[2]);
    assert_eq!(
        host.editor_state().editor_ui.home.pending_template,
        Some(HomeTemplate::DesignSystems)
    );
    assert_eq!(
        host.editor_state_mut().editor_ui.pending_file_action.take(),
        Some(FileAction::New)
    );
    press(&mut host, l.nav[4]);
    assert!(host.editor_state().editor_ui.agent_settings_open);
}

#[test]
fn recent_items_open_and_the_toggle_switches_views() {
    let mut host = host_with_home();
    host.editor_state_mut().editor_ui.recent_files = vec![
        RecentFile {
            path: "/Users/me/designs/landing.de".into(),
            modified_at: 1,
        },
        RecentFile {
            path: "/Users/me/designs/deck.de".into(),
            modified_at: 2,
        },
    ];
    let l = layout(&host);
    assert_eq!(l.recent_items.len(), 2);
    press(&mut host, l.recent_items[1]);
    assert_eq!(
        host.editor_state_mut().editor_ui.pending_file_action.take(),
        Some(FileAction::OpenRecent(1))
    );
    press(&mut host, l.view_list);
    assert_eq!(
        host.editor_state().editor_ui.home.recent_view,
        RecentView::List
    );
}

#[test]
fn the_mcp_popover_sets_up_copies_and_closes() {
    let mut host = host_with_home();
    let card = layout(&host).mcp_card;
    press(&mut host, card);
    assert!(host.editor_state().editor_ui.home.mcp_popover_open);

    let home = HomeSurface::for_editor(host.editor_state()).expect("home");
    let popover = home.mcp_popover(&home.layout(W, H), W);
    drop(home);
    let codex = popover
        .tabs
        .iter()
        .find(|(cli, _)| *cli == McpCli::Codex)
        .map(|(_, rect)| *rect)
        .expect("codex tab");
    press(&mut host, codex);
    assert_eq!(host.editor_state().editor_ui.home.mcp_agent, McpCli::Codex);

    press(&mut host, popover.action);
    let settings = &host.editor_state().editor_ui.agent_settings;
    assert!(settings.mcp_cli_enabled[McpCli::Codex.index()]);
    assert!(settings.mcp_server.running);

    press(&mut host, popover.copy_snippet);
    assert!(host
        .editor_state()
        .chat
        .pending_copy_text
        .as_deref()
        .is_some_and(|text| text.starts_with("[mcp_servers.design-editor]")));

    // Removing flips the same slot back.
    let home = HomeSurface::for_editor(host.editor_state()).expect("home");
    let popover = home.mcp_popover(&home.layout(W, H), W);
    drop(home);
    press(&mut host, popover.action);
    assert!(!host.editor_state().editor_ui.agent_settings.mcp_cli_enabled[McpCli::Codex.index()]);

    let outside = layout(&host).chips[0];
    press(&mut host, outside);
    assert!(!host.editor_state().editor_ui.home.mcp_popover_open);
    assert_eq!(
        host.editor_state().editor_ui.home.task,
        HomeFamily::LandingPage,
        "the outside press only closes the popover"
    );
}

#[test]
fn the_more_menu_picks_an_agent_without_a_tab() {
    let mut host = host_with_home();
    host.editor_state_mut().editor_ui.home.mcp_popover_open = true;
    let home = HomeSurface::for_editor(host.editor_state()).expect("home");
    let popover = home.mcp_popover(&home.layout(W, H), W);
    drop(home);
    press(&mut host, popover.more_button);
    assert!(host.editor_state().editor_ui.home.mcp_agent_menu_open);
    let (cli, row) = popover.more_rows[0];
    press(&mut host, row);
    assert_eq!(host.editor_state().editor_ui.home.mcp_agent, cli);
    assert!(!host.editor_state().editor_ui.home.mcp_agent_menu_open);
}

#[test]
fn escape_returns_to_the_document_only_when_home_was_opened_over_one() {
    let mut host = host_with_home();
    assert!(!host.apply_escape() || host.home_visible());
    assert!(host.home_visible(), "launch Home stays");

    host.editor_state_mut().editor_ui.home.show(true);
    assert!(host.apply_escape());
    assert!(!host.home_visible());
}

#[test]
fn typing_and_enter_queue_the_wrapped_turn() {
    let mut host = host_with_home();
    host.editor_state_mut()
        .editor_ui
        .agent_settings
        .builtin_agents
        .push(de_editor_core::BuiltinAgentConfig {
            id: "builtin-1".into(),
            preset: de_editor_core::BuiltinAgentPresetKey::Custom,
            display_name: "MiniMax".into(),
            kind: de_editor_core::BuiltinAgentKind::OpenAiCompat,
            api_key: "sk-test".into(),
            models: vec!["MiniMax-M3".into()],
            base_url: "http://localhost:9".into(),
            enabled: true,
        });
    for character in "a bakery".chars() {
        assert!(host.apply_text(character));
    }
    let expected = host
        .editor_state()
        .editor_ui
        .home
        .generation_prompt()
        .expect("prompt");
    assert!(host.apply_send());
    assert!(!host.home_visible());
    assert_eq!(
        host.editor_state().chat.pending_send.as_deref(),
        Some(expected.as_str())
    );
}

#[test]
fn the_title_strip_drags_the_window_except_over_controls() {
    let host = host_with_home();
    let l = layout(&host);
    assert!(host.home_window_drag_at(700.0, 8.0, W, H), "blank strip");
    let new_file = center(l.new_file);
    assert!(
        !host.home_window_drag_at(new_file.x, 20.0, W, H),
        "the New file button keeps its press"
    );
    assert!(
        !host.home_window_drag_at(700.0, 200.0, W, H),
        "below the strip"
    );
}
