use super::*;

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

fn pending(id: u64) -> (PendingPermission, mpsc::Receiver<bool>, Arc<AtomicBool>) {
    let (ack_tx, ack_rx) = mpsc::sync_channel(1);
    let expired = Arc::new(AtomicBool::new(false));
    let pending = PendingPermission {
        id,
        tool: "delete_node".into(),
        args: r#"{"nodeId":"cta"}"#.into(),
        action: DestructiveAction::DeleteLayer,
        ack: ack_tx,
        expired: Arc::clone(&expired),
        tab: None,
    };
    (pending, ack_rx, expired)
}

fn server() -> McpLiveServer {
    McpLiveServer::start(0).expect("bind an ephemeral port")
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
fn a_refused_call_gets_a_tool_error() {
    let (id, ..) = destructive_call(DELETE_CALL).unwrap();
    let declined = refused_response(&id, PermissionAnswer::Declined);
    assert!(declined.contains("declined"), "{declined}");
    assert!(declined.contains("\"id\":7"), "{declined}");
    let late = refused_response(&id, PermissionAnswer::TimedOut);
    assert!(late.contains("in time"), "{late}");
}

#[test]
fn the_pump_publishes_the_setting_and_answers_at_once_while_it_is_off() {
    let mut server = server();
    let mut state = EditorState::default();
    let (question, ack, _) = pending(1);
    server.pending_permissions.push(question);
    server.pump(&mut state);
    assert_eq!(ack.try_recv().ok(), Some(true));
    assert!(!server.ui_side.shared().asks_permission());
    server.pump(&mut asking_state());
    assert!(server.ui_side.shared().asks_permission());
    server.stop();
}

#[test]
fn the_pump_waits_for_the_user_and_passes_the_answer_on() {
    for allow in [true, false] {
        let mut server = server();
        let mut state = asking_state();
        let (question, ack, _) = pending(1);
        server.pending_permissions.push(question);
        assert!(server.pump(&mut state).repaint, "the prompt shows");
        assert!(ack.try_recv().is_err(), "no answer yet");
        assert!(
            !server.pump(&mut state).repaint,
            "a waiting prompt repaints nothing"
        );
        state.answer_agent_permission(allow);
        server.pump(&mut state);
        assert_eq!(ack.try_recv().ok(), Some(allow));
        assert!(!server.has_pending_permission());
        server.stop();
    }
}

#[test]
fn the_prompt_stays_on_its_tab_and_a_tab_switch_does_not_ask_twice() {
    let mut server = server();
    let mut state = asking_state();
    let (question, ack, _) = pending(1);
    server.pending_permissions.push(question);
    server.pump(&mut state);
    let first = state.chat.active_index();
    state.chat.new_tab();
    server.pump(&mut state);
    assert!(
        state.chat.permission_prompt.is_none(),
        "no duplicate on the tab now on screen"
    );
    state.chat.switch_to(first);
    state.answer_agent_permission(true);
    server.pump(&mut state);
    assert_eq!(ack.try_recv().ok(), Some(true));
    server.stop();
}

#[test]
fn a_hidden_chat_raises_the_waiting_notice() {
    let mut server = server();
    let mut state = asking_state();
    state.chat.minimize();
    let (question, _ack, _) = pending(1);
    server.pending_permissions.push(question);
    server.pump(&mut state);
    let toast = state.editor_ui.editor_toast.clone().expect("notice");
    assert_eq!(toast.i18n_key, WAITING_TOAST);
    server.stop();
}

#[test]
fn an_expired_wait_closes_its_prompt_and_a_late_allow_runs_nothing() {
    let mut server = server();
    let mut state = asking_state();
    let (question, ack, expired) = pending(1);
    server.pending_permissions.push(question);
    server.pump(&mut state);
    assert!(state.chat.permission_prompt.is_some());
    expired.store(true, Ordering::SeqCst);
    server.pump(&mut state);
    assert!(state.chat.permission_prompt.is_none(), "prompt closed");
    assert!(!server.has_pending_permission());
    assert!(!state.answer_agent_permission(true));
    assert!(ack.try_recv().is_err(), "nothing was sent to run it");
    server.stop();
}

#[test]
fn a_connection_gives_up_at_its_deadline() {
    let (req_tx, req_rx) = mpsc::channel();
    let wake: UiWake = Arc::new(|| {});
    let answer = request_permission(
        &req_tx,
        &wake,
        "delete_node".into(),
        "{}".into(),
        DestructiveAction::DeleteLayer,
        Duration::from_millis(20),
    );
    assert_eq!(answer, PermissionAnswer::TimedOut);
    let Ok(UiRequest::Permission(question)) = req_rx.try_recv() else {
        panic!("the question reached the UI side");
    };
    assert!(question.expired.load(Ordering::SeqCst));
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
            PERMISSION_DEADLINE,
        )
    });
    let question = req_rx.recv().expect("asked");
    drop(question);
    drop(req_rx);
    assert_eq!(asker.join().unwrap(), PermissionAnswer::Declined);
}

#[test]
fn a_fresh_server_asks_before_its_first_pump() {
    let mut server = server();
    assert!(!server.ui_side.shared().asks_permission());
    server.note_editor(&asking_state());
    assert!(server.ui_side.shared().asks_permission());
    server.stop();
}

fn execute_body(script: &str) -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": 9,
        "method": "tools/call",
        "params": {"name": "execute", "arguments": {"script": script}},
    })
    .to_string()
}

fn state_with_hero() -> EditorState {
    let doc = jian_ops_schema::load_str(
        r#"{"version":"1.0.0","children":[
          {"type":"frame","id":"hero","name":"Hero","x":0,"y":0,"width":100,"height":100}
        ]}"#,
    )
    .unwrap()
    .value;
    EditorState::from_document(doc)
}

#[test]
fn an_execute_script_is_gated_by_what_its_dry_run_does() {
    let wake: UiWake = Arc::new(|| {});
    let (req_tx, req_rx) = mpsc::channel();
    let mut state = state_with_hero();
    // A read-only script runs without a question, and the state is put back.
    assert!(gate_execute(&execute_body("Get('hero')"), &mut state, &req_tx, &wake).is_none());
    assert!(req_rx.try_recv().is_err());
    assert_eq!(state.active_children().len(), 1);
    // A delete spelled through globalThis asks; the user says no.
    let answerer = std::thread::spawn(move || {
        let Ok(UiRequest::Permission(question)) = req_rx.recv() else {
            panic!("asked");
        };
        assert_eq!(question.action, DestructiveAction::ScriptedBatch);
        question.ack.send(false).unwrap();
    });
    let reply = gate_execute(
        &execute_body("globalThis['Del' + 'ete']('hero')"),
        &mut state,
        &req_tx,
        &wake,
    )
    .expect("refused");
    assert!(reply.contains("declined"), "{reply}");
    answerer.join().unwrap();
    assert_eq!(
        state.active_children().len(),
        1,
        "the dry run applied nothing"
    );
}

#[test]
fn an_execute_aimed_at_another_file_asks_before_the_lock() {
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/call",
        "params": {"name": "execute", "arguments": {"script": "Get('a')", "filePath": "/tmp/x.de"}},
    })
    .to_string();
    assert!(destructive_call(&body).is_some());
    assert!(destructive_call(&execute_body("Delete('a')")).is_none());
}
