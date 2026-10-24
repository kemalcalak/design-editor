//! Local text paint benchmark (#197): `DE_BENCH_DOC=<file.de> cargo test
//! --release -p de-host-services --test text_paint_bench -- --ignored --nocapture`.

use std::time::Instant;

#[test]
#[ignore = "benchmark, run by hand"]
fn text_heavy_document_frame_time() {
    let Ok(path) = std::env::var("DE_BENCH_DOC") else {
        return;
    };
    let state = de_host_services::doc_io::load_editor_state(
        std::path::Path::new(&path),
        de_editor_core::Locale::EnUs,
    )
    .expect("load");
    let t = Instant::now();
    let scene = de_pen_loader::editor_state_to_active_page_layout_scene(&state);
    let layout_cold = t.elapsed();
    let t = Instant::now();
    let _ = de_pen_loader::editor_state_to_active_page_layout_scene(&state);
    let layout_warm = t.elapsed();
    let page = scene.active_page().expect("page");
    let bounds = de_render_export::page_bounds(page).expect("bounds");
    let (w, h) = (
        bounds.size.x.ceil().min(8000.0) as i32,
        bounds.size.y.ceil().min(8000.0) as i32,
    );
    let mut surface = skia_safe::surfaces::raster_n32_premul((w, h)).expect("surface");
    let canvas = surface.canvas();
    canvas.translate((-bounds.origin.x, -bounds.origin.y));
    let t = Instant::now();
    de_render_export::paint_nodes(canvas, &page.children);
    let first = t.elapsed();
    let frames = 30;
    let t = Instant::now();
    for _ in 0..frames {
        de_render_export::paint_nodes(canvas, &page.children);
    }
    let warm = t.elapsed() / frames;
    println!(
        "BENCH layout_cold={:.1}ms layout_warm={:.1}ms first_frame={:.1}ms warm_frame={:.2}ms ({w}x{h})",
        layout_cold.as_secs_f64() * 1e3,
        layout_warm.as_secs_f64() * 1e3,
        first.as_secs_f64() * 1e3,
        warm.as_secs_f64() * 1e3,
    );
}
