//! Frame-cost guard and benchmark for a large, shader-heavy canvas (#208).
//!
//! The fixture stands in for a big pen.dev import: a grid of boards, each
//! with a time-driven shader fill, an image fill, wrapped text and
//! component instances. The guard asserts the steady-state work a frame
//! does when only the shader clock moved (no text relayout, only the
//! animated regions repainted); the ignored benchmark prints frame times:
//!
//! `cargo test --release -p de-host-native --features gl-host --lib large_doc_frame -- --ignored --nocapture`

use super::WidgetHostNative;
use crate::backend::{NativeBackend, NativeFrameBackend};
use de_editor_ui::widgets::canvas_text_layout_memo;
use std::fmt::Write as _;

const W: i32 = 1200;
const H: i32 = 900;
const BOARDS_X: usize = 4;
const BOARDS_Y: usize = 3;

const TIME_SKSL: &str = "uniform float time; uniform float2 size; half4 main(float2 p) { float2 uv = p / size; return half4(0.5 + 0.5 * sin(time + uv.x * 6.0), uv.y, 0.6, 1); }";

/// A tiny PNG as a data URL, so the image fills need no file.
fn png_data_url() -> String {
    let mut surface = skia_safe::surfaces::raster_n32_premul((64, 64)).expect("surface");
    surface
        .canvas()
        .clear(skia_safe::Color::from_rgb(200, 120, 40));
    let image = surface.image_snapshot();
    let data = image
        .encode(None, skia_safe::EncodedImageFormat::PNG, None)
        .expect("png");
    format!("data:image/png;base64,{}", base64(data.as_bytes()))
}

/// Standard base64 with padding (the crate has no base64 dependency).
fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(ALPHABET[(n >> (18 - 6 * i)) as usize & 63] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// The fixture document: `BOARDS_X × BOARDS_Y` boards plus one component.
pub(super) fn large_doc_json() -> String {
    let image = png_data_url();
    let mut nodes = String::from(
        r##"{"type":"frame","id":"chip","name":"Chip","reusable":true,"x":-400,"y":0,"width":120,"height":32,"layout":"horizontal","padding":8,"fill":[{"type":"solid","color":"#e8eef7"}],"children":[{"type":"text","id":"chip-label","content":"Component chip","fontSize":12}]}"##,
    );
    for by in 0..BOARDS_Y {
        for bx in 0..BOARDS_X {
            let id = format!("b{bx}{by}");
            let mut children = String::new();
            write!(
                children,
                r#"{{"type":"rectangle","id":"{id}-shader","x":20,"y":20,"width":220,"height":150,"fill":[{{"type":"shader","sksl":"{TIME_SKSL}"}}]}},"#
            )
            .unwrap();
            write!(
                children,
                r#"{{"type":"rectangle","id":"{id}-image","x":260,"y":20,"width":120,"height":150,"fill":[{{"type":"image","url":"{image}","mode":"fill"}}]}},"#
            )
            .unwrap();
            for t in 0..6 {
                write!(
                    children,
                    r#"{{"type":"text","id":"{id}-t{t}","x":20,"y":{y},"width":360,"textGrowth":"fixed-width","fontSize":14,"content":"Board {bx}{by} line {t}: a wrapped paragraph of body text that needs a few lines at this width to fit."}},"#,
                    y = 190 + t * 40
                )
                .unwrap();
            }
            for i in 0..3 {
                write!(
                    children,
                    r#"{{"type":"ref","id":"{id}-chip{i}","ref":"chip","x":{x},"y":440}},"#,
                    x = 20 + i * 130
                )
                .unwrap();
            }
            children.pop();
            write!(
                nodes,
                r##",{{"type":"frame","id":"{id}","name":"Board {bx}{by}","x":{x},"y":{y},"width":400,"height":500,"fill":[{{"type":"solid","color":"#ffffff"}}],"children":[{children}]}}"##,
                x = bx * 460,
                y = by * 560
            )
            .unwrap();
        }
    }
    format!(r#"{{"version":"1.0.0","children":[{nodes}]}}"#)
}

fn fixture_host() -> WidgetHostNative {
    let doc = jian_ops_schema::load_str(&large_doc_json())
        .expect("fixture JSON parses")
        .value;
    let mut state = de_editor_core::EditorState::from_document(doc);
    state.chat.minimize();
    state.editor_ui.sidebar_open = false;
    // Every board on screen, like a zoom-to-fit overview.
    state.viewport.zoom = 0.45;
    state.viewport.pan_x = 220.0;
    state.viewport.pan_y = 90.0;
    let mut host = WidgetHostNative::new();
    *host.editor_state_mut() = state;
    host.mark_paint_dirty_for_test();
    host
}

fn paint_at(
    host: &mut WidgetHostNative,
    backend: &mut NativeBackend,
    surface: &mut skia_safe::Surface,
    now_ms: u64,
) {
    host.set_now_ms(now_ms);
    surface.canvas().clear(skia_safe::Color::WHITE);
    let mut frame = NativeFrameBackend::new(backend, surface.canvas());
    host.paint(&mut frame, W as f32, H as f32);
}

#[test]
fn steady_shader_frames_repaint_regions_and_relayout_no_text() {
    let _indicator_guard = crate::agent_indicator_test_support::read();
    let fonts0 = jian_skia::font_generation();
    let mut backend = NativeBackend::with_dpi(1.0);
    let mut surface = skia_safe::surfaces::raster_n32_premul((W, H)).expect("surface");
    let mut host = fixture_host();
    // Frame 1 paints direct, frame 2 builds the rest layer.
    paint_at(&mut host, &mut backend, &mut surface, 1_000);
    paint_at(&mut host, &mut backend, &mut surface, 1_016);
    let layouts = canvas_text_layout_memo::computed_layouts();
    let refreshes = host.pan_cache_animation_refreshes;
    let builds = host.pan_cache_builds_for_test();
    for i in 2..6 {
        paint_at(&mut host, &mut backend, &mut surface, 1_000 + i * 16);
    }
    if jian_skia::font_generation() != fonts0 {
        return; // a sibling test's font churn rebuilt the scene
    }
    assert!(
        de_editor_ui::widgets::canvas_shader_clock::animated_shader_painted(),
        "the shaders keep the frame loop alive"
    );
    assert_eq!(
        host.pan_cache_animation_refreshes,
        refreshes + 4,
        "each steady frame repaints the animated regions"
    );
    assert_eq!(host.pan_cache_builds_for_test(), builds, "no layer rebuild");
    assert_eq!(
        canvas_text_layout_memo::computed_layouts(),
        layouts,
        "no text node is laid out again while nothing changed"
    );
    assert_eq!(host.canvas_animation.rects.len(), BOARDS_X * BOARDS_Y);
}

/// Median of `frames` paints in ms.
fn median_ms(mut paint: impl FnMut(u64), frames: u64) -> f64 {
    let mut times: Vec<f64> = (0..frames)
        .map(|i| {
            let t = std::time::Instant::now();
            paint(10_000 + i * 16);
            t.elapsed().as_secs_f64() * 1e3
        })
        .collect();
    times.sort_by(f64::total_cmp);
    times[times.len() / 2]
}

#[test]
#[ignore = "benchmark, run by hand"]
fn large_doc_frame_benchmark() {
    let mut backend = NativeBackend::with_dpi(1.0);
    let mut surface = skia_safe::surfaces::raster_n32_premul((W, H)).expect("surface");
    let mut host = fixture_host();
    paint_at(&mut host, &mut backend, &mut surface, 1_000);
    // Full canvas repaint every frame (the cache dropped each time).
    let full = median_ms(
        |now| {
            host.mark_paint_dirty_for_test();
            paint_at(&mut host, &mut backend, &mut surface, now);
        },
        40,
    );
    // Steady animated frames served from the layer.
    paint_at(&mut host, &mut backend, &mut surface, 2_000);
    paint_at(&mut host, &mut backend, &mut surface, 2_016);
    let served = median_ms(
        |now| paint_at(&mut host, &mut backend, &mut surface, now),
        40,
    );
    println!(
        "BENCH large_doc ({BOARDS_X}x{BOARDS_Y} boards, {W}x{H} raster): full_repaint_median={full:.2}ms animated_frame_median={served:.2}ms"
    );
}
