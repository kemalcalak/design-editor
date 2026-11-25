//! `save_document` reports reaching the host (#403). Split out of
//! `mcp_live_tests.rs` to keep that file under the 800-line cap.

use super::tests::fresh_mcp_server;
use super::*;

#[test]
fn a_save_document_reply_reaches_the_host_through_the_pump() {
    let (req_tx, req_rx) = mpsc::channel();
    let (stop_tx, _stop_rx) = mpsc::channel();
    let woke = Arc::new(AtomicUsize::new(0));
    let wake_ui: UiWake = {
        let woke = Arc::clone(&woke);
        Arc::new(move || {
            woke.fetch_add(1, Ordering::AcqRel);
        })
    };
    let line = r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"save_document","arguments":{"filePath":"/tmp/agent.de"}}}"#;
    let reply = serde_json::json!({"jsonrpc":"2.0","id":3,"result":{"content":[
        {"type":"text","text":r#"{"ok":"true","filePath":"/tmp/agent.de"}"#}
    ]}})
    .to_string();
    let live = McpSavedRevision {
        document_epoch: 2,
        generation: 1,
        revision: 7,
    };
    report_saved_document(line, &reply, Some(live), &req_tx, &wake_ui);
    let other = r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"get_node","arguments":{}}}"#;
    report_saved_document(other, &reply, Some(live), &req_tx, &wake_ui);
    // A `sourceFilePath` copy reports no live revision.
    report_saved_document(line, &reply, None, &req_tx, &wake_ui);
    assert_eq!(
        woke.load(Ordering::Acquire),
        2,
        "only the saves wake the UI"
    );

    let mut server = fresh_mcp_server(req_rx, stop_tx);
    let mut state = EditorState::new();
    let outcome = server.pump(&mut state);
    assert!(outcome.repaint);
    assert!(
        !outcome.document_edited,
        "a save does not edit the live document"
    );
    let path = std::path::PathBuf::from("/tmp/agent.de");
    assert_eq!(
        server.take_saved_documents(),
        vec![
            McpSavedDocument {
                path: path.clone(),
                saved_from: Some(live),
            },
            McpSavedDocument {
                path,
                saved_from: None,
            },
        ]
    );
    assert!(server.take_saved_documents().is_empty());
}

#[test]
fn a_snapshot_carries_the_host_document_epoch() {
    let (req_tx, req_rx) = mpsc::channel();
    let (stop_tx, _stop_rx) = mpsc::channel();
    let mut server = fresh_mcp_server(req_rx, stop_tx);
    server.set_document_epoch(5);
    let wake_ui: UiWake = Arc::new(|| {});
    let requester = thread::spawn(move || {
        request_snapshot_with_epoch(&req_tx, &wake_ui, &LiveShared::default())
    });
    let mut state = EditorState::new();
    state.mark_document_changed();
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while !requester.is_finished() && std::time::Instant::now() < deadline {
        server.pump(&mut state);
        thread::sleep(Duration::from_millis(1));
    }
    let (snapshot, epoch) = requester
        .join()
        .expect("requester thread should not panic")
        .expect("snapshot request should complete");
    assert_eq!(epoch, 5);
    assert_eq!(snapshot.document_generation(), state.document_generation());
    assert_eq!(snapshot.document_revision(), state.document_revision());
}

fn save_call(arguments: serde_json::Value) -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": 9,
        "method": "tools/call",
        "params": {"name": "save_document", "arguments": arguments},
    })
    .to_string()
}

fn scratch_dir(label: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("de-mcp-live-save-{label}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

/// The saved documents the connection reported, after answering any
/// snapshot request with a state at `epoch`.
fn reported_saves(
    req_rx: Receiver<UiRequest>,
    epoch: u64,
    state: EditorState,
) -> thread::JoinHandle<Vec<McpSavedDocument>> {
    thread::spawn(move || {
        let mut state = state;
        let mut saved = Vec::new();
        while let Ok(request) = req_rx.recv_timeout(Duration::from_secs(10)) {
            match request {
                UiRequest::Snapshot { ack, .. } => {
                    let _ = ack.send(LiveSnapshot {
                        snapshot: de_editor_core::request_snapshot::shared_snapshot(
                            &mut state, None,
                        ),
                        document_epoch: epoch,
                    });
                }
                UiRequest::DocumentSaved(document) => saved.push(document),
                _ => break,
            }
        }
        saved
    })
}

#[test]
fn a_live_save_reports_the_snapshot_it_wrote() {
    use super::admission::tests::{drive, request, PORT};
    let dir = scratch_dir("live");
    let target = dir.join("agent.de");
    let mut state = EditorState::new();
    state.mark_document_changed();
    state.mark_document_changed();
    let (generation, revision) = (state.document_generation(), state.document_revision());
    let (req_tx, req_rx) = mpsc::channel();
    let saves = reported_saves(req_rx, 4, state);

    let body = save_call(serde_json::json!({"filePath": target.display().to_string()}));
    let headers = format!("Host: 127.0.0.1:{PORT}\r\n");
    let response = drive(&request("/mcp", &headers, &body), &req_tx);
    drop(req_tx);

    assert!(response.contains(r#"\"ok\":\"true\""#), "{response}");
    assert_eq!(
        saves.join().expect("responder"),
        vec![McpSavedDocument {
            path: target.clone(),
            saved_from: Some(McpSavedRevision {
                document_epoch: 4,
                generation,
                revision,
            }),
        }]
    );
    assert!(target.exists());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_copy_of_another_file_reports_no_live_revision() {
    use super::admission::tests::{drive, request, PORT};
    let dir = scratch_dir("copy");
    let source = dir.join("source.de");
    std::fs::write(&source, r#"{"version":"1.0.0","children":[]}"#).expect("source");
    let target = dir.join("copy.de");
    let (req_tx, req_rx) = mpsc::channel();
    let saves = reported_saves(req_rx, 4, EditorState::new());

    let body = save_call(serde_json::json!({
        "filePath": target.display().to_string(),
        "sourceFilePath": source.display().to_string(),
    }));
    let headers = format!("Host: 127.0.0.1:{PORT}\r\n");
    let response = drive(&request("/mcp", &headers, &body), &req_tx);
    drop(req_tx);

    assert!(response.contains(r#"\"ok\":\"true\""#), "{response}");
    assert_eq!(
        saves.join().expect("responder"),
        vec![McpSavedDocument {
            path: target.clone(),
            saved_from: None,
        }]
    );
    let _ = std::fs::remove_dir_all(dir);
}
