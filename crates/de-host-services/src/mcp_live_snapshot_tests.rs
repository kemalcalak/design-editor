//! Snapshot requests a connection stopped waiting for (#350). Split out of
//! `mcp_live_tests.rs` to keep that file under the 800-line cap.

use super::tests::fresh_mcp_server;
use super::*;

#[test]
fn a_timed_out_snapshot_request_is_not_copied_by_the_ui() {
    // A busy UI frame used to answer every queued snapshot request, even
    // ones whose connection had already given up, copying the whole
    // document for each and stalling the calls behind them.
    let (req_tx, req_rx) = mpsc::channel();
    let wake_ui: UiWake = Arc::new(|| {});
    let shared = LiveShared::default();
    let outcome = request_snapshot_within(&req_tx, &wake_ui, &shared, Duration::from_millis(10));
    assert!(outcome.is_err(), "nobody answered, so the wait times out");
    let Ok(UiRequest::Snapshot { abandoned, .. }) = req_rx.try_recv() else {
        panic!("the request stays queued for the UI");
    };
    assert!(abandoned.load(Ordering::Acquire), "the timeout marks it");
    let mut state = EditorState::new();
    assert!(snapshot_unless_abandoned(&abandoned, &mut state, None).is_none());
    let live = AtomicBool::new(false);
    assert!(snapshot_unless_abandoned(&live, &mut state, None).is_some());
}

#[test]
fn a_write_whose_connection_gave_up_still_lands() {
    // Only snapshots are skipped once abandoned. A write must land even
    // when its connection stopped waiting for the ack: dropping it would
    // lose the edit the client already sent.
    let (req_tx, req_rx) = mpsc::channel();
    let (stop_tx, _stop_rx) = mpsc::channel();
    let mut server = fresh_mcp_server(req_rx, stop_tx);
    let mut state = EditorState::new();

    let (snapshot_tx, snapshot_rx) = mpsc::sync_channel(1);
    req_tx
        .send(UiRequest::Snapshot {
            ack: snapshot_tx,
            abandoned: Arc::new(AtomicBool::new(true)),
        })
        .expect("queue snapshot");
    let (apply_tx, apply_rx) = mpsc::sync_channel(1);
    req_tx
        .send(UiRequest::Apply {
            tool_name: "insert_node".to_string(),
            cmd: EditorCommand::InsertNode {
                kind: "rect".into(),
                name: "Kept".into(),
                x: 0,
                y: 0,
                width: 10,
                height: 10,
                fill_hex: None,
                target_parent: de_editor_core::NodeId::NONE,
                page_id: None,
            },
            unplaced: None,
            ack: apply_tx,
        })
        .expect("queue apply");
    // The connection timed out on both: nobody reads either ack.
    drop(apply_rx);

    let roots_before = state.active_children().len();
    server.pump(&mut state);

    assert!(
        snapshot_rx.try_recv().is_err(),
        "the abandoned snapshot is not answered"
    );
    assert_eq!(
        state.active_children().len(),
        roots_before + 1,
        "the write landed"
    );
}
