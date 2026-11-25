//! Measurement, not a regression gate (#406): a ~200k-node document driven
//! through the live snapshot path many times, printing the UI-side capture
//! time, the connection-side copy time and the process RSS as it goes, so
//! growth per call shows up. Run it by hand:
//!
//! `cargo test -p de-host-services --release --lib large_document_snapshot_memory -- --ignored --nocapture`

use super::tests::fresh_mcp_server;
use super::*;

/// 14 pages like the reported document; nodes per page set the total.
const PAGES: usize = 14;
const BOARDS_PER_PAGE: usize = 100;
const ROWS_PER_BOARD: usize = 46;
const CALLS: usize = 200;

fn rss_mb() -> f64 {
    let out = std::process::Command::new("ps")
        .args(["-o", "rss=", "-p", &std::process::id().to_string()])
        .output()
        .expect("ps");
    String::from_utf8_lossy(&out.stdout)
        .trim()
        .parse::<f64>()
        .unwrap_or(0.0)
        / 1024.0
}

/// One page: boards of rows, each row a frame holding a rectangle and a
/// text, so the tree is deep and wide like a real screen design.
fn page_json(page: usize) -> serde_json::Value {
    let boards: Vec<serde_json::Value> = (0..BOARDS_PER_PAGE)
        .map(|board| {
            let rows: Vec<serde_json::Value> = (0..ROWS_PER_BOARD)
                .map(|row| {
                    let id = format!("p{page}b{board}r{row}");
                    serde_json::json!({
                        "type": "frame", "id": id, "name": "Row", "width": 360, "height": 40,
                        "layout": "horizontal", "gap": 8,
                        "children": [
                            {"type": "rectangle", "id": format!("{id}-icon"), "width": 24, "height": 24,
                             "fill": [{"type": "solid", "color": "#3355FF"}], "cornerRadius": 6},
                            {"type": "text", "id": format!("{id}-label"), "content": format!("Row {row} of board {board}"),
                             "fontSize": 14, "fill": [{"type": "solid", "color": "#111111"}]}
                        ]
                    })
                })
                .collect();
            serde_json::json!({
                "type": "frame", "id": format!("p{page}b{board}"), "name": format!("Board {board}"),
                "x": (board % 10) * 420, "y": (board / 10) * 2000, "width": 400, "height": 1900,
                "layout": "vertical", "children": rows
            })
        })
        .collect();
    serde_json::json!({"id": format!("page-{page}"), "name": format!("Page {page}"), "children": boards})
}

#[test]
#[ignore = "measurement; run by hand with --release --nocapture"]
fn large_document_snapshot_memory() {
    let rss_start = rss_mb();
    let pages: Vec<serde_json::Value> = (0..PAGES).map(page_json).collect();
    let doc_json =
        serde_json::json!({"version": "1.0", "pages": pages, "children": []}).to_string();
    let doc: jian_ops_schema::PenDocument = serde_json::from_str(&doc_json).expect("document");
    let mut state = EditorState::new();
    state.replace_document(doc);
    let nodes = PAGES * BOARDS_PER_PAGE * (1 + ROWS_PER_BOARD * 3);
    println!(
        "document: {nodes} nodes, {:.0} MB JSON; RSS {:.0} MB → {:.0} MB after load",
        doc_json.len() as f64 / 1e6,
        rss_start,
        rss_mb()
    );
    drop(doc_json);

    let started = Instant::now();
    let catalog = de_mcp::element_tools::KitToolCatalog::builtin();
    let first = crate::mcp_serve::tools_list_response_for("1", catalog, false);
    let cold = started.elapsed();
    let started = Instant::now();
    let _ = crate::mcp_serve::tools_list_response_for("2", catalog, false);
    println!(
        "tools/list: {} KB, cold {:?}, cached {:?}",
        first.len() / 1024,
        cold,
        started.elapsed()
    );

    let (req_tx, req_rx) = mpsc::channel();
    let (stop_tx, _stop_rx) = mpsc::channel();
    let mut server = fresh_mcp_server(req_rx, stop_tx);
    let done = Arc::new(AtomicBool::new(false));
    let client_done = Arc::clone(&done);
    let client = thread::spawn(move || {
        let wake_ui: UiWake = Arc::new(|| {});
        let shared = LiveShared::default();
        let mut slowest = Duration::ZERO;
        for call in 1..=CALLS {
            let started = Instant::now();
            let state = request_snapshot_within(&req_tx, &wake_ui, &shared, UI_ACK_TIMEOUT)
                .expect("snapshot");
            slowest = slowest.max(started.elapsed());
            drop(state);
            if call == 1 || call % 50 == 0 {
                let (copies, copy_ms) = snapshot_copy_stats();
                println!(
                    "call {call}: RSS {:.0} MB, {copies} copies, {:.0} ms per copy, slowest call {slowest:?}",
                    rss_mb(),
                    copy_ms as f64 / copies.max(1) as f64
                );
            }
        }
        client_done.store(true, Ordering::Release);
    });
    let mut pump_time = Duration::ZERO;
    let mut pumps = 0u32;
    while !done.load(Ordering::Acquire) {
        let started = Instant::now();
        server.pump(&mut state);
        pump_time += started.elapsed();
        pumps += 1;
        thread::sleep(Duration::from_millis(1));
    }
    client.join().expect("client");
    println!(
        "UI thread: {:?} in {pumps} pumps for {CALLS} snapshots ({:?} per snapshot)",
        pump_time,
        pump_time / CALLS as u32
    );
    println!("RSS at the end: {:.0} MB", rss_mb());
}
