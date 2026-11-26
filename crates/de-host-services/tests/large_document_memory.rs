//! Large-document memory and latency benchmarks for live MCP calls (#456).
//!
//! Measurement, not a gate: run by hand in a release build and compare the
//! printed tables before and after a change.
//!
//! `cargo test --release -p de-host-services --test large_document_memory -- --ignored --nocapture --test-threads=1`
//!
//! A counting global allocator gives exact heap numbers on every platform,
//! so the "extra memory per call" figure does not depend on how eagerly the
//! system allocator hands pages back. The process footprint
//! (`de_host_services::process_memory`) is printed next to it.
//!
//! Each call goes the way a live MCP call goes: the UI side shares what the
//! call reads (`de_mcp::snapshot_pages` → `request_snapshot_pages::kept_pages`
//! → `request_snapshot::shared_snapshot_pages`, anchored on the previous
//! capture like the pump's `SnapshotAnchor`), the connection side
//! materializes its state (`SharedRequestSnapshot::into_state`) and
//! dispatches the tool (`mcp_serve::process_message_with_write_applier`).
//!
//! The fixture follows #412 (14 pages × 100 boards × 46 rows, ~195k nodes)
//! plus what a real file has: a component page whose master every board
//! instantiates, and an image fill per board. `DE_MEM_PAGES`,
//! `DE_MEM_BOARDS`, `DE_MEM_ROWS` and `DE_MEM_CALLS` shrink or grow it.

mod support;

use de_editor_core::history_snapshot::SharedDoc;
use de_editor_core::request_snapshot::shared_snapshot_pages;
use de_editor_core::request_snapshot_pages::{kept_pages, PageNeeds};
use de_editor_core::EditorState;
use std::time::{Duration, Instant};
use support::counting_alloc::{self, Counting};
use support::large_doc::{self, Fixture};

#[global_allocator]
static ALLOC: Counting = Counting;

/// One measured call: the tool and its JSON arguments.
struct Call {
    tool: &'static str,
    args: String,
}

fn read_calls(fixture: &Fixture) -> Vec<Call> {
    let board = fixture.board_id(0, 0);
    let row = fixture.row_id(0, 0, 0);
    let other_page = fixture.page_id(fixture.pages - 1);
    let calls: [(&'static str, String); 11] = [
        ("get_history_depth", "{}".into()),
        ("get_document_info", "{}".into()),
        ("get_node", format!(r#"{{"node_id":"{row}"}}"#)),
        (
            "batch_get",
            format!(r#"{{"nodeIds":["{board}"],"readDepth":2}}"#),
        ),
        (
            "read_nodes",
            format!(r#"{{"nodeIds":["{board}"],"depth":1}}"#),
        ),
        ("snapshot_layout", r#"{"maxDepth":1}"#.into()),
        (
            "snapshot_layout",
            format!(r#"{{"maxDepth":1,"pageId":"{other_page}"}}"#),
        ),
        ("find_nodes", r#"{"query":"Board 7","limit":20}"#.into()),
        ("get_variables", "{}".into()),
        ("get_screenshot", format!(r#"{{"nodeId":"{board}"}}"#)),
        (
            "execute",
            format!(
                r#"{{"script":"const n = Get(\"{board}\", {{depth: 1}}); Print(n.name, n.children.length);"}}"#
            ),
        ),
    ];
    calls
        .into_iter()
        .map(|(tool, args)| Call { tool, args })
        .collect()
}

fn write_call(fixture: &Fixture, i: usize) -> Call {
    let row = fixture.row_id(0, i % fixture.boards, 0);
    Call {
        tool: "update_node",
        args: format!(r#"{{"nodeId":"{row}","data":{{"name":"Row edited {i}"}}}}"#),
    }
}

fn request_line(id: usize, call: &Call) -> String {
    format!(
        r#"{{"jsonrpc":"2.0","id":{id},"method":"tools/call","params":{{"name":"{}","arguments":{}}}}}"#,
        call.tool, call.args
    )
}

/// What one live call cost.
struct Sample {
    took: Duration,
    /// Heap above the pre-call level at the call's peak.
    peak_extra: usize,
    /// Heap above the pre-call level once the call returned (the pump's
    /// anchor and anything else the call left behind).
    kept: isize,
    ok: bool,
}

/// One live call: UI-side capture, connection-side copy, dispatch. A write
/// is applied to the live state too, as the UI thread would.
/// The pump's side of live snapshots: the sharing anchor and the page
/// needs it keeps per document revision.
#[derive(Default)]
struct Anchor {
    doc: Option<SharedDoc>,
    needs: Option<PageNeeds>,
}

fn live_call(live: &mut EditorState, anchor: &mut Anchor, id: usize, call: &Call) -> Sample {
    let line = request_line(id, call);
    let before = counting_alloc::reset_peak();
    let started = Instant::now();
    let pages = de_mcp::snapshot_pages::for_request(&line);
    let keep = if pages == de_mcp::snapshot_pages::SnapshotPages::All {
        None
    } else {
        if !anchor.needs.as_ref().is_some_and(|n| n.is_current(live)) {
            anchor.needs = Some(PageNeeds::of(live));
        }
        anchor
            .needs
            .as_ref()
            .and_then(|needs| kept_pages(live, &pages, needs))
    };
    let snapshot = shared_snapshot_pages(live, anchor.doc.as_ref(), keep.as_deref());
    anchor.doc = Some(match (&keep, anchor.doc.as_ref()) {
        (Some(keep), Some(doc)) => doc.merged_pages(snapshot.shared_doc(), keep),
        _ => snapshot.shared_doc().clone(),
    });
    let mut state = snapshot.into_state();
    let response = de_host_services::mcp_serve::process_message_with_write_applier(
        &mut state,
        &line,
        |_tool, local, write| {
            let applied = live.apply(write.command.clone());
            if applied {
                let _ = local.apply(write.command.clone());
            }
            applied
        },
    )
    .ok()
    .flatten()
    .unwrap_or_default();
    drop(state);
    let took = started.elapsed();
    let peak_extra = counting_alloc::peak().saturating_sub(before);
    let kept = counting_alloc::live() as isize - before as isize;
    Sample {
        took,
        peak_extra,
        kept,
        ok: !response.contains("\"isError\":true") && !response.contains("\"error\""),
    }
}

fn mb(bytes: f64) -> f64 {
    bytes / (1024.0 * 1024.0)
}

fn median(mut values: Vec<f64>) -> f64 {
    values.sort_by(f64::total_cmp);
    values.get(values.len() / 2).copied().unwrap_or(0.0)
}

#[test]
#[ignore = "measurement; run by hand with --release --nocapture --test-threads=1"]
fn live_mcp_call_memory_and_latency() {
    let fixture = Fixture::from_env();
    let calls_per_tool = large_doc::env_usize("DE_MEM_CALLS", 5);
    let heap_start = counting_alloc::live();
    let mut live = fixture.build_state();
    let doc_heap = counting_alloc::live().saturating_sub(heap_start);
    println!(
        "document: {} nodes on {} pages; editor heap {:.0} MB ({} B per node, PenNode inline {} B); process {}",
        fixture.node_count(),
        fixture.pages,
        mb(doc_heap as f64),
        doc_heap / fixture.node_count().max(1),
        std::mem::size_of::<jian_ops_schema::node::PenNode>(),
        footprint_mb()
    );

    // What every call adds before its tool runs: the request state moved
    // into a shared handle and back (`SharedState::take_from`).
    let started = Instant::now();
    for _ in 0..100 {
        de_editor_core::shared_state::SharedState::take_from(&mut live).restore_into(&mut live);
    }
    println!(
        "shared handle take + restore: {:.1} µs",
        started.elapsed().as_secs_f64() * 1e6 / 100.0
    );

    let mut anchor = Anchor::default();
    // The first capture has no anchor and no history to share with.
    let first = live_call(
        &mut live,
        &mut anchor,
        0,
        &Call {
            tool: "get_history_depth",
            args: "{}".into(),
        },
    );
    println!(
        "first call (no anchor): {:.1} ms, peak +{:.0} MB, kept {:+.0} MB",
        first.took.as_secs_f64() * 1e3,
        mb(first.peak_extra as f64),
        mb(first.kept as f64)
    );

    println!(
        "{:<34} {:>9} {:>11} {:>9} {:>8}",
        "tool", "median ms", "peak +MB", "% of doc", "kept MB"
    );
    let mut id = 1;
    for call in read_calls(&fixture) {
        let mut times = Vec::new();
        let mut peaks = Vec::new();
        let mut kept = 0isize;
        let mut ok = true;
        for _ in 0..calls_per_tool {
            let sample = live_call(&mut live, &mut anchor, id, &call);
            id += 1;
            times.push(sample.took.as_secs_f64() * 1e3);
            peaks.push(sample.peak_extra as f64);
            kept = kept.max(sample.kept);
            ok &= sample.ok;
        }
        let label = format!("{} {}", call.tool, short_args(&call.args));
        let peak = median(peaks);
        println!(
            "{:<34} {:>9.1} {:>11.0} {:>8.1}% {:>8.1}{}",
            label,
            median(times),
            mb(peak),
            100.0 * peak / doc_heap.max(1) as f64,
            mb(kept as f64),
            if ok { "" } else { "  (tool error)" }
        );
    }
    let mut times = Vec::new();
    let mut peaks = Vec::new();
    for i in 0..calls_per_tool {
        let sample = live_call(&mut live, &mut anchor, id, &write_call(&fixture, i));
        id += 1;
        assert!(sample.ok, "update_node applies");
        times.push(sample.took.as_secs_f64() * 1e3);
        peaks.push(sample.peak_extra as f64);
    }
    println!(
        "{:<34} {:>9.1} {:>11.0} {:>8.1}%",
        "update_node (write)",
        median(times),
        mb(median(peaks.clone())),
        100.0 * median(peaks) / doc_heap.max(1) as f64
    );
    println!(
        "editor heap at the end {:.0} MB (history {} steps); process {}",
        mb(counting_alloc::live().saturating_sub(heap_start) as f64),
        live.history.past.len(),
        footprint_mb()
    );
}

/// Undo history growth: one undoable edit per step (a person's rename:
/// history snapshot, then the change), with the heap counted every 20
/// steps, then undo to the start.
#[test]
#[ignore = "measurement; run by hand with --release --nocapture --test-threads=1"]
fn undo_history_memory() {
    let fixture = Fixture::from_env();
    let heap_start = counting_alloc::live();
    let mut live = fixture.build_state();
    let doc_heap = counting_alloc::live().saturating_sub(heap_start);
    println!(
        "document: {} nodes; editor heap {:.0} MB",
        fixture.node_count(),
        mb(doc_heap as f64)
    );
    let base = counting_alloc::live();
    let steps = de_editor_core::history::HISTORY_CAP + 20;
    let started = Instant::now();
    for i in 0..steps {
        let row = fixture.row_id(0, i % fixture.boards, 0);
        live.commit_history();
        let renamed = live.apply(de_editor_core::EditorCommand::SetNodeName {
            node_id: de_editor_core::NodeId::new(row),
            name: format!("Row edited {i}"),
        });
        assert!(renamed, "edit {i} applies");
        if i == 0 || (i + 1) % 20 == 0 {
            let usage = de_editor_core::memory_report::history_usage(&live);
            println!(
                "after {:>3} edits: history {:>3} steps, heap +{:.0} MB over the document (estimate {:.0} MB), {:.2} ms per edit",
                i + 1,
                live.history.past.len(),
                mb(counting_alloc::live().saturating_sub(base) as f64),
                mb(usage.bytes as f64),
                started.elapsed().as_secs_f64() * 1e3 / (i + 1) as f64
            );
        }
    }
    let undo_started = Instant::now();
    let mut undone = 0;
    while live.undo() {
        undone += 1;
    }
    println!(
        "undo all {undone}: {:.1} ms; heap +{:.0} MB",
        undo_started.elapsed().as_secs_f64() * 1e3,
        mb(counting_alloc::live().saturating_sub(base) as f64)
    );
}

fn footprint_mb() -> String {
    let memory = de_host_services::process_memory::current();
    match (memory.resident, memory.footprint) {
        (Some(r), Some(f)) => format!(
            "RSS {:.0} MB, footprint {:.0} MB",
            mb(r as f64),
            mb(f as f64)
        ),
        _ => "n/a".into(),
    }
}

fn short_args(args: &str) -> String {
    if args.contains("pageId") {
        "(other page)".into()
    } else {
        String::new()
    }
}
