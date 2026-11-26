//! Large multi-page document: frame time, page switch time, process memory
//! and per-cache bytes over a scripted session (#456).
//!
//! Measurement, not a gate; compare the printed numbers before and after a
//! change on the same machine:
//!
//! `cargo test --release -p de-host-native --features gl-host --lib large_doc_session -- --ignored --nocapture --test-threads=1`
//!
//! The document is #412's shape (14 pages × 100 boards × 46 rows, ~195k
//! nodes; `DE_MEM_PAGES` / `DE_MEM_BOARDS` / `DE_MEM_ROWS` change it) with
//! an image fill per board. One script pass pans, zooms, edits and undoes
//! on page 0, then visits every page and comes back. `DE_MEM_BENCH_SECS`
//! repeats the pass for that long (an hour: 3600), sampling memory every
//! pass, to show whether anything grows without bound.

use super::WidgetHostNative;
use crate::backend::{NativeBackend, NativeFrameBackend};
use std::fmt::Write as _;
use std::time::{Duration, Instant};

const W: i32 = 1440;
const H: i32 = 900;

/// A 1 × 1 PNG, so image fills need no file.
const PNG_DATA_URL: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";

fn env_usize(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn document_json(pages: usize, boards: usize, rows: usize) -> String {
    let mut out = String::from(r#"{"version":"1.0","children":[],"pages":["#);
    for page in 0..pages {
        if page > 0 {
            out.push(',');
        }
        write!(
            out,
            r#"{{"id":"page-{page}","name":"Page {page}","children":["#
        )
        .unwrap();
        for board in 0..boards {
            if board > 0 {
                out.push(',');
            }
            let id = format!("p{page}b{board}");
            write!(
                out,
                r##"{{"type":"frame","id":"{id}","name":"Board {board}","x":{x},"y":{y},"width":400,"height":2300,"layout":"vertical","fill":[{{"type":"solid","color":"#FFFFFF"}}],"children":[{{"type":"rectangle","id":"{id}-image","width":360,"height":120,"fill":[{{"type":"image","url":"{PNG_DATA_URL}","mode":"fill"}}]}}"##,
                x = (board % 10) * 420,
                y = (board / 10) * 2400
            )
            .unwrap();
            for row in 0..rows {
                write!(
                    out,
                    r##",{{"type":"frame","id":"{id}r{row}","name":"Row","width":360,"height":40,"layout":"horizontal","gap":8,"children":[{{"type":"rectangle","id":"{id}r{row}-icon","width":24,"height":24,"cornerRadius":6,"fill":[{{"type":"solid","color":"#3355FF"}}]}},{{"type":"text","id":"{id}r{row}-label","content":"Row {row} of board {board}","fontSize":14}}]}}"##
                )
                .unwrap();
            }
            out.push_str("]}");
        }
        out.push_str("]}");
    }
    out.push_str("]}");
    out
}

fn session_host(pages: usize, boards: usize, rows: usize) -> WidgetHostNative {
    let json = document_json(pages, boards, rows);
    let doc = jian_ops_schema::load_str(&json)
        .expect("fixture JSON parses")
        .value;
    drop(json);
    let mut state = de_editor_core::EditorState::from_opened_document(doc);
    state.chat.minimize();
    state.editor_ui.sidebar_open = false;
    state.viewport.zoom = 0.5;
    state.viewport.pan_x = 40.0;
    state.viewport.pan_y = 40.0;
    let mut host = WidgetHostNative::new();
    *host.editor_state_mut() = state;
    host.mark_paint_dirty_for_test();
    host
}

struct Painter {
    backend: NativeBackend,
    surface: skia_safe::Surface,
    now_ms: u64,
}

impl Painter {
    fn paint(&mut self, host: &mut WidgetHostNative) -> f64 {
        self.now_ms += 16;
        host.set_now_ms(self.now_ms);
        let started = Instant::now();
        self.surface.canvas().clear(skia_safe::Color::WHITE);
        let mut frame = NativeFrameBackend::new(&mut self.backend, self.surface.canvas());
        host.paint(&mut frame, W as f32, H as f32);
        started.elapsed().as_secs_f64() * 1e3
    }
}

/// Median and 95th percentile.
fn stats(mut ms: Vec<f64>) -> (f64, f64) {
    ms.sort_by(f64::total_cmp);
    let at = |q: f64| ms[((ms.len() - 1) as f64 * q).round() as usize];
    (at(0.5), at(0.95))
}

fn footprint_mb() -> f64 {
    use sysinfo::{Pid, ProcessRefreshKind, System};
    let mut system = System::new();
    let pid = Pid::from_u32(std::process::id());
    system.refresh_process_specifics(pid, ProcessRefreshKind::new().with_memory());
    system
        .process(pid)
        .map_or(0.0, |process| process.memory() as f64 / (1024.0 * 1024.0))
}

/// One script pass; returns the phase timings in ms.
struct Pass {
    pan: Vec<f64>,
    zoom: Vec<f64>,
    edit: Vec<f64>,
    undo: Vec<f64>,
    page_switch: Vec<f64>,
    page_return: f64,
}

fn script_pass(host: &mut WidgetHostNative, painter: &mut Painter, pages: usize) -> Pass {
    let (cx, cy) = (W as f32 / 2.0, H as f32 / 2.0);
    let pan = (0..60)
        .map(|i| {
            let dx = if (i / 15) % 2 == 0 { -24.0 } else { 24.0 };
            host.apply_pan_gesture(cx, cy, dx, -8.0, W as f32, H as f32);
            painter.paint(host)
        })
        .collect();
    let zoom = (0..30)
        .map(|i| {
            let dy = if i < 15 { -4.0 } else { 4.0 };
            host.apply_pinch_gesture(cx, cy, dy, W as f32, H as f32);
            painter.paint(host)
        })
        .collect();
    host.editor_state_mut()
        .apply(de_editor_core::EditorCommand::SetSelection {
            node_id: de_editor_core::NodeId::new("p0b0"),
        });
    host.mark_editor_state_dirty();
    painter.paint(host);
    let edit = (0..20)
        .map(|_| {
            host.apply_nudge(1.0, 0.0);
            painter.paint(host)
        })
        .collect();
    let undo = (0..20)
        .map(|_| {
            host.editor_state_mut().undo();
            host.mark_paint_dirty_for_test();
            painter.paint(host)
        })
        .collect();
    let page_switch = (1..pages)
        .map(|page| {
            let started = Instant::now();
            host.editor_state_mut().set_active_page(page);
            host.mark_paint_dirty_for_test();
            painter.paint(host);
            started.elapsed().as_secs_f64() * 1e3
        })
        .collect();
    let started = Instant::now();
    host.editor_state_mut().set_active_page(0);
    host.mark_paint_dirty_for_test();
    painter.paint(host);
    let page_return = started.elapsed().as_secs_f64() * 1e3;
    Pass {
        pan,
        zoom,
        edit,
        undo,
        page_switch,
        page_return,
    }
}

fn print_report(host: &WidgetHostNative, painter: &Painter) {
    let report = host.memory_report(&painter.backend);
    let mut line = String::new();
    for cache in report.largest(8) {
        write!(
            line,
            " {}={:.1}MB/{}",
            cache.id,
            cache.bytes as f64 / (1024.0 * 1024.0),
            cache.entries
        )
        .unwrap();
    }
    println!("  caches:{line}");
}

#[test]
#[ignore = "benchmark, run by hand"]
fn large_doc_session_benchmark() {
    let pages = env_usize("DE_MEM_PAGES", 14);
    let boards = env_usize("DE_MEM_BOARDS", 100);
    let rows = env_usize("DE_MEM_ROWS", 46);
    let seconds = env_usize("DE_MEM_BENCH_SECS", 0);
    let rss_start = footprint_mb();
    let started = Instant::now();
    let mut host = session_host(pages, boards, rows);
    let mut painter = Painter {
        backend: NativeBackend::with_dpi(2.0),
        surface: skia_safe::surfaces::raster_n32_premul((W, H)).expect("surface"),
        now_ms: 1_000,
    };
    let load_ms = started.elapsed().as_secs_f64() * 1e3;
    let first = painter.paint(&mut host);
    println!(
        "BENCH session: {pages}×{boards}×{rows} ({} nodes), load {load_ms:.0} ms, first paint {first:.0} ms, RSS {rss_start:.0} → {:.0} MB",
        pages * boards * (2 + rows * 3),
        footprint_mb()
    );
    print_report(&host, &painter);

    let deadline = Instant::now() + Duration::from_secs(seconds as u64);
    let mut pass_no = 0;
    loop {
        pass_no += 1;
        let pass = script_pass(&mut host, &mut painter, pages);
        let (pan, pan95) = stats(pass.pan);
        let (zoom, zoom95) = stats(pass.zoom);
        let (edit, edit95) = stats(pass.edit);
        let (undo, undo95) = stats(pass.undo);
        let (switch, switch95) = stats(pass.page_switch);
        println!(
            "BENCH pass {pass_no} at {:.0} s: pan {pan:.2}/{pan95:.2} ms, zoom {zoom:.2}/{zoom95:.2} ms, edit {edit:.2}/{edit95:.2} ms, undo {undo:.2}/{undo95:.2} ms, page switch {switch:.0}/{switch95:.0} ms, return {:.0} ms (median/p95); RSS {:.0} MB",
            started.elapsed().as_secs_f64(),
            pass.page_return,
            footprint_mb()
        );
        print_report(&host, &painter);
        if Instant::now() >= deadline {
            break;
        }
    }
}
