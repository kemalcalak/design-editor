use super::*;
use std::sync::Arc;

const DELETE_CALL: &str = r#"{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"delete_node","arguments":{"nodeId":"cta"}}}"#;
const READ_CALL: &str = r#"{"jsonrpc":"2.0","id":8,"method":"tools/call","params":{"name":"get_document_info","arguments":{}}}"#;

fn asking_state() -> EditorState {
    let mut state = EditorState::default();
    state
        .editor_ui
        .agent_settings
        .confirm_destructive_agent_actions = true;
    state
}

#[test]
fn only_destructive_calls_ask() {
    let (_, tool, args, action) = destructive_call(DELETE_CALL).expect("a delete asks");
    assert_eq!(tool, "delete_node");
    assert!(args.contains("cta"));
    assert_eq!(action, DestructiveAction::DeleteLayer);
    assert!(destructive_call(READ_CALL).is_none());
}

#[test]
fn a_declined_call_gets_the_tool_error() {
    let (id, ..) = destructive_call(DELETE_CALL).unwrap();
    let reply = declined_response(&id);
    assert!(reply.contains("declined"), "{reply}");
    assert!(reply.contains("\"id\":7"), "{reply}");
}

/// A server whose pump settles `pending` against `state`.
fn settle(state: &mut EditorState, answer: Option<bool>) -> Option<bool> {
    let mut server = McpLiveServer::start(0).expect("bind an ephemeral port");
    let (ack_tx, ack_rx) = mpsc::sync_channel(1);
    server.pending_permissions.push(PendingPermission {
        tool: "delete_node".into(),
        args: r#"{"nodeId":"cta"}"#.into(),
        action: DestructiveAction::DeleteLayer,
        ack: ack_tx,
    });
    let outcome = server.pump(state);
    if let Some(allow) = answer {
        assert!(outcome.repaint, "the prompt shows");
        assert!(server.has_pending_permission());
        assert!(ack_rx.try_recv().is_err(), "no answer yet");
        state.answer_agent_permission(allow);
        server.pump(state);
    }
    let got = ack_rx.try_recv().ok();
    server.stop();
    got
}

#[test]
fn the_pump_answers_at_once_while_the_setting_is_off() {
    assert_eq!(settle(&mut EditorState::default(), None), Some(true));
}

#[test]
fn the_pump_waits_for_the_user_and_passes_the_answer_on() {
    assert_eq!(settle(&mut asking_state(), Some(true)), Some(true));
    assert_eq!(settle(&mut asking_state(), Some(false)), Some(false));
}

#[test]
fn a_stopped_server_declines_a_waiting_question() {
    let (req_tx, req_rx) = mpsc::channel();
    let wake: UiWake = Arc::new(|| {});
    let asker = std::thread::spawn(move || {
        request_permission(
            &req_tx,
            &wake,
            "delete_node".into(),
            "{}".into(),
            DestructiveAction::DeleteLayer,
        )
    });
    // The UI side receives the question, then the server goes away.
    let question = req_rx.recv().expect("asked");
    drop(question);
    drop(req_rx);
    assert!(!asker.join().unwrap());
}
