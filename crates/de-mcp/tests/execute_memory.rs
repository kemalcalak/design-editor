//! How much memory one read-only `execute` allocates (#350).
//!
//! A counting global allocator records the peak
//! allocation of a call. On a 184k-node document every copy of the
//! document is about 520 MB, and a read-only call used to make three of
//! them plus one more per `Get`. The bound is a ratio to one document
//! copy, so it holds for any document size: the call may copy the snapshot
//! once (the tool registry's copy), never once per read.
//!
//! An integration test rather than an in-crate bench like
//! `script_node_bench_tests.rs`: the global allocator applies to its whole
//! test binary, and here that binary holds nothing else.
//!
//! The default document is small enough for CI. `DE_EXECUTE_MEMORY_NODES`
//! sets a larger one (the issue's size: `184000`) and the test prints the
//! numbers; run with `--release --nocapture` to read them.
#![cfg(feature = "script")]

use std::alloc::{GlobalAlloc, Layout, System};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use de_editor_core::{EditorCommand, EditorState, NodeId};
use de_mcp::{McpTool, ToolOutcome};

struct Counting;

static CURRENT: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

// SAFETY: forwards to `System` and only adds bookkeeping.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = System.alloc(layout);
        if !ptr.is_null() {
            let now = CURRENT.fetch_add(layout.size(), Ordering::Relaxed) + layout.size();
            PEAK.fetch_max(now, Ordering::Relaxed);
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout);
        CURRENT.fetch_sub(layout.size(), Ordering::Relaxed);
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

/// Peak bytes allocated on top of what was live when `f` started.
fn peak_of<T>(f: impl FnOnce() -> T) -> (T, usize) {
    let base = CURRENT.load(Ordering::Relaxed);
    PEAK.store(base, Ordering::Relaxed);
    let out = f();
    (out, PEAK.load(Ordering::Relaxed).saturating_sub(base))
}

const PAGES: usize = 3;
const FRAMES: usize = 12;
const GROUPS: usize = 10;
const DEFAULT_NODES: usize = 12_000;

/// `PAGES` pages of `FRAMES` auto-layout boards, each `GROUPS` columns of
/// text + rectangle pairs bound to colour variables, and 100 undo steps.
fn document(total_nodes: usize) -> EditorState {
    let per_group = (total_nodes / (PAGES * FRAMES * GROUPS * 2)).max(1);
    let mut pages = Vec::new();
    for p in 0..PAGES {
        let mut frames = Vec::new();
        for f in 0..FRAMES {
            let mut groups = Vec::new();
            for g in 0..GROUPS {
                let mut leaves = Vec::new();
                for l in 0..per_group {
                    leaves.push(format!(
                        r#"{{"type":"text","id":"t{p}-{f}-{g}-{l}","content":"Label {l}","fontSize":14,"fill":[{{"type":"solid","color":"$fg"}}]}}"#
                    ));
                    leaves.push(format!(
                        r#"{{"type":"rectangle","id":"r{p}-{f}-{g}-{l}","width":40,"height":8,"fill":[{{"type":"solid","color":"$accent"}}]}}"#
                    ));
                }
                groups.push(format!(
                    r#"{{"type":"frame","id":"g{p}-{f}-{g}","layout":"vertical","gap":4,"width":"fill_container","children":[{}]}}"#,
                    leaves.join(",")
                ));
            }
            frames.push(format!(
                r#"{{"type":"frame","id":"f{p}-{f}","name":"Frame {f}","x":{x},"y":0,"width":400,"layout":"vertical","children":[{}]}}"#,
                groups.join(","),
                x = f * 500
            ));
        }
        pages.push(format!(
            r#"{{"id":"p{p}","name":"Page {p}","children":[{}]}}"#,
            frames.join(",")
        ));
    }
    let doc = format!(
        r##"{{"version":"1.0.0","variables":{{"fg":{{"type":"color","value":"#111111"}},"accent":{{"type":"color","value":"#3b82f6"}}}},"pages":[{}],"children":[]}}"##,
        pages.join(",")
    );
    let mut state = EditorState::from_document(
        jian_ops_schema::load_str(&doc)
            .expect("synthetic document")
            .value,
    );
    for i in 0..100 {
        state.commit_history();
        assert!(state.apply(EditorCommand::UpdateNode {
            node_id: NodeId::new(format!("r0-{}-0-0", i % FRAMES)),
            x: None,
            y: None,
            width: Some(40 + (i as i32 % 7)),
            height: None,
            name: None,
            fill_hex: None,
            page_id: None,
        }));
    }
    state
}

/// The issue's audit: one board per call, visited, plus the variables.
/// `extra_gets` repeats single-node reads, which used to copy the document
/// each.
fn audit_script(page: usize, frame: usize, extra_gets: usize) -> String {
    format!(
        r#"
let n = 0;
Get("f{page}-{frame}", (node, c) => {{ n++; }});
for (let i = 0; i < {extra_gets}; i++) Get("g{page}-{frame}-0");
GetVariables();
Print("visited", n);
"#
    )
}

fn run_read_only(state: &EditorState, page: usize, frame: usize, extra_gets: usize) -> usize {
    let mut args = BTreeMap::new();
    args.insert("script".to_string(), audit_script(page, frame, extra_gets));
    args.insert("pageId".to_string(), format!("p{page}"));
    let (outcome, peak) = peak_of(|| de_mcp::script_execute::execute_snapshot(state).call(&args));
    let ToolOutcome::OkJsonWithImages { json, command, .. } = outcome else {
        panic!("execute failed: {outcome:?}");
    };
    assert!(command.is_none(), "a read-only script ships no command");
    assert!(json.contains(r#""visited "#), "{json}");
    peak
}

#[test]
fn a_read_only_execute_copies_the_document_at_most_once() {
    let nodes = std::env::var("DE_EXECUTE_MEMORY_NODES")
        .ok()
        .and_then(|raw| raw.parse().ok())
        .unwrap_or(DEFAULT_NODES);
    let state = document(nodes);
    let (copy, doc_bytes) = peak_of(|| state.doc.clone());
    drop(copy);
    let mb = |bytes: usize| bytes as f64 / 1_048_576.0;
    println!(
        "{} nodes, one document copy {:.1} MB",
        state.collect_node_ids().len(),
        mb(doc_bytes)
    );

    let mut worst = 0;
    for (label, page, extra_gets) in [
        ("active page", 0, 0),
        ("other page", 2, 0),
        ("other page, 20 more Get", 2, 20),
    ] {
        let started = Instant::now();
        let peak = run_read_only(&state, page, 3, extra_gets);
        println!(
            "execute ({label}): {:?}, peak +{:.1} MB ({:.2} document copies)",
            started.elapsed(),
            mb(peak),
            peak as f64 / doc_bytes as f64
        );
        worst = worst.max(peak);
    }
    // Before #350: ~3 copies for one `Get`, one more for every further
    // `Get`. Now: the registry's copy moved into the run, plus one board's
    // layout and serialized reads.
    assert!(
        (worst as f64) < 1.5 * doc_bytes as f64,
        "a read-only execute peaked at {:.1} MB, more than 1.5 document copies ({:.1} MB each)",
        mb(worst),
        mb(doc_bytes)
    );

    // The report was about calls piling up: an audit that runs one call per
    // board. Run 25 in a row over every page and board, a few `Get`s each,
    // and check nothing is left behind between calls.
    const CALLS: usize = 25;
    let held_before = CURRENT.load(Ordering::Relaxed);
    let started = Instant::now();
    let mut worst_in_run = 0;
    for call in 0..CALLS {
        let peak = run_read_only(&state, call % PAGES, call % FRAMES, 5);
        worst_in_run = worst_in_run.max(peak);
    }
    let per_call = started.elapsed() / CALLS as u32;
    let held = CURRENT.load(Ordering::Relaxed).saturating_sub(held_before);
    println!(
        "{CALLS} calls in a row: {per_call:?} per call, worst peak +{:.1} MB, held after +{:.1} MB",
        mb(worst_in_run),
        mb(held)
    );
    assert!(
        (worst_in_run as f64) < 1.5 * doc_bytes as f64,
        "a call in the run peaked at {:.1} MB",
        mb(worst_in_run)
    );
    assert!(
        (held as f64) < 0.05 * doc_bytes as f64,
        "{CALLS} read-only calls left {:.1} MB allocated",
        mb(held)
    );
}
