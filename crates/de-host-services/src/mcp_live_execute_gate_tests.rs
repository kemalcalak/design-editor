//! A gated live `execute` (#398) through a real connection: the state lock
//! is free while the user is asked, and a document that changed meanwhile
//! cannot make the real run do what the prompt did not show.

use super::tests::fresh_mcp_server;
use super::*;
use de_editor_core::{EditorCommand, NodeId};

fn live_state() -> EditorState {
    let doc = jian_ops_schema::load_str(
        r#"{"version":"1.0.0","children":[
          {"type":"frame","id":"hero","name":"Hero","x":0,"y":0,"width":100,"height":100},
          {"type":"frame","id":"side","name":"Side","x":200,"y":0,"width":100,"height":100}
        ]}"#,
    )
    .unwrap()
    .value;
    let mut state = EditorState::from_document(doc);
    state
        .editor_ui
        .agent_settings
        .confirm_destructive_agent_actions = true;
    state
}

fn execute_line(script: &str) -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {"name": "execute", "arguments": {"script": script}},
    })
    .to_string()
}

fn ids(state: &EditorState) -> Vec<String> {
    state
        .active_children()
        .iter()
        .map(|node| de_editor_core::PenNodeExt::id_str(node).to_string())
        .collect()
}

/// Serve `line` on a connection thread while this thread pumps the UI
/// side; `on_prompt` runs once, when the prompt opens, and answers it.
fn serve_with_prompt(
    line: String,
    live: &mut EditorState,
    on_prompt: impl FnOnce(&LiveShared, &mut EditorState),
) -> (String, bool) {
    let (req_tx, req_rx) = mpsc::channel();
    let (stop_tx, _stop_rx) = mpsc::channel();
    let mut server = fresh_mcp_server(req_rx, stop_tx);
    server.note_editor(live);
    let shared = Arc::clone(server.ui_side.shared());
    let client_shared = Arc::clone(&shared);
    let done = Arc::new(AtomicBool::new(false));
    let client_done = Arc::clone(&done);
    let client = thread::spawn(move || {
        let wake_ui: UiWake = Arc::new(|| {});
        let reply = connection::serve_stateful(&line, &req_tx, &client_shared, &wake_ui);
        client_done.store(true, Ordering::Release);
        reply
    });
    let mut on_prompt = Some(on_prompt);
    let deadline = Instant::now() + Duration::from_secs(30);
    while !done.load(Ordering::Acquire) && Instant::now() < deadline {
        server.pump(live);
        if live.chat.permission_prompt.is_some() {
            if let Some(answer) = on_prompt.take() {
                answer(&shared, live);
            }
        }
        thread::sleep(Duration::from_millis(1));
    }
    let reply = client.join().expect("client").expect("reply");
    (reply, on_prompt.is_none())
}

#[test]
fn the_lock_is_free_while_the_user_is_asked() {
    let mut live = live_state();
    let (reply, asked) =
        serve_with_prompt(execute_line("Delete('hero')"), &mut live, |shared, live| {
            let probe = shared.lock_state_within("probe", Duration::from_millis(50));
            assert!(
                probe.is_ok(),
                "another client gets the lock during the prompt"
            );
            drop(probe);
            live.answer_agent_permission(true);
        });
    assert!(asked);
    assert!(!reply.contains("\"isError\":true"), "{reply}");
    assert_eq!(ids(&live), ["side"], "the allowed delete ran");
}

#[test]
fn a_document_changed_during_the_prompt_cannot_widen_the_run() {
    let mut live = live_state();
    // The dry run deletes `hero`; once `hero` is renamed the same script
    // would delete `side`, which the prompt never showed.
    let script = "Delete(Get('hero').name === 'Hero' ? 'hero' : 'side')";
    let (reply, asked) = serve_with_prompt(execute_line(script), &mut live, |_, live| {
        assert!(live.apply(EditorCommand::SetNodeName {
            node_id: NodeId::new("hero"),
            name: "Top".into(),
        }));
        live.answer_agent_permission(true);
    });
    assert!(asked);
    assert!(reply.contains("permission prompt"), "{reply}");
    assert_eq!(ids(&live), ["hero", "side"], "nothing was applied");
}

#[test]
fn a_random_branch_does_what_its_dry_run_did() {
    // Whatever branch the dry run took, the real run takes it too: it is
    // either not asked about and deletes nothing, or asked and deletes.
    for _ in 0..4 {
        let mut live = live_state();
        let script = "if (Math.random() < 0.5) Delete('hero'); else Update('side', {name: 'S'})";
        let (reply, asked) = serve_with_prompt(execute_line(script), &mut live, |_, live| {
            live.answer_agent_permission(true);
        });
        assert!(!reply.contains("\"isError\":true"), "{reply}");
        assert_eq!(ids(&live).len(), if asked { 1 } else { 2 });
    }
}
