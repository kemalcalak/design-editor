//! The composer's bottom row (#26: "+" | model chip | send) and the
//! dropdowns it and the session row open.

use super::super::*;
use super::support::*;
use crate::widgets::ai_chat_hit::AIChatHit;
use crate::widgets::chat_click_flow::apply_chat_hit;

fn rail_state() -> EditorState {
    let mut s = EditorState::new();
    // The expanded panel only exists where the conversation lives: the
    // rail's Agent tab. Elsewhere the chat is composer-only.
    s.editor_ui.enter_chat_tab();
    s
}

fn rect() -> Rect {
    Rect::xywh(0.0, 0.0, AI_CHAT_WIDTH, AI_CHAT_HEIGHT)
}

fn centre(r: Rect) -> Point2D {
    Point2D::new(r.origin.x + r.size.x / 2.0, r.origin.y + r.size.y / 2.0)
}

#[test]
fn row_is_plus_then_model_chip_then_round_send() {
    let mut s = rail_state();
    seed_available_model(&mut s);
    let panel = AIChatPlaceholder::from_editor(&s);
    let footer = panel.footer_layout(rect());
    let card = panel.composer_card(rect());
    assert!(
        (footer.send.size.x - footer.send.size.y).abs() < 0.01,
        "send is round"
    );
    assert_eq!(
        footer.stop.origin, footer.send.origin,
        "stop shares the send slot"
    );
    assert!(footer.plus.origin.x >= card.origin.x);
    assert!(footer.plus.origin.x + footer.plus.size.x <= footer.model.origin.x);
    assert!(footer.model.origin.x + footer.model.size.x <= footer.send.origin.x);
    assert!(footer.send.origin.x + footer.send.size.x <= card.origin.x + card.size.x);
    // The chip hugs send on the right, as in the frames.
    assert!(footer.send.origin.x - (footer.model.origin.x + footer.model.size.x) < 10.0);
    // Every control sits inside the card.
    for r in [footer.plus, footer.model, footer.send] {
        assert!(
            r.origin.y >= card.origin.y && r.origin.y + r.size.y <= card.origin.y + card.size.y
        );
    }
}

#[test]
fn a_narrow_panel_squeezes_the_chip_not_the_buttons() {
    let mut s = rail_state();
    seed_available_model(&mut s);
    let panel = AIChatPlaceholder::from_editor(&s);
    let narrow = Rect::xywh(0.0, 0.0, AI_CHAT_MIN_WIDTH, AI_CHAT_HEIGHT);
    let footer = panel.footer_layout(narrow);
    assert!(footer.plus.origin.x + footer.plus.size.x <= footer.model.origin.x + 0.01);
    assert!(footer.model.origin.x + footer.model.size.x <= footer.send.origin.x + 0.01);
    assert!(footer.model.size.x >= 0.0);
}

#[test]
fn row_hits_resolve_plus_model_and_send() {
    let mut s = rail_state();
    seed_available_model(&mut s);
    let panel = AIChatPlaceholder::from_editor(&s);
    let footer = panel.footer_layout(rect());
    assert_eq!(
        panel.hit_test(rect(), centre(footer.plus)),
        Some(AIChatHit::TogglePlusMenu)
    );
    // `seed_available_model` adds a model, but no agent is connected, so the
    // chip is the "Add a model provider" link.
    assert_eq!(
        panel.hit_test(rect(), centre(footer.model)),
        Some(AIChatHit::OpenProviderSettings)
    );
    // Empty input: send focuses the input instead of sending nothing.
    assert_eq!(
        panel.hit_test(rect(), centre(footer.send)),
        Some(AIChatHit::FocusInput)
    );
    assert_eq!(
        panel.footer_hover_at(rect(), centre(footer.plus)),
        Some(de_editor_core::ChatFooterButton::PlusMenu)
    );
}

#[test]
fn stop_is_live_only_while_a_turn_streams() {
    let mut s = rail_state();
    seed_available_model(&mut s);
    s.chat.set_input_text("hi".to_string());
    s.chat.begin_send();
    let panel = AIChatPlaceholder::from_editor(&s);
    let footer = panel.footer_layout(rect());
    assert_eq!(
        panel.hit_test(rect(), centre(footer.stop)),
        Some(AIChatHit::Stop)
    );
}

#[test]
fn the_plus_menu_opens_upward_and_its_rows_are_the_old_controls() {
    let mut s = rail_state();
    apply_chat_hit(&mut s, AIChatHit::TogglePlusMenu, 0);
    assert!(s.editor_ui.chat_plus_menu_open);
    let panel = AIChatPlaceholder::from_editor(&s);
    let menu = panel.plus_menu_bounds(rect()).expect("menu is open");
    let footer = panel.footer_layout(rect());
    assert!(menu.origin.y + menu.size.y <= footer.plus.origin.y);
    let row = |i: f32| Point2D::new(menu.origin.x + 40.0, menu.origin.y + 4.0 + 34.0 * i + 17.0);
    assert_eq!(
        panel.hit_test(rect(), row(0.0)),
        Some(AIChatHit::AddAttachment)
    );
    assert_eq!(
        panel.hit_test(rect(), row(1.0)),
        Some(AIChatHit::OpenPromptCenter)
    );
    assert_eq!(
        panel.hit_test(rect(), row(2.0)),
        Some(AIChatHit::CycleThinking)
    );
    assert_eq!(
        panel.hit_test(rect(), row(3.0)),
        Some(AIChatHit::ToggleParallelAgentsPicker)
    );
    assert_eq!(panel.plus_menu_hover_at(rect(), row(2.0)), Some(2));
    // A press anywhere else closes the menu.
    assert_eq!(
        panel.hit_test(rect(), Point2D::new(AI_CHAT_WIDTH - 20.0, 200.0)),
        Some(AIChatHit::TogglePlusMenu)
    );
}

#[test]
fn thinking_cycles_from_the_menu_and_keeps_it_open() {
    let mut s = rail_state();
    apply_chat_hit(&mut s, AIChatHit::TogglePlusMenu, 0);
    let before = s.chat.thinking_mode;
    apply_chat_hit(&mut s, AIChatHit::CycleThinking, 0);
    assert_ne!(s.chat.thinking_mode, before);
    assert!(s.editor_ui.chat_plus_menu_open);
    apply_chat_hit(&mut s, AIChatHit::AddAttachment, 0);
    assert!(!s.editor_ui.chat_plus_menu_open);
    assert!(s.chat.pending_attachment_pick);
}

#[test]
fn parallel_agents_replace_the_menu_and_pick_a_count() {
    let mut s = rail_state();
    apply_chat_hit(&mut s, AIChatHit::TogglePlusMenu, 0);
    apply_chat_hit(&mut s, AIChatHit::ToggleParallelAgentsPicker, 0);
    assert!(!s.editor_ui.chat_plus_menu_open);
    assert!(s.editor_ui.parallel_agents_picker_open);
    let panel = AIChatPlaceholder::from_editor(&s);
    let picker = panel
        .parallel_picker_bounds(rect())
        .expect("picker is open");
    let footer = panel.footer_layout(rect());
    assert!(picker.origin.y + picker.size.y <= footer.plus.origin.y);
    let third = Point2D::new(
        picker.origin.x + 20.0,
        picker.origin.y + 32.0 + 28.0 * 2.0 + 14.0,
    );
    assert_eq!(
        panel.hit_test(rect(), third),
        Some(AIChatHit::SetParallelAgents(3))
    );
    assert_eq!(
        panel.parallel_agents_picker_hover_at(rect(), third),
        Some(3)
    );
    assert_eq!(
        panel.hit_test(rect(), Point2D::new(AI_CHAT_WIDTH - 20.0, 200.0)),
        Some(AIChatHit::ToggleParallelAgentsPicker)
    );
    apply_chat_hit(&mut s, AIChatHit::SetParallelAgents(3), 0);
    assert_eq!(s.chat.agent_team_size, 3);
    assert!(!s.editor_ui.parallel_agents_picker_open);
}

#[test]
fn session_row_controls_resolve_their_hits() {
    let s = rail_state();
    let panel = AIChatPlaceholder::from_editor(&s);
    let header = panel.header(rect());
    assert_eq!(
        panel.hit_test(rect(), centre(header.new_chat)),
        Some(AIChatHit::NewChat)
    );
    assert_eq!(
        panel.hit_test(rect(), centre(header.select)),
        Some(AIChatHit::ToggleSessionMenu)
    );
}

#[test]
fn the_session_list_switches_and_closes_chats() {
    let mut s = rail_state();
    s.chat.new_tab();
    apply_chat_hit(&mut s, AIChatHit::ToggleSessionMenu, 0);
    assert!(s.editor_ui.chat_session_menu_open);
    let panel = AIChatPlaceholder::from_editor(&s);
    let menu = panel.session_menu_bounds(rect()).expect("list is open");
    let first = Point2D::new(menu.origin.x + 30.0, menu.origin.y + 4.0 + 16.0);
    assert_eq!(panel.hit_test(rect(), first), Some(AIChatHit::SwitchTab(0)));
    assert_eq!(panel.tab_hover_at(rect(), first), Some(0));
    // With the row hovered, its right edge is the close glyph.
    s.editor_ui.chat_tab_hover = Some(0);
    let panel = AIChatPlaceholder::from_editor(&s);
    let close = Point2D::new(menu.origin.x + menu.size.x - 4.0 - 8.0 - 7.0, first.y);
    assert_eq!(panel.hit_test(rect(), close), Some(AIChatHit::CloseTab(0)));
    apply_chat_hit(&mut s, AIChatHit::SwitchTab(0), 0);
    assert!(!s.editor_ui.chat_session_menu_open);
    assert_eq!(s.chat.active_index(), 0);
}

#[test]
fn opening_one_chat_dropdown_closes_the_others() {
    let mut s = rail_state();
    apply_chat_hit(&mut s, AIChatHit::ToggleSessionMenu, 0);
    apply_chat_hit(&mut s, AIChatHit::TogglePlusMenu, 0);
    assert!(!s.editor_ui.chat_session_menu_open);
    assert!(s.editor_ui.chat_plus_menu_open);
    assert!(s.editor_ui.escape_chat_menus());
    assert!(!s.editor_ui.chat_plus_menu_open);
    assert!(!s.editor_ui.escape_chat_menus());
}
