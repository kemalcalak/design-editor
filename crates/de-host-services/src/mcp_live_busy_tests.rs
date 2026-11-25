//! `tools/list` without the document, and busy answers instead of blocked
//! clients (#406). Driven through the real `serve_connection` router.

use super::*;

const PORT: u16 = 51235;
const TOOLS_LIST: &str = r#"{"jsonrpc":"2.0","id":4,"method":"tools/list"}"#;
const GET_NODE_CALL: &str = r#"{"jsonrpc":"2.0","id":9,"method":"tools/call","params":{"name":"get_document_info","arguments":{}}}"#;

struct MockStream {
    input: std::io::Cursor<Vec<u8>>,
    output: Vec<u8>,
}

impl std::io::Read for MockStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        std::io::Read::read(&mut self.input, buf)
    }
}

impl std::io::Write for MockStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.output.extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Serve one JSON-RPC `body` and return the response body, or panic with
/// the whole HTTP response.
fn serve(body: &str, req_tx: &Sender<UiRequest>, shared: &LiveShared) -> String {
    let request = format!(
        "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1:{PORT}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let admission = LiveAdmission::new("tok".to_string(), PORT);
    let wake_ui: UiWake = Arc::new(|| {});
    let mut stream = MockStream {
        input: std::io::Cursor::new(request.into_bytes()),
        output: Vec::new(),
    };
    serve_connection(
        &mut stream,
        req_tx,
        &admission,
        shared,
        &AtomicBool::new(false),
        &wake_ui,
        &Mutex::new(None),
    )
    .expect("the request is answered on the wire");
    let raw = String::from_utf8_lossy(&stream.output).into_owned();
    raw.split_once("\r\n\r\n")
        .map(|(_, body)| body.to_string())
        .unwrap_or_else(|| panic!("no HTTP body in {raw}"))
}

fn tool_names(response: &str) -> Vec<String> {
    let value: serde_json::Value = serde_json::from_str(response).expect("valid JSON");
    value["result"]["tools"]
        .as_array()
        .expect("tools array")
        .iter()
        .map(|tool| tool["name"].as_str().expect("name").to_string())
        .collect()
}

#[test]
fn tools_list_answers_while_another_request_holds_the_editor() {
    let (req_tx, req_rx) = mpsc::channel();
    let shared = LiveShared::default();
    let _held = shared
        .lock_state("export_frames")
        .expect("free lock is taken");
    let started = Instant::now();
    let response = serve(TOOLS_LIST, &req_tx, &shared);
    assert!(
        started.elapsed() < Duration::from_millis(100),
        "tools/list must not wait for the state lock"
    );
    assert!(tool_names(&response).contains(&"batch_design".to_string()));
    assert!(
        req_rx.try_recv().is_err(),
        "tools/list must not ask the UI thread for anything"
    );
}

#[test]
fn tools_list_answers_while_the_ui_never_answers() {
    // The receiver stays alive and unread: a UI thread stuck loading.
    let (req_tx, req_rx) = mpsc::channel();
    let shared = LiveShared::default();
    let started = Instant::now();
    let response = serve(TOOLS_LIST, &req_tx, &shared);
    assert!(started.elapsed() < Duration::from_millis(100));
    assert!(response.contains(r#""id":4"#), "{response}");
    assert!(req_rx.try_recv().is_err());
}

#[test]
fn cached_tools_list_equals_the_document_built_one() {
    // What `tools/list` answered before #406: the static catalog plus one
    // schema per kit component of the live state, built per request.
    let state = EditorState::new();
    let mut entries: Vec<String> = crate::mcp_serve::TOOL_SCHEMAS
        .iter()
        .map(|schema| de_mcp::root_placement::advertise_root_placement(schema))
        .collect();
    entries.extend(
        de_mcp::element_tools::element_tool_schemas(&state)
            .iter()
            .map(|schema| de_mcp::root_placement::advertise_root_placement(schema)),
    );
    // pen.dev's names (#392) close the catalog.
    entries.extend(
        de_mcp::pen_dev_aliases::PEN_DEV_ALIAS_SCHEMAS
            .iter()
            .map(|schema| (*schema).to_string()),
    );
    let before = format!(
        r#"{{"jsonrpc":"2.0","id":4,"result":{{"tools":[{}]}}}}"#,
        entries.join(",")
    );
    let (req_tx, _req_rx) = mpsc::channel();
    // Twice: the second answer comes from the cache.
    for _ in 0..2 {
        let response = serve(TOOLS_LIST, &req_tx, &LiveShared::default());
        if de_mcp::debug_tools_enabled() {
            assert!(response.starts_with(&before[..before.len() - 3]));
        } else {
            assert_eq!(response, before);
        }
    }
}

#[test]
fn an_imported_kit_reaches_tools_list_once_the_ui_pumps() {
    let (req_tx, _req_rx) = mpsc::channel();
    let mut ui_side = LiveUiSide::default();
    let shared = Arc::clone(ui_side.shared());
    let mut kits = de_editor_core::builtin_kits();
    let mut imported = kits[0].clone();
    imported.id = "imported-kit".into();
    imported.built_in = false;
    imported.components.truncate(1);
    imported.components[0].id = "imported-hero".into();
    kits.push(imported);

    let before = tool_names(&serve(TOOLS_LIST, &req_tx, &shared));
    assert!(!before.contains(&"insert_imported_hero".to_string()));
    ui_side.pumped(&kits);
    let after = tool_names(&serve(TOOLS_LIST, &req_tx, &shared));
    assert!(after.contains(&"insert_imported_hero".to_string()));
    assert_eq!(after.len(), before.len() + 1);
}

#[test]
fn a_call_while_the_ui_is_stuck_is_answered_busy_within_the_bound() {
    let (req_tx, req_rx) = mpsc::channel();
    let shared = LiveShared::default();

    let started = Instant::now();
    let first = serve(GET_NODE_CALL, &req_tx, &shared);
    let first_took = started.elapsed();
    let value: serde_json::Value = serde_json::from_str(&first).expect("JSON-RPC");
    assert_eq!(value["id"], 9, "{first}");
    assert_eq!(value["error"]["code"], -32000, "{first}");
    assert_eq!(value["error"]["data"]["reason"], "editor-busy", "{first}");
    assert_eq!(value["error"]["data"]["retryAfterMs"], 2000, "{first}");
    assert!(
        first_took >= UI_BUSY_WAIT && first_took < UI_BUSY_WAIT + Duration::from_secs(1),
        "the first call waits the busy bound, not the old 15 s ack: {first_took:?}"
    );

    // The UI is now known to be stuck: the next call is told within a
    // second instead of waiting the whole bound again.
    let started = Instant::now();
    let second = serve(GET_NODE_CALL, &req_tx, &shared);
    assert!(started.elapsed() < Duration::from_secs(1));
    assert!(second.contains("editor-busy"), "{second}");

    // Both queued snapshot requests were abandoned, so the UI skips their
    // copies when it comes back (#350).
    for _ in 0..2 {
        let Ok(UiRequest::Snapshot { abandoned, .. }) = req_rx.try_recv() else {
            panic!("each call queued one snapshot request");
        };
        assert!(abandoned.load(Ordering::Acquire));
    }
}

#[test]
fn a_pump_lets_reads_wait_their_full_time_again() {
    let mut ui_side = LiveUiSide::default();
    let shared = Arc::clone(ui_side.shared());
    shared.mark_ui_stalled();
    assert_eq!(shared.ui_wait(UI_BUSY_WAIT), UI_STALLED_WAIT);
    ui_side.pumped(&de_editor_core::builtin_kits());
    assert_eq!(shared.ui_wait(UI_BUSY_WAIT), UI_BUSY_WAIT);
}

#[test]
fn a_held_lock_turns_the_next_request_away_naming_its_holder() {
    let shared = LiveShared::default();
    let held = shared.lock_state("export_frames").expect("free");
    let Err(McpLiveError::Busy(busy)) =
        shared.lock_state_within("get_node", Duration::from_millis(30))
    else {
        panic!("the second request must not get the lock");
    };
    let EditorBusy::StateLock { holder, .. } = &busy else {
        panic!("a lock wait, not a UI wait: {busy:?}");
    };
    assert_eq!(holder.as_deref(), Some("export_frames"));
    assert_eq!(busy.reason(), "request-in-progress");
    assert!(busy.to_string().contains("export_frames"), "{busy}");
    drop(held);
    assert!(shared.lock_state_within("get_node", Duration::ZERO).is_ok());
}

#[test]
fn the_watchdog_reports_a_long_hold_once() {
    let shared = LiveShared::default();
    assert!(!shared.watchdog_after(Duration::ZERO), "nothing held");
    let _held = shared.lock_state("batch_design").expect("free");
    assert!(!shared.watchdog_after(Duration::from_secs(60)));
    assert!(shared.watchdog_after(Duration::ZERO));
    assert!(!shared.watchdog_after(Duration::ZERO), "once per hold");
}

#[test]
fn a_released_lock_wakes_its_waiter() {
    let shared = Arc::new(LiveShared::default());
    let held = shared.lock_state("export_frames").expect("free");
    let waiter = {
        let shared = Arc::clone(&shared);
        thread::spawn(move || {
            shared
                .lock_state_within("get_node", Duration::from_secs(5))
                .map(|_guard| ())
                .is_ok()
        })
    };
    thread::sleep(Duration::from_millis(20));
    drop(held);
    assert!(
        waiter.join().expect("waiter"),
        "the waiter gets the lock once released"
    );
}

#[test]
fn a_page_switch_is_not_queued_behind_a_stuck_ui() {
    // A write cannot be withdrawn once queued, so while the UI is known to
    // be stuck it is answered busy instead of waiting the 15 s apply ack.
    let (req_tx, req_rx) = mpsc::channel();
    let shared = LiveShared::default();
    shared.mark_ui_stalled();
    let call = r#"{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"set_active_page","arguments":{"index":"1"}}}"#;
    let started = Instant::now();
    let response = serve(call, &req_tx, &shared);
    assert!(started.elapsed() < Duration::from_millis(100));
    assert!(response.contains("editor-busy"), "{response}");
    assert!(req_rx.try_recv().is_err(), "nothing was queued");
}

#[test]
fn the_label_names_the_tool_and_a_foreign_file() {
    assert_eq!(
        request_label(GET_NODE_CALL),
        "get_document_info".to_string()
    );
    assert_eq!(
        request_label(
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"list_pages","arguments":{"filePath":"/tmp/big.de"}}}"#
        ),
        "list_pages on /tmp/big.de".to_string()
    );
    assert_eq!(
        request_label(
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"list_pages","arguments":{"filePath":"live://canvas"}}}"#
        ),
        "list_pages".to_string()
    );
    assert_eq!(request_label("not json"), "MCP request");
}

#[test]
fn a_busy_answer_to_a_notification_has_no_body() {
    assert!(crate::mcp_serve::busy_error_response(
        r#"{"jsonrpc":"2.0","method":"tools/call","params":{}}"#,
        "busy",
        "editor-busy",
        2000
    )
    .is_none());
}

#[test]
fn snapshot_copies_are_counted() {
    let (req_tx, req_rx) = mpsc::channel();
    let wake_ui: UiWake = Arc::new(|| {});
    let (copies_before, _) = snapshot_copy_stats();
    let responder = thread::spawn(move || {
        if let Ok(UiRequest::Snapshot { ack, .. }) = req_rx.recv_timeout(Duration::from_secs(5)) {
            let _ = ack.send(LiveSnapshot {
                snapshot: de_editor_core::request_snapshot::shared_snapshot(
                    &mut EditorState::new(),
                    None,
                ),
                document_epoch: 0,
            });
        }
    });
    request_snapshot(&req_tx, &wake_ui, &LiveShared::default()).expect("answered");
    responder.join().expect("responder");
    assert!(snapshot_copy_stats().0 > copies_before);
}
