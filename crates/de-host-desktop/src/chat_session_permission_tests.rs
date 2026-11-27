//! "Ask before destructive agent actions" (#398) through the real pump: a
//! gated call is held unanswered until the user decides, then runs or is
//! declined.

use super::*;
use de_editor_core::ChatMessage;
use std::sync::mpsc::{channel, sync_channel, Receiver};

fn host_with_frame() -> WidgetHostNative {
    let mut host = WidgetHostNative::new();
    let doc = jian_ops_schema::load_str(
        r#"{"version":"1.0.0","children":[
          {"type":"frame","id":"hero","name":"Hero","x":0,"y":0,"width":100,"height":100}
        ]}"#,
    )
    .unwrap()
    .value;
    *host.editor_state_mut() = de_editor_core::EditorState::from_document(doc);
    host.editor_state_mut()
        .chat
        .messages
        .push(ChatMessage::assistant_streaming());
    host.editor_state_mut()
        .editor_ui
        .agent_settings
        .confirm_destructive_agent_actions = true;
    host
}

/// Send one `delete_node hero` request and pump once; returns the ack.
fn request_delete(
    host: &mut WidgetHostNative,
    current: &mut Option<ChatSession>,
    tool_tx: &std::sync::mpsc::Sender<de_editor_host_core::chat::ChatToolRequest>,
) -> Receiver<ChatToolResult> {
    let (ack_tx, ack_rx) = sync_channel(1);
    tool_tx
        .send(de_editor_host_core::chat::ChatToolRequest {
            name: "delete_node".into(),
            args_json: r#"{"nodeId":"hero"}"#.into(),
            ack: ack_tx,
        })
        .unwrap();
    pump(host, current, None, None, (1200.0, 800.0));
    ack_rx
}

#[test]
fn a_gated_call_waits_then_runs_on_allow() {
    let mut host = host_with_frame();
    let (_delta_tx, delta_rx) = channel::<de_ai::chat_provider::ChatDelta>();
    let (tool_tx, tool_rx) = channel();
    let mut current = Some(ChatSession::from_channels(delta_rx, Some(tool_rx)));
    let ack = request_delete(&mut host, &mut current, &tool_tx);
    assert!(ack.try_recv().is_err(), "held until the user answers");
    let prompt = host.editor_state().chat.permission_prompt.clone().unwrap();
    assert_eq!(prompt.target, "Hero");
    assert_eq!(host.editor_state().active_children().len(), 1);
    // Pumping again changes nothing while it waits.
    pump(&mut host, &mut current, None, None, (1200.0, 800.0));
    assert!(ack.try_recv().is_err());
    assert!(host.editor_state_mut().answer_agent_permission(true));
    pump(&mut host, &mut current, None, None, (1200.0, 800.0));
    let result = ack.try_recv().expect("runs after Allow");
    assert!(!result.is_error, "{}", result.content);
    assert!(host.editor_state().active_children().is_empty());
    assert!(host.editor_state().chat.permission_prompt.is_none());
}

#[test]
fn deny_answers_the_agent_and_keeps_the_layer() {
    let mut host = host_with_frame();
    let (_delta_tx, delta_rx) = channel::<de_ai::chat_provider::ChatDelta>();
    let (tool_tx, tool_rx) = channel();
    let mut current = Some(ChatSession::from_channels(delta_rx, Some(tool_rx)));
    let ack = request_delete(&mut host, &mut current, &tool_tx);
    host.editor_state_mut().answer_agent_permission(false);
    pump(&mut host, &mut current, None, None, (1200.0, 800.0));
    let result = ack.try_recv().expect("declined result");
    assert!(result.is_error);
    assert!(result.content.contains("declined"));
    assert_eq!(host.editor_state().active_children().len(), 1);
}

#[test]
fn with_the_setting_off_the_call_runs_at_once() {
    let mut host = host_with_frame();
    host.editor_state_mut()
        .editor_ui
        .agent_settings
        .confirm_destructive_agent_actions = false;
    let (_delta_tx, delta_rx) = channel::<de_ai::chat_provider::ChatDelta>();
    let (tool_tx, tool_rx) = channel();
    let mut current = Some(ChatSession::from_channels(delta_rx, Some(tool_rx)));
    let ack = request_delete(&mut host, &mut current, &tool_tx);
    assert!(ack.try_recv().is_ok());
    assert!(host.editor_state().chat.permission_prompt.is_none());
}
