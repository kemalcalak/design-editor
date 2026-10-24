//! Animated shaders over the retained canvas layer (#208): a frame where
//! only the shader clock moved repaints just the shader boxes into the
//! layer, and must show exactly what a full repaint at that time shows.

use super::WidgetHostNative;
use crate::backend::{NativeBackend, NativeFrameBackend};

const W: i32 = 800;
const H: i32 = 600;

/// Red channel follows the clock, so every frame differs under the shader.
const TIME_SKSL: &str =
    "uniform float time; half4 main(float2 p) { return half4(fract(time), 0.25, 0.5, 1); }";

fn host_with(nodes: &str) -> WidgetHostNative {
    let json = format!(r#"{{"version":"1.0.0","children":[{nodes}]}}"#);
    let doc = jian_ops_schema::load_str(&json)
        .expect("fixture JSON parses")
        .value;
    let mut state = de_editor_core::EditorState::from_document(doc);
    state.chat.minimize();
    state.editor_ui.sidebar_open = false;
    let mut host = WidgetHostNative::new();
    *host.editor_state_mut() = state;
    host.mark_paint_dirty_for_test();
    host
}

fn shader_rect(id: &str, x: f32, y: f32, extra: &str) -> String {
    format!(
        r#"{{"type":"rectangle","id":"{id}","x":{x},"y":{y},"width":120,"height":90{extra},"fill":[{{"type":"shader","sksl":"{TIME_SKSL}"}}]}}"#
    )
}

fn paint_at(host: &mut WidgetHostNative, backend: &mut NativeBackend, now_ms: u64) -> Vec<u8> {
    host.set_now_ms(now_ms);
    let mut surface =
        skia_safe::surfaces::raster_n32_premul((W, H)).expect("raster surface allocated");
    surface.canvas().clear(skia_safe::Color::WHITE);
    {
        let mut frame = NativeFrameBackend::new(backend, surface.canvas());
        host.paint(&mut frame, W as f32, H as f32);
    }
    let stride = (W * 4) as usize;
    let mut pixels = vec![0u8; stride * H as usize];
    let info = skia_safe::ImageInfo::new(
        (W, H),
        skia_safe::ColorType::RGBA8888,
        skia_safe::AlphaType::Premul,
        None,
    );
    assert!(surface.read_pixels(&info, &mut pixels, stride, (0, 0)));
    pixels
}

/// Largest per-channel difference between two frames.
fn max_diff(a: &[u8], b: &[u8]) -> u8 {
    a.iter().zip(b).map(|(x, y)| x.abs_diff(*y)).max().unwrap_or(0)
}

/// Run frames at 1000, 1016, 1032, ... on one host and compare the last
/// with a fresh host painting only that time (a full repaint).
fn served_matches_full_repaint(nodes: &str, frames: u64) -> (WidgetHostNative, u8) {
    let mut backend = NativeBackend::with_dpi(1.0);
    let mut served = host_with(nodes);
    let mut last = Vec::new();
    for i in 0..frames {
        last = paint_at(&mut served, &mut backend, 1_000 + i * 16);
    }
    let mut fresh = host_with(nodes);
    let full = paint_at(&mut fresh, &mut backend, 1_000 + (frames - 1) * 16);
    let diff = max_diff(&last, &full);
    (served, diff)
}

#[test]
fn a_time_shader_frame_repaints_only_its_region_and_matches_a_full_paint() {
    let _indicator_guard = crate::agent_indicator_test_support::read();
    let fonts0 = jian_skia::font_generation();
    let nodes = format!(
        r##"{},{{"type":"rectangle","id":"still","x":420,"y":260,"width":80,"height":80,"fill":[{{"type":"solid","color":"#00aa00"}}]}}"##,
        shader_rect("s1", 200.0, 200.0, "")
    );
    let (host, diff) = served_matches_full_repaint(&nodes, 5);
    if jian_skia::font_generation() != fonts0 {
        return; // font churn from a sibling test rebuilt the scene
    }
    assert!(
        host.pan_cache_animation_refreshes >= 2,
        "rest frames after the layer build repaint only the animated region"
    );
    assert!(host.pan_cache.is_some(), "the layer stays resident at rest");
    assert!(diff <= 1, "served frame differs from a full repaint by {diff}");
}

#[test]
fn content_above_and_rotated_shaders_stay_correct() {
    let _indicator_guard = crate::agent_indicator_test_support::read();
    let fonts0 = jian_skia::font_generation();
    // A rotated shader, and an opaque card painted over half of another one.
    let nodes = format!(
        r##"{},{},{{"type":"rectangle","id":"card","x":260,"y":120,"width":60,"height":200,"fill":[{{"type":"solid","color":"#2040ff"}}]}}"##,
        shader_rect("spin", 420.0, 300.0, r#","rotation":33"#),
        shader_rect("under", 200.0, 150.0, "")
    );
    let (host, diff) = served_matches_full_repaint(&nodes, 6);
    if jian_skia::font_generation() != fonts0 {
        return;
    }
    assert!(host.pan_cache_animation_refreshes >= 2);
    assert!(diff <= 1, "served frame differs from a full repaint by {diff}");
}

#[test]
fn a_backdrop_blur_keeps_full_repaints() {
    let _indicator_guard = crate::agent_indicator_test_support::read();
    let fonts0 = jian_skia::font_generation();
    // Frosted glass over the shader samples it: no region shortcut.
    let nodes = format!(
        r##"{},{{"type":"rectangle","id":"glass","x":250,"y":180,"width":120,"height":120,"fill":[{{"type":"solid","color":"#ffffff40"}}],"effects":[{{"type":"background_blur","radius":12}}]}}"##,
        shader_rect("s1", 200.0, 200.0, "")
    );
    let (host, diff) = served_matches_full_repaint(&nodes, 5);
    if jian_skia::font_generation() != fonts0 {
        return;
    }
    assert_eq!(host.pan_cache_animation_refreshes, 0);
    assert!(diff <= 1, "frame differs from a full repaint by {diff}");
}

#[test]
fn a_still_canvas_builds_no_rest_layer() {
    let _indicator_guard = crate::agent_indicator_test_support::read();
    let nodes = r##"{"type":"rectangle","id":"still","x":200,"y":200,"width":80,"height":80,"fill":[{"type":"solid","color":"#00aa00"}]}"##;
    let mut backend = NativeBackend::with_dpi(1.0);
    let mut host = host_with(nodes);
    for i in 0..3 {
        paint_at(&mut host, &mut backend, 1_000 + i * 16);
    }
    assert!(host.pan_cache.is_none());
    assert_eq!(host.pan_cache_builds_for_test(), 0);
}
