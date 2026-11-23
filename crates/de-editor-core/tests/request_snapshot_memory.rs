//! What one MCP request snapshot costs the UI thread (#352).
//!
//! The live MCP server used to deep-copy the document on the UI thread for
//! every tool call (`narrowed_snapshot`): ~520 MB and ~150 ms on a
//! 184k-node document. `shared_snapshot` shares unchanged top-level nodes
//! with the previous capture, so a repeated call allocates a small fraction
//! of one document copy; the copy moves to the consumer
//! (`SharedRequestSnapshot::into_state`).
//!
//! A counting global allocator records the peak allocation, so this is an
//! integration test: the allocator applies to the whole test binary.
//! `DE_SNAPSHOT_MEMORY_NODES` sets a larger document (the issue's size:
//! `184000`); run with `--release --nocapture` to read the numbers.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;

use de_editor_core::request_snapshot::{narrowed_snapshot, shared_snapshot};
use de_editor_core::{EditorCommand, EditorState, NodeId};

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
const DEFAULT_NODES: usize = 12_000;

/// `PAGES` pages of `FRAMES` boards of text + rectangle pairs, and 100 undo
/// steps, like the #350 probe.
fn document(total_nodes: usize) -> EditorState {
    let per_frame = (total_nodes / (PAGES * FRAMES * 2)).max(1);
    let mut pages = Vec::new();
    for p in 0..PAGES {
        let mut frames = Vec::new();
        for f in 0..FRAMES {
            let mut leaves = Vec::new();
            for l in 0..per_frame {
                leaves.push(format!(
                    r#"{{"type":"text","id":"t{p}-{f}-{l}","content":"Label {l}","fontSize":14}}"#
                ));
                leaves.push(format!(
                    r#"{{"type":"rectangle","id":"r{p}-{f}-{l}","width":40,"height":8}}"#
                ));
            }
            frames.push(format!(
                r#"{{"type":"frame","id":"f{p}-{f}","x":{x},"y":0,"width":400,"layout":"vertical","children":[{}]}}"#,
                leaves.join(","),
                x = f * 500
            ));
        }
        pages.push(format!(
            r#"{{"id":"p{p}","name":"Page {p}","children":[{}]}}"#,
            frames.join(",")
        ));
    }
    let doc = format!(
        r#"{{"version":"1.0.0","pages":[{}],"children":[]}}"#,
        pages.join(",")
    );
    let mut state = EditorState::from_document(
        jian_ops_schema::load_str(&doc)
            .expect("synthetic document")
            .value,
    );
    for i in 0..100 {
        state.commit_history();
        assert!(resize(&mut state, &format!("r0-{}-0", i % FRAMES), i));
    }
    state
}

fn resize(state: &mut EditorState, id: &str, i: usize) -> bool {
    state.apply(EditorCommand::UpdateNode {
        node_id: NodeId::new(id.to_string()),
        x: None,
        y: None,
        width: Some(40 + (i as i32 % 7) + 1),
        height: None,
        name: None,
        fill_hex: None,
        page_id: None,
    })
}

#[test]
fn a_repeated_request_snapshot_does_not_copy_the_document() {
    let nodes = std::env::var("DE_SNAPSHOT_MEMORY_NODES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_NODES);
    let mut state = document(nodes);

    let started = Instant::now();
    let (full, full_bytes) = peak_of(|| narrowed_snapshot(&mut state));
    let full_time = started.elapsed();
    drop(full);

    // First capture: anchored on the newest undo snapshot.
    let (first, _) = peak_of(|| shared_snapshot(&mut state, None));
    // A read-only call: nothing changed since the last capture.
    let started = Instant::now();
    let (second, shared_bytes) = peak_of(|| shared_snapshot(&mut state, Some(first.shared_doc())));
    let shared_time = started.elapsed();
    println!(
        "{nodes} nodes: deep copy {} MB in {full_time:?}; shared {} KB in {shared_time:?}",
        full_bytes / 1_000_000,
        shared_bytes / 1_000,
    );
    assert!(
        shared_bytes * 20 < full_bytes,
        "a repeated snapshot allocated {shared_bytes} bytes, a full copy is {full_bytes}"
    );
    let owned = second.clone().into_state();
    assert_eq!(owned.doc, state.doc, "the consumer gets the live document");
    assert_eq!(owned.revision, state.revision);

    // An edit between calls: only the edited board is copied again.
    assert!(resize(&mut state, "r0-5-0", 3));
    let third = shared_snapshot(&mut state, Some(second.shared_doc()));
    let before = second.shared_doc().page_children(0).expect("page 0");
    let after = third.shared_doc().page_children(0).expect("page 0");
    for (i, (b, a)) in before.iter().zip(after).enumerate() {
        assert_eq!(Arc::ptr_eq(b, a), i != 5, "board {i} sharing");
    }
    assert_eq!(third.into_state().doc, state.doc, "the edit is visible");
}
