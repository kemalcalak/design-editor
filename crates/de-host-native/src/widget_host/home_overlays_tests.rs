//! gl-host tests for Home's model-access surface: the model chip, the
//! provider link, the Home-anchored picker, and the orchestrator launch
//! route. Run with `--features gl-host`.

use super::WidgetHostNative;
use de_editor_core::{
    BuiltinAgentConfig, BuiltinAgentKind, BuiltinAgentPresetKey, HomeHit, LaunchRoute,
};
use de_editor_ui::widgets::ai_chat_model_picker::{MODEL_GROUP_H, MODEL_ROW_H, MODEL_SEARCH_H};
use de_editor_ui::widgets::HomeSurface;
use de_editor_ui::{Point2D, Rect};

const W: f32 = 1440.0;
const H: f32 = 900.0;

fn host_with_home() -> WidgetHostNative {
    let mut host = WidgetHostNative::new();
    host.editor_state_mut().editor_ui.home.visible = true;
    host
}

fn host_with_usable_agent() -> WidgetHostNative {
    let mut host = host_with_home();
    // A ready built-in agent: opening the picker rebuilds the catalog
    // (`rebuild_chat_models`), and built-in entries survive that
    // rebuild without CLI discovery or verified-connection state.
    host.editor_state_mut()
        .editor_ui
        .agent_settings
        .builtin_agents
        .push(BuiltinAgentConfig {
            id: "builtin-1".into(),
            preset: BuiltinAgentPresetKey::Custom,
            display_name: "MiniMax".into(),
            kind: BuiltinAgentKind::OpenAiCompat,
            api_key: "sk-test".into(),
            models: vec!["MiniMax-M2.7".into(), "MiniMax-M3".into()],
            base_url: "http://localhost:9".into(),
            enabled: true,
        });
    host
}

fn center(rect: Rect) -> Point2D {
    Point2D::new(
        rect.origin.x + rect.size.x / 2.0,
        rect.origin.y + rect.size.y / 2.0,
    )
}

fn chip_center(host: &WidgetHostNative) -> Point2D {
    let home = HomeSurface::for_editor(host.editor_state()).expect("home visible");
    center(home.layout(W, H).model_chip)
}

#[test]
fn pressing_the_model_chip_opens_the_chat_model_picker() {
    let mut host = host_with_usable_agent();
    let point = chip_center(&host);
    assert!(host.apply_press(point.x, point.y, W, H));
    assert!(
        host.editor_state().editor_ui.chat_model_picker.open,
        "the chip must open the same picker the chat panel uses"
    );
}

#[test]
fn the_provider_link_opens_the_agents_settings_tab() {
    let mut host = host_with_home();
    let point = chip_center(&host);
    let home = HomeSurface::for_editor(host.editor_state()).expect("home visible");
    assert_eq!(
        home.hit_test(W, H, point),
        Some(HomeHit::AddProvider),
        "with nothing to answer, the model slot is the provider link"
    );
    drop(home);
    assert!(host.apply_press(point.x, point.y, W, H));
    assert!(host.editor_state().editor_ui.agent_settings_open);
    assert_eq!(
        host.editor_state().editor_ui.agent_settings.tab,
        de_editor_core::AgentSettingsTab::Agents
    );
    assert!(!host.editor_state().editor_ui.chat_model_picker.open);
}

#[test]
fn pressing_send_without_a_usable_agent_queues_no_turn() {
    let mut host = host_with_home();
    host.editor_state_mut().editor_ui.home.set_draft("a bakery");
    let home = HomeSurface::for_editor(host.editor_state()).expect("home visible");
    let send = center(home.layout(W, H).send);
    assert!(host.apply_press(send.x, send.y, W, H));
    assert!(
        host.editor_state().chat.pending_send.is_none(),
        "no turn may be queued while nothing can answer it"
    );
    assert!(
        !host.editor_state().editor_ui.agent_settings_open,
        "a greyed-out Send does nothing"
    );
    assert!(host.home_visible());
}

#[test]
fn enter_without_a_usable_agent_queues_no_turn() {
    let mut host = host_with_home();
    host.editor_state_mut().editor_ui.home.set_draft("a bakery");
    assert!(!host.apply_send(), "Enter is refused like the greyed Send");
    assert!(host.editor_state().chat.pending_send.is_none());
    assert!(host.home_visible());
}

#[test]
fn home_send_marks_the_turn_for_the_orchestrator_route() {
    let mut host = host_with_usable_agent();
    for character in "a bakery".chars() {
        assert!(host.apply_text(character));
    }
    assert!(host.apply_send());
    assert_eq!(
        host.editor_state().chat.launch_route,
        LaunchRoute::Orchestrator,
        "a Home-launched brief must take the orchestrator pipeline"
    );
    assert!(host.editor_state().chat.pending_send.is_some());
}

#[test]
fn picking_a_row_from_the_home_picker_selects_that_model() {
    let mut host = host_with_usable_agent();
    let chip = chip_center(&host);
    assert!(host.apply_press(chip.x, chip.y, W, H));
    let card = host
        .home_model_picker_geometry(W, H)
        .expect("picker geometry resolves above the chip");
    // Second row of the single Claude Code group: search strip + pad +
    // group header + one row, centred in the second 28 px row.
    let row_center = Point2D::new(
        card.origin.x + 24.0,
        card.origin.y + MODEL_SEARCH_H + 6.0 + MODEL_GROUP_H + MODEL_ROW_H + MODEL_ROW_H / 2.0,
    );
    assert!(host.apply_press(row_center.x, row_center.y, W, H));
    assert_eq!(
        host.editor_state()
            .chat
            .selected_model_entry()
            .map(|entry| entry.display_name.as_str()),
        Some("MiniMax-M3")
    );
    assert!(!host.editor_state().editor_ui.chat_model_picker.open);
}

#[test]
fn pressing_outside_the_home_picker_closes_it_without_touching_home() {
    let mut host = host_with_usable_agent();
    let chip = chip_center(&host);
    assert!(host.apply_press(chip.x, chip.y, W, H));
    assert!(host.editor_state().editor_ui.chat_model_picker.open);
    // A press far from the card closes the picker and is still consumed.
    assert!(host.apply_press(120.0, 120.0, W, H));
    assert!(!host.editor_state().editor_ui.chat_model_picker.open);
    assert!(host.home_visible(), "Home itself must survive the close");
}

#[test]
fn the_home_picker_connect_row_lives_inside_the_card_now() {
    // The action used to be an external row BELOW the card, which meant
    // Home showed 接入更多模型 twice (the picker carries its own footer)
    // and the cursor had to leave the popover to reach it.
    let mut host = host_with_usable_agent();
    let chip = chip_center(&host);
    assert!(host.apply_press(chip.x, chip.y, W, H));
    let card = host
        .home_model_picker_geometry(W, H)
        .expect("picker geometry resolves");
    // Nothing hangs below the card any more: a press just under it is
    // an outside press and closes the picker.
    let below = Point2D::new(card.origin.x + 24.0, card.origin.y + card.size.y + 20.0);
    assert!(host.apply_press(below.x, below.y, W, H));
    assert!(!host.editor_state().editor_ui.chat_model_picker.open);
    assert!(!host.editor_state().editor_ui.agent_settings_open);
}

/// Hover must never dismiss a click-opened popover: moving the cursor
/// off the card used to close it, so its own footer row was unreachable.
#[test]
fn moving_the_cursor_off_the_home_picker_keeps_it_open() {
    let mut host = host_with_usable_agent();
    let chip = chip_center(&host);
    assert!(host.apply_press(chip.x, chip.y, W, H));
    let card = host.home_model_picker_geometry(W, H).expect("picker open");
    assert!(host.editor_state().editor_ui.chat_model_picker.open);
    for point in [
        Point2D::new(card.origin.x - 40.0, card.origin.y + 20.0),
        Point2D::new(card.origin.x + 24.0, card.origin.y + card.size.y + 30.0),
        Point2D::new(W - 20.0, H - 20.0),
    ] {
        host.apply_cursor_move(point.x, point.y);
        assert!(
            host.editor_state().editor_ui.chat_model_picker.open,
            "hover at {point:?} must not dismiss"
        );
    }
}

#[test]
fn home_hit_test_still_routes_the_sheet_under_the_new_chip() {
    // The chip must not have swallowed the reference row or the sheet.
    let host = host_with_usable_agent();
    let home = HomeSurface::for_editor(host.editor_state()).expect("home visible");
    let layout = home.layout(W, H);
    let sheet = center(layout.input_box);
    assert_eq!(
        home.hit_test(W, H, sheet),
        Some(HomeHit::Sheet),
        "the input area keeps its caret-press hit"
    );
}
