//! Paint + hit-test coverage for the chat-context chrome (#398): the `@`
//! popup and the context panel, driven through the real panel so the
//! rects paint uses are the rects hit-test resolves.

use super::tests::{seed_available_model, PanelPaintBackend};
use super::*;
use crate::widgets::AIChatHit;
use de_editor_core::chat_mentions::{ChatMention, MentionKind};

fn agent_tab_state() -> EditorState {
    let mut s = EditorState::new();
    s.editor_ui.enter_chat_tab();
    seed_available_model(&mut s);
    s
}

fn panel_rect() -> Rect {
    Rect::xywh(0.0, 0.0, AI_CHAT_WIDTH, AI_CHAT_HEIGHT)
}

fn painted(s: &EditorState) -> PanelPaintBackend {
    let panel = AIChatPlaceholder::from_editor(s);
    let mut backend = PanelPaintBackend::default();
    panel.paint(
        &mut PaintCx {
            backend: &mut backend,
        },
        panel_rect(),
    );
    backend
}

#[test]
fn typing_at_paints_the_popup_and_its_rows_pick_their_mention() {
    let mut s = agent_tab_state();
    s.chat.project_files.files = vec!["brief.md".to_string()].into();
    s.chat.focused = true;
    s.chat.input.set_text("@bri");
    s.chat.input.set_caret(4, 0);
    s.sync_chat_mentions();
    let backend = painted(&s);
    let panel = AIChatPlaceholder::from_editor(&s);
    let menu = panel
        .mention_popup_bounds(panel_rect())
        .expect("popup open");
    let label = backend
        .texts
        .iter()
        .find(|(text, ..)| text == "brief.md")
        .expect("the file row is painted");
    assert!(menu.contains(label.3), "the row paints inside the popup");
    assert!(menu.origin.y + menu.size.y <= panel.composer_card(panel_rect()).origin.y);
    let row_point = Point2D::new(menu.origin.x + 40.0, menu.origin.y + 16.0);
    assert_eq!(
        panel.hit_test(panel_rect(), row_point),
        Some(AIChatHit::PickMention(0))
    );
    assert_eq!(panel.mention_hover_at(panel_rect(), row_point), Some(0));
}

#[test]
fn context_button_opens_a_panel_listing_staged_items() {
    let mut s = agent_tab_state();
    let panel = AIChatPlaceholder::from_editor(&s);
    let button = panel.footer_layout(panel_rect()).context;
    let centre = Point2D::new(
        button.origin.x + button.size.x / 2.0,
        button.origin.y + button.size.y / 2.0,
    );
    assert_eq!(
        panel.hit_test(panel_rect(), centre),
        Some(AIChatHit::ToggleContextPanel)
    );
    // The button sits between "+" and the model chip.
    let footer = panel.footer_layout(panel_rect());
    assert!(footer.plus.origin.x + footer.plus.size.x <= button.origin.x);
    assert!(button.origin.x + button.size.x <= footer.model.origin.x);

    s.chat.input.set_text("@Brief");
    s.chat.pending_mentions.push(ChatMention {
        kind: MentionKind::File,
        key: "brief.md".into(),
        label: "Brief".into(),
    });
    s.editor_ui.toggle_chat_context_panel();
    let backend = painted(&s);
    let texts: Vec<&str> = backend.texts.iter().map(|(t, ..)| t.as_str()).collect();
    assert!(texts.contains(&"In context"));
    assert!(texts.contains(&"Document"));
    assert!(texts.contains(&"Brief"));
    // A press outside the open panel closes it.
    let panel = AIChatPlaceholder::from_editor(&s);
    assert_eq!(
        panel.hit_test(panel_rect(), Point2D::new(5.0, 60.0)),
        Some(AIChatHit::ToggleContextPanel)
    );
}
