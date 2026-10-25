//! What script nodes cost the editor (#19). No `criterion`: each case
//! times a loop with `Instant`, prints the number (run with
//! `--nocapture` to read it) and asserts a bound with a wide margin, so
//! a regression of an order of magnitude fails while timing noise does
//! not.
//!
//!   1. the scan the host runs when the revision moves, on a 10k-node
//!      document with 20 current script nodes (the per-frame cost of a
//!      drag in such a document);
//!   2. the same scan with 50 KB sources (a chart library pasted in);
//!   3. one run of a script returning 2000 nodes, the output cap;
//!   4. landing those 2000 nodes into the 10k-node document, and the
//!      layout scene rebuild the landing causes.

use super::*;
use de_editor_core::script_regen::no_layout;
use de_editor_core::NodeId;
use std::time::Instant;

const BARS: &str = "// @input n: number = 2000\n\
    const out = [];\n\
    for (let i = 0; i < pencil.input.n; i++) {\n\
      out.push({ type: 'rectangle', x: (i % 50) * 4, y: Math.floor(i / 50) * 4, \
                 width: 3, height: 3, fill: '#3b82f6' });\n\
    }\n\
    return out;";

/// About 50 KB of comment padding, as a pasted helper library would add.
fn padded(source: &str) -> String {
    let filler = "// helper: keeps the source realistically large\n".repeat(1000);
    format!("{source}\n{filler}")
}

fn big_document() -> EditorState {
    document_with(BARS)
}

fn document_with(source: &str) -> EditorState {
    let mut roots = Vec::new();
    for f in 0..100 {
        let rects: Vec<String> = (0..100)
            .map(|r| {
                format!(
                    r#"{{"type":"rectangle","id":"r{f}-{r}","x":{r},"y":0,"width":4,"height":4}}"#
                )
            })
            .collect();
        roots.push(format!(
            r#"{{"type":"frame","id":"f{f}","x":{x},"y":0,"width":400,"height":400,"children":[{}]}}"#,
            rects.join(","),
            x = f * 500
        ));
    }
    for s in 0..20 {
        roots.push(format!(
            r#"{{"type":"script","id":"s{s}","x":{x},"y":600,"width":200,"height":200,"script":{source}}}"#,
            x = s * 300,
            source = serde_json::to_string(source).unwrap()
        ));
    }
    let doc = format!(r#"{{"version":"1.0.0","children":[{}]}}"#, roots.join(","));
    EditorState::from_document(jian_ops_schema::load_str(&doc).unwrap().value)
}

/// Mark every script current, as a saved document opens.
fn settle(state: &mut EditorState) {
    for job in state.take_script_jobs(&no_layout) {
        state.land_script_output(
            &job.node_id,
            &job.hash,
            ScriptOutcome::Generated(vec![]),
            0,
            &no_layout,
        );
    }
    assert!(state.take_script_jobs(&no_layout).is_empty());
}

#[test]
fn bench_scan_on_a_revision_change() {
    let mut state = big_document();
    settle(&mut state);
    const ROUNDS: u32 = 50;
    let started = Instant::now();
    for _ in 0..ROUNDS {
        state.mark_document_changed();
        assert!(state.take_script_jobs(&no_layout).is_empty());
    }
    let per_scan = started.elapsed() / ROUNDS;
    eprintln!("script scan, 10k nodes + 20 scripts: {per_scan:?} per revision change");
    assert!(per_scan.as_millis() < 50, "scan took {per_scan:?}");
    // No revision change: no walk at all.
    let started = Instant::now();
    for _ in 0..ROUNDS {
        state.take_script_jobs(&no_layout);
    }
    eprintln!(
        "script scan, unchanged revision: {:?} per frame",
        started.elapsed() / ROUNDS
    );
}

#[test]
fn bench_scan_with_large_sources() {
    let mut state = document_with(&padded(BARS));
    settle(&mut state);
    const ROUNDS: u32 = 20;
    let started = Instant::now();
    for _ in 0..ROUNDS {
        state.mark_document_changed();
        assert!(state.take_script_jobs(&no_layout).is_empty());
    }
    let per_scan = started.elapsed() / ROUNDS;
    eprintln!("script scan, 10k nodes + 20 scripts of 50 KB: {per_scan:?} per revision change");
    assert!(per_scan.as_millis() < 200, "scan took {per_scan:?}");
}

#[test]
fn bench_run_and_land_the_output_cap() {
    let mut state = big_document();
    let jobs = state.take_script_jobs(&no_layout);
    let job = jobs
        .into_iter()
        .find(|j| j.node_id == NodeId::new("s0"))
        .unwrap();
    let run = run_script_job(&job);
    eprintln!("script run, 2000 nodes: {:?}", run.elapsed);
    assert!(run.elapsed < TIME_LIMIT);
    let outcome = run.outcome();
    let started = Instant::now();
    assert!(state.land_script_output(&job.node_id, &job.hash, outcome, 0, &no_layout));
    let landed = started.elapsed();
    eprintln!("script land, 2000 nodes into 10k: {landed:?}");
    assert!(landed.as_millis() < 500, "landing took {landed:?}");
    let started = Instant::now();
    let scene = de_pen_loader::editor_state_to_layout_scene(&state);
    eprintln!(
        "layout scene rebuild after the landing, 12k nodes: {:?}",
        started.elapsed()
    );
    assert!(!scene.pages.is_empty());
}
