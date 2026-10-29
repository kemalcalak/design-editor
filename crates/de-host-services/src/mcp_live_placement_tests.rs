//! #37 live-apply guard, split from `mcp_live_tests.rs` at the 800-line cap.

use super::*;

/// #37: a root-placed write falls back to the unplaced insert when the live
/// document moved on after the connection's snapshot, so the placement's
/// predicted ids can never move somebody else's node.
#[test]
fn pump_applies_the_unplaced_write_when_the_document_moved_on() {
    for (moved_on, expected_x) in [(false, 1280.0), (true, 200.0)] {
        let (req_tx, req_rx) = mpsc::channel();
        let (stop_tx, _stop_rx) = mpsc::channel();
        let mut server = McpLiveServer {
            port: 0,
            token: "test-token".to_string(),
            quit_flag: Arc::new(AtomicBool::new(false)),
            req_rx,
            stop_tx,
            client_identity: Arc::new(Mutex::new(None)),
            last_mcp_epoch: 0,
            pending_design_md: VecDeque::new(),
            saved_documents: Vec::new(),
            follow_target: None,
        };
        let src = r#"{"version":"1.0.0","children":[{"type":"frame","id":"seed","x":0,"y":0,
          "width":1200,"height":800,"children":[{"type":"text","id":"t","content":"Existing"}]}]}"#;
        let mut state =
            EditorState::from_document(jian_ops_schema::load_str(src).expect("doc").value);
        let insert = EditorCommand::InsertNode {
            kind: "rect".to_string(),
            name: "Box".to_string(),
            x: 200,
            y: 200,
            width: 400,
            height: 300,
            fill_hex: None,
            target_parent: de_editor_core::NodeId::NONE,
            page_id: None,
        };
        let mut snapshot = state.clone();
        let placed = de_mcp::root_placement::place_new_roots(
            &mut snapshot,
            "insert_node",
            &Default::default(),
            &insert,
        )
        .expect("placed");
        let revision = state.document_revision();
        if moved_on {
            assert!(state.apply(EditorCommand::UpdateNode {
                node_id: de_editor_core::NodeId::new("seed"),
                x: Some(10),
                y: None,
                width: None,
                height: None,
                name: None,
                fill_hex: None,
                page_id: None,
            }));
        }
        let (ack_tx, ack_rx) = mpsc::sync_channel(1);
        req_tx
            .send(UiRequest::Apply {
                tool_name: "insert_node".to_string(),
                cmd: placed,
                unplaced: Some(Box::new(UnplacedWrite {
                    revision,
                    command: insert,
                })),
                ack: ack_tx,
            })
            .expect("queue apply");
        let _ = server.pump(&mut state);
        assert!(ack_rx.try_recv().expect("acked").applied);
        let boxed = state
            .active_children()
            .iter()
            .find(|n| n.base().name.as_deref() == Some("Box"))
            .expect("box inserted");
        assert_eq!(boxed.base().x, Some(expected_x), "moved_on={moved_on}");
    }
}
