//! A minimized window reports a 0×0 client area (#302). Hosts are meant
//! to skip painting then, but a stray frame at a degenerate viewport must
//! still return: painting, hit-testing and the preview resize hook all
//! have to terminate, and the next real-size frame must paint normally.

use super::WidgetHostNative;
use crate::backend::{NativeBackend, NativeFrameBackend};
use std::sync::mpsc;
use std::time::Duration;

const W: f32 = 1280.0;
const H: f32 = 800.0;

/// Viewports a minimize, a restore or a squeezed drag-resize can report.
const DEGENERATE: [(f32, f32); 6] = [
    (0.0, 0.0),
    (1.0, 1.0),
    (0.0, H),
    (W, 0.0),
    (40.0, 30.0),
    (-1.0, -1.0),
];

fn seed_document(host: &mut WidgetHostNative) {
    let json = r##"{"version":"1.0.0","children":[
        {"type":"frame","id":"f1","x":0,"y":0,"width":600,"height":400,"layout":"vertical","gap":8,"padding":16,"children":[
            {"type":"text","id":"t1","width":360,"textGrowth":"fixed-width","fontSize":14,"content":"A wrapped paragraph of body text that needs a few lines at this width to fit."},
            {"type":"rectangle","id":"r1","width":"fill_container","height":40,"fill":[{"type":"solid","color":"#ff0000"}]}
        ]},
        {"type":"browser","id":"b1","x":700,"y":0,"width":390,"height":844,"url":"https://example.com"}
    ]}"##;
    let doc = jian_ops_schema::load_str(json)
        .expect("fixture JSON parses")
        .value;
    let mut state = de_editor_core::EditorState::from_document(doc);
    state.editor_ui.sidebar_open = true;
    *host.editor_state_mut() = state;
    host.mark_paint_dirty_for_test();
}

fn paint_at(host: &mut WidgetHostNative, backend: &mut NativeBackend, w: f32, h: f32) {
    let mut surface =
        skia_safe::surfaces::raster_n32_premul((W as i32, H as i32)).expect("raster surface");
    let mut frame = NativeFrameBackend::new(backend, surface.canvas());
    host.paint(&mut frame, w, h);
}

/// Run `body` on a worker thread and fail if it has not returned in time,
/// so a spin shows up as a failed test instead of a hung test run.
fn must_finish(name: &'static str, body: impl FnOnce() + Send + 'static) {
    let (tx, rx) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        body();
        let _ = tx.send(());
    });
    match rx.recv_timeout(Duration::from_secs(60)) {
        Ok(()) => worker.join().expect("worker finished"),
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            if let Err(panic) = worker.join() {
                std::panic::resume_unwind(panic);
            }
        }
        Err(mpsc::RecvTimeoutError::Timeout) => {
            // The spinning worker would keep its test locks, so a panic here
            // could still hang the run: end the process instead.
            eprintln!("{name} did not return within 60 s");
            std::process::abort();
        }
    }
}

#[test]
fn degenerate_viewports_paint_and_hit_test_without_spinning() {
    must_finish("degenerate paint", || {
        let _indicator_guard = crate::agent_indicator_test_support::read();
        let mut host = WidgetHostNative::new();
        seed_document(&mut host);
        let mut backend = NativeBackend::with_dpi(1.0);
        host.set_now_ms(1_000);
        paint_at(&mut host, &mut backend, W, H);
        for (i, (w, h)) in DEGENERATE.into_iter().enumerate() {
            host.set_now_ms(1_100 + i as u64 * 100);
            host.preview_resize(w, h);
            paint_at(&mut host, &mut backend, w, h);
            let _ = host.cursor_over_layer_panel(10.0, 10.0, w, h);
            let _ = host.update_layer_hover(10.0, 10.0, w, h);
            // Restore: the real size paints again after every squeeze.
            host.preview_resize(W, H);
            paint_at(&mut host, &mut backend, W, H);
        }
    });
}

// Preview tests abort inside the Windows CI Skia stack, like their siblings.
#[cfg(not(target_os = "windows"))]
#[test]
fn degenerate_viewports_in_preview_return() {
    must_finish("degenerate preview", || {
        let _indicator_guard = crate::agent_indicator_test_support::read();
        let mut host = WidgetHostNative::new();
        seed_document(&mut host);
        let mut backend = NativeBackend::with_dpi(1.0);
        host.set_now_ms(1_000);
        paint_at(&mut host, &mut backend, W, H);
        host.toggle_preview((W, H));
        for (i, (w, h)) in DEGENERATE.into_iter().enumerate() {
            host.set_now_ms(1_100 + i as u64 * 100);
            host.preview_resize(w, h);
            paint_at(&mut host, &mut backend, w, h);
            host.preview_resize(W, H);
            paint_at(&mut host, &mut backend, W, H);
        }
    });
}
