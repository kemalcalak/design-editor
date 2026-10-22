use super::WidgetHost;
use de_editor_core::chat::{AgentProvider, ChatAttachment, ChatRole, ModelEntry};
use de_editor_ui::Point2D;

#[test]
fn send_allows_attachment_only_chat_turn() {
    let mut host = WidgetHost::new();
    host.editor_state.chat.focused = true;
    host.editor_state
        .chat
        .available_models
        .push(ModelEntry::new(AgentProvider::CodexCli, "gpt-5", "GPT-5"));
    assert!(host.editor_state.chat.add_attachment(ChatAttachment {
        name: "reference.png".into(),
        media_type: "image/png".into(),
        data: vec![1, 2, 3],
    }));

    assert!(host.apply_send());

    let messages = &host.editor_state.chat.messages;
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].role, ChatRole::User);
    assert_eq!(messages[0].content, "");
    assert_eq!(messages[0].images.len(), 1);
    assert_eq!(messages[1].role, ChatRole::Assistant);
}

#[test]
fn attachment_button_queues_web_file_picker_like_native() {
    let mut host = WidgetHost::new();
    let viewport_w = 1440.0;
    let viewport_h = 900.0;
    let chat_rect = host
        .ai_chat_rect(viewport_w, viewport_h)
        .expect("chat panel visible");
    // Attach lives in the composer's "+" menu (#26): "+" sits at the card's
    // left, 34 px above the panel bottom; the menu opens upward from it and
    // its first row (Attach file) centres 176 px above the bottom.
    let bottom = chat_rect.origin.y + chat_rect.size.y;
    let plus = Point2D::new(chat_rect.origin.x + 37.0, bottom - 34.0);
    let attach = Point2D::new(chat_rect.origin.x + 64.0, bottom - 176.0);

    assert!(host.apply_click(plus.x, plus.y, viewport_w, viewport_h));
    assert!(host.editor_state.editor_ui.chat_plus_menu_open);
    assert!(host.apply_click(attach.x, attach.y, viewport_w, viewport_h));
    assert!(host.editor_state.chat.pending_attachment_pick);
}
