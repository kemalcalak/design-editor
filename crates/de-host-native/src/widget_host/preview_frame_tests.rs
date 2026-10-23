//! Device-frame host lifecycle, scroll, and capture integration tests.

#![cfg(all(test, not(target_os = "windows")))]

use super::{preview_frame::scroll_max, WidgetHostNative};
use de_editor_core::{EditorState, PreviewDeviceKind};
use std::sync::{LazyLock, Mutex, MutexGuard};

static TEST_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

fn test_lock() -> MutexGuard<'static, ()> {
    TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner())
}

fn load(src: &str) -> jian_ops_schema::PenDocument {
    jian_ops_schema::load_str(src)
        .expect("parse device-frame fixture")
        .value
}

fn phone_doc(height: u32) -> jian_ops_schema::PenDocument {
    load(&format!(
        r##"{{
            "version": "1.0.0",
            "children": [{{
                "type": "frame", "id": "screen", "x": 0, "y": 0,
                "width": 390, "height": {height},
                "fill": [{{"type":"solid","color":"#ffffff"}}],
                "children": []
            }}]
        }}"##
    ))
}

fn narrow_semantic_nav_doc() -> jian_ops_schema::PenDocument {
    load(
        r##"{
            "version": "1.0.0",
            "children": [{
                "type": "frame", "id": "screen", "x": 0, "y": 0,
                "width": 390, "height": 2000,
                "fill": [{"type":"solid","color":"#ffffff"}],
                "children": [{
                    "type": "frame", "id": "nav", "x": 95, "y": 1940,
                    "width": 200, "height": 60,
                    "semantics": { "role": "nav" },
                    "fill": [{"type":"solid","color":"#222222"}],
                    "children": []
                }]
            }]
        }"##,
    )
}

fn short_authored_bottom_tab_app_doc() -> jian_ops_schema::PenDocument {
    load(
        r##"{
            "version": "1.1",
            "formatVersion": "1.1",
            "id": "short-app",
            "app": { "name": "Short App", "version": "1", "id": "short-app" },
            "pages": [{
                "id": "canvas", "name": "Canvas", "children": [{
                    "type": "frame", "id": "screen", "screen": "/",
                    "x": 0, "y": 0, "width": 375, "height": 816,
                    "fill": [{"type":"solid","color":"#0b0c10"}],
                    "children": [
                        { "type": "rectangle", "id": "content", "x": 0, "y": 0,
                          "width": 375, "height": 710,
                          "fill": [{"type":"solid","color":"#111111"}] },
                        { "type": "frame", "id": "nav", "name": "Bottom Navigation Bar",
                          "role": "bottom-tab-bar", "x": 0, "y": 726,
                          "width": 375, "height": 80,
                          "fill": [{"type":"solid","color":"#171717"}] }
                    ]
                }]
            }]
        }"##,
    )
}

fn host_with_doc(doc: jian_ops_schema::PenDocument) -> WidgetHostNative {
    let mut host = WidgetHostNative::new();
    let imported = EditorState::from_document(doc);
    host.install_imported_state(imported);
    host
}

fn animated_opacity_doc() -> jian_ops_schema::PenDocument {
    load(
        r##"{
            "version":"1.1","formatVersion":"1.1","id":"motion",
            "app":{"name":"motion","version":"1","id":"motion"},
            "children":[{
                "type":"frame","id":"screen","x":0,"y":0,"width":390,"height":844,
                "fill":[{"type":"solid","color":"#ffffff"}],
                "children":[{
                    "type":"rectangle","id":"button","x":0,"y":0,"width":100,"height":100,
                    "opacity":1,
                    "fill":[{"type":"solid","color":"#000000"}],
                    "events":{"onTap":[{"animate":{
                        "target":"button","property":"opacity","from":1,"to":0.2,
                        "durationMs":600,"iterations":2,"direction":"alternate",
                        "fillMode":"forwards"
                    }}]}
                }]
            }]
        }"##,
    )
}

fn tap_preview_session(host: &mut WidgetHostNative) {
    let session = host.preview.as_mut().expect("preview session");
    let mut down = jian_core::gesture::PointerEvent::simple_at(
        1,
        jian_core::gesture::pointer::PointerPhase::Down,
        jian_core::geometry::point(50.0, 50.0),
        0,
    );
    down.kind = jian_core::gesture::pointer::PointerKind::Touch;
    let _ = session.dispatch_input(de_preview_core::PreviewInputEnvelope::new(
        de_preview_core::PreviewInput::Pointer(down),
    ));
    let mut up = jian_core::gesture::PointerEvent::simple_at(
        1,
        jian_core::gesture::pointer::PointerPhase::Up,
        jian_core::geometry::point(50.0, 50.0),
        0,
    );
    up.kind = jian_core::gesture::pointer::PointerKind::Touch;
    let _ = session.dispatch_input(de_preview_core::PreviewInputEnvelope::new(
        de_preview_core::PreviewInput::Pointer(up),
    ));
}

#[test]
fn preview_animation_deadline_replaces_the_old_blanket_tick() {
    let _guard = test_lock();
    let mut host = host_with_doc(animated_opacity_doc());
    assert!(host.enter_preview((800.0, 600.0)));
    host.preview_mode_transition = None;
    host.set_now_ms(0);
    tap_preview_session(&mut host);

    let deadline = host
        .next_animation_deadline_ms()
        .expect("running animate track must own a wake");
    assert!(deadline <= 16, "first animation wake should be frame paced");
}

#[test]
fn idle_preview_has_no_blanket_33ms_wake() {
    let _guard = test_lock();
    let mut host = host_with_doc(animated_opacity_doc());
    assert!(host.enter_preview((800.0, 600.0)));
    host.preview_mode_transition = None;
    host.set_now_ms(0);

    // The old host kept every idle preview alive with a blanket 33 ms tick.
    // An idle session now contributes no preview-driven wake at all.
    assert_eq!(host.next_animation_deadline_ms(), None);
}

#[test]
fn enter_preview_infers_and_writes_back_kind() {
    let _guard = test_lock();
    let mut host = host_with_doc(phone_doc(800));
    host.enter_preview((800.0, 600.0));
    assert_eq!(
        host.editor_state.editor_ui.preview.device,
        Some(PreviewDeviceKind::Phone)
    );
    host.exit_preview();
    assert_eq!(host.editor_state.editor_ui.preview.device, None);
}

#[test]
fn app_preview_pins_short_authored_bottom_tab_to_device_bottom() {
    let _guard = test_lock();
    let mut host = host_with_doc(short_authored_bottom_tab_app_doc());
    host.last_viewport_w = 800.0;
    host.last_viewport_h = 900.0;
    host.enter_preview((800.0, 900.0));

    let frame = host
        .preview_device_frame
        .as_ref()
        .expect("app preview device frame");
    let pinned = frame.pinned.as_ref().expect("authored bottom tab pinned");
    let strip_bottom = pinned.strip.origin.y + pinned.strip.size.y;
    let device_bottom = frame.frame.origin.y + frame.frame.size.y;

    assert_eq!(pinned.node_id, "nav");
    assert!((strip_bottom - device_bottom).abs() < 0.5);
    assert_eq!(scroll_max(frame), 0.0, "short app screen must not scroll");
}

#[test]
fn manual_pick_wins_until_exit_then_reinfers() {
    let _guard = test_lock();
    let mut host = host_with_doc(phone_doc(800));
    host.enter_preview((800.0, 600.0));
    host.set_preview_device(PreviewDeviceKind::Desktop, 800.0, 600.0);
    assert_eq!(host.infer_device_kind(), PreviewDeviceKind::Desktop);
    host.exit_preview();
    host.enter_preview((800.0, 600.0));
    assert_eq!(
        host.editor_state.editor_ui.preview.device,
        Some(PreviewDeviceKind::Phone),
        "re-enter re-infers"
    );
}

#[test]
fn device_scroll_divides_by_fit_and_clamps() {
    let _guard = test_lock();
    let mut host = host_with_doc(phone_doc(2000));
    host.last_viewport_w = 800.0;
    host.last_viewport_h = 400.0;
    assert!(
        host.enter_preview((800.0, 400.0)),
        "preview entry failed: {:?}",
        host.editor_state().editor_ui.preview.warnings
    );
    host.recompute_device_frame(800.0, 400.0);
    let fit = host.preview_device_frame.as_ref().unwrap().fit;
    host.apply_device_scroll(-100.0);
    assert!((host.preview_scroll_y - 100.0 / fit).abs() < 0.5);
    host.apply_device_scroll(1e6);
    assert_eq!(host.preview_scroll_y, 0.0);
}

/// Page-scroll contract: the device frame's scroll position is mirrored
/// into the session so `$scroll` under the framed root tracks it.
#[test]
fn device_scroll_feeds_the_session_page_scroll() {
    let _guard = test_lock();
    let mut host = host_with_doc(phone_doc(2000));
    host.last_viewport_w = 800.0;
    host.last_viewport_h = 400.0;
    assert!(host.enter_preview((800.0, 400.0)));
    host.recompute_device_frame(800.0, 400.0);
    let frame = host.preview_device_frame.as_ref().unwrap();
    let (fit, max) = (frame.fit, scroll_max(frame));
    assert!(max > 0.0, "a 2000 px screen overflows the frame");
    assert_eq!(
        host.preview.as_ref().unwrap().page_scroll(),
        Some((0.0, max)),
        "entering preview publishes max_offset before any scroll"
    );
    host.apply_device_scroll(-100.0);
    let (offset, published_max) = host.preview.as_ref().unwrap().page_scroll().unwrap();
    assert!(
        (offset - 100.0 / fit).abs() < 0.5,
        "offset mirrors preview_scroll_y"
    );
    assert_eq!(published_max, max);
    assert_eq!(offset, host.preview_scroll_y);
}

#[test]
fn dead_zone_press_leaves_no_stale_capture() {
    let _guard = test_lock();
    let mut host = host_with_doc(narrow_semantic_nav_doc());
    host.last_viewport_w = 800.0;
    host.last_viewport_h = 900.0;
    host.enter_preview((800.0, 900.0));
    host.recompute_device_frame(800.0, 900.0);
    let strip = host
        .preview_device_frame
        .as_ref()
        .unwrap()
        .pinned
        .as_ref()
        .unwrap()
        .strip;
    let consumed =
        host.preview_dispatch_press(strip.origin.x + 2.0, strip.origin.y + 10.0, 800.0, 900.0);
    assert!(!consumed, "dead-zone press must not dispatch");
    assert!(host.preview_surface_capture.is_none());
    assert!(!host.preview_dispatch_release());
    assert!(host.preview_surface_capture.is_none());
}

#[test]
fn screen_switch_resets_scroll_and_reinfers() {
    let _guard = test_lock();
    let mut host = host_with_doc(phone_doc(2000));
    host.last_viewport_w = 800.0;
    host.last_viewport_h = 900.0;
    host.enter_preview((800.0, 900.0));
    host.recompute_device_frame(800.0, 900.0);
    host.preview_scroll_y = 250.0;
    host.on_preview_screen_switched(800.0, 900.0);
    assert_eq!(host.preview_scroll_y, 0.0);
    assert!(host.preview_surface_capture.is_none());
    assert!(host.preview_device_frame.is_some());
}

// --- Track C-3 real-frame regression (the user-reported "闪切无动画") ---
//
// The unit tests in `preview/tests_transition.rs` prove
// `PreviewSession::paint_framed_animated` composites correctly when
// called directly. What they don't prove is that the PRODUCTION path —
// `WidgetHostNative::paint_device_frame` fed by the per-frame
// `reconcile` + the redraw-scheduling deadline aggregation
// (`next_animation_deadline_ms`) — actually renders more than one
// distinct frame during the animation window. If the deadline chain
// or the device-frame rebuild broke, the symptom would look exactly
// like "code is there but nothing visibly animates": one composited
// frame painted, then a jump straight to the settled screen.

fn two_screen_tabbed_doc() -> jian_ops_schema::PenDocument {
    load(
        r##"{
            "version": "1.1",
            "formatVersion": "1.1",
            "id": "x",
            "app": { "name": "x", "version": "1", "id": "x" },
            "pages": [
                { "id": "canvas", "name": "Canvas", "children": [
                    { "type": "frame", "id": "home", "name": "Home", "screen": "/",
                      "x": 0, "y": 0, "width": 390, "height": 800,
                      "fill": [{"type":"solid","color":"#ff0000"}], "children": [] },
                    { "type": "frame", "id": "profile", "name": "Profile", "screen": "/profile",
                      "x": 500, "y": 0, "width": 390, "height": 800,
                      "fill": [{"type":"solid","color":"#0000ff"}], "children": [] }
                ] }
            ]
        }"##,
    )
}

/// Records every `fill_rect` alpha plus the opacity applied by an isolated
/// fill-stack layer. Canonical multi-fill scenes apply node opacity when the
/// layer is composited, rather than mutating every fill colour.
#[derive(Default)]
struct AlphaCaptureBackend {
    fill_alphas: Vec<f32>,
    composite_alphas: Vec<f32>,
}

impl de_editor_ui::RenderBackend for AlphaCaptureBackend {
    fn begin_frame(&mut self) {}
    fn end_frame(&mut self) {}
    fn fill_rect(&mut self, _: de_editor_ui::Rect, color: de_editor_ui::Color) {
        self.fill_alphas.push(color.a);
    }
    fn stroke_rect(&mut self, _: de_editor_ui::Rect, _: de_editor_ui::Color, _: f32) {}
    fn draw_text(&mut self, _: &de_editor_ui::TextLayout, _: de_editor_ui::Point2D) {}
    fn clip_rect(&mut self, _: de_editor_ui::Rect) {}
    fn save(&mut self) {}
    fn push_composite_layer(
        &mut self,
        _: de_editor_ui::Rect,
        opacity: f32,
        _: de_editor_ui::ImageBlendMode,
    ) {
        self.composite_alphas.push(opacity);
    }
    fn restore(&mut self) {}
    fn translate(&mut self, _: de_editor_ui::Point2D) {}
    fn stroke_line(
        &mut self,
        _: de_editor_ui::Point2D,
        _: de_editor_ui::Point2D,
        _: de_editor_ui::Color,
        _: f32,
    ) {
    }
    fn fill_round_rect(&mut self, _: de_editor_ui::Rect, _: f32, _: de_editor_ui::Color) {}
    fn stroke_round_rect(&mut self, _: de_editor_ui::Rect, _: f32, _: de_editor_ui::Color, _: f32) {
    }
    fn stroke_svg_path(
        &mut self,
        _: &str,
        _: de_editor_ui::Point2D,
        _: f32,
        _: de_editor_ui::Color,
        _: f32,
    ) {
    }
    fn resize(&mut self, _: u32, _: u32) {}
    fn dpi_scale(&self) -> f32 {
        1.0
    }
}

#[test]
fn tab_switch_animates_across_real_frames_through_the_production_paint_path() {
    let _guard = test_lock();
    let mut host = host_with_doc(two_screen_tabbed_doc());
    assert!(host.enter_preview((1200.0, 800.0)));
    assert!(
        host.preview.as_ref().unwrap().is_app_mode(),
        "two authored screens must enter APP MODE"
    );
    assert_eq!(
        host.editor_state.editor_ui.preview.device,
        Some(PreviewDeviceKind::Phone)
    );

    host.set_now_ms(1_000);
    // Same navigation shape BOTH real paths drive: the screen-switcher
    // pill's release handler and a runtime `onTap` binding both end in
    // `router.replace(path)` — same stack depth, so `reconcile`
    // classifies it `Replace` (the 160ms cross-fade), exactly the kind
    // the user's own repro (device switcher + pill row) exercises.
    host.preview
        .as_ref()
        .unwrap()
        .navigate_to_screen("/profile");
    let outcome = host.preview.as_mut().unwrap().reconcile(1_000);
    assert!(outcome.switched, "same-depth replace must be detected");
    host.on_preview_screen_switched(1200.0, 800.0);

    // The redraw-scheduling deadline-aggregation chain
    // (`next_animation_deadline_ms`, consumed by
    // `de-host-desktop::app_handler`'s `WaitUntil` scheduling) must
    // keep reporting a pending wake for the whole animation window —
    // otherwise the compositor below never gets a second real frame to
    // advance the fade on.
    assert!(
        host.next_animation_deadline_ms().is_some(),
        "the host must keep scheduling wakeups while previewing, or the \
         transition renders its first frame and then waits indefinitely"
    );

    let canvas_rect = de_editor_ui::Rect {
        origin: de_editor_ui::Point2D::new(0.0, 0.0),
        size: de_editor_ui::Point2D::new(1200.0, 800.0),
    };

    host.set_now_ms(1_010);
    let mut early = AlphaCaptureBackend::default();
    host.paint_device_frame(&mut early, canvas_rect);

    host.set_now_ms(1_150);
    let mut late = AlphaCaptureBackend::default();
    host.paint_device_frame(&mut late, canvas_rect);

    assert_eq!(
        early.fill_alphas.len(),
        2,
        "mid-animation: both the outgoing and entering root paint"
    );
    assert_eq!(late.fill_alphas.len(), 2);
    assert_ne!(
        early.composite_alphas, late.composite_alphas,
        "the composited alpha must move between two real frames sampled \
         inside the 160ms window — a transition that paints identical \
         alpha at two different real timestamps is indistinguishable \
         from an instant cut with a wasted animation object sitting \
         behind it"
    );

    host.set_now_ms(1_400);
    let mut settled = AlphaCaptureBackend::default();
    host.paint_device_frame(&mut settled, canvas_rect);
    assert_eq!(
        settled.fill_alphas,
        vec![1.0],
        "once the 160ms window elapses only the destination screen \
         paints, at full alpha"
    );
    assert!(
        settled.composite_alphas.is_empty(),
        "the settled opaque fill stack no longer needs an opacity layer"
    );
}

/// Viewport for the switcher tests: wide enough that the floating chrome
/// halves sit clear of the centred device pill.
const SWITCHER_VIEWPORT: (f32, f32) = (1400.0, 900.0);

/// A phone screen parked far off the default viewport, so no edit-scene
/// node sits under the switcher and only the pill itself can refuse a
/// window drag.
fn offscreen_phone_doc() -> jian_ops_schema::PenDocument {
    load(
        r##"{
            "version": "1.0.0",
            "children": [{
                "type": "frame", "id": "screen", "x": 5000, "y": 5000,
                "width": 390, "height": 844,
                "fill": [{"type":"solid","color":"#ffffff"}],
                "children": []
            }]
        }"##,
    )
}

fn switcher_segment_centres(host: &WidgetHostNative) -> Vec<(PreviewDeviceKind, f32, f32)> {
    let (viewport_w, viewport_h) = SWITCHER_VIEWPORT;
    let canvas = host.preview_canvas_rect(viewport_w, viewport_h);
    de_editor_ui::widgets::PreviewDeviceSwitcher::segment_rects(canvas)
        .into_iter()
        .map(|(kind, rect)| {
            (
                kind,
                rect.origin.x + rect.size.x / 2.0,
                rect.origin.y + rect.size.y / 2.0,
            )
        })
        .collect()
}

#[test]
fn device_switcher_is_not_a_window_drag_handle() {
    let _guard = test_lock();
    let mut host = host_with_doc(offscreen_phone_doc());
    let (viewport_w, viewport_h) = SWITCHER_VIEWPORT;
    assert!(host.enter_preview((viewport_w, viewport_h)));
    for (kind, x, y) in switcher_segment_centres(&host) {
        assert!(
            y < de_editor_ui::widgets::WINDOW_STRIP_H,
            "the pill sits inside the window strip, so the carve-out matters"
        );
        assert!(
            !host.editor_window_drag_at(x, y, viewport_w, viewport_h),
            "a press on the {kind:?} segment must reach the switcher, not drag the window"
        );
    }
    // The empty band beside the pill still drags the window.
    let canvas = host.preview_canvas_rect(viewport_w, viewport_h);
    let pill = de_editor_ui::widgets::PreviewDeviceSwitcher::pill_rect(canvas);
    assert!(host.editor_window_drag_at(
        pill.origin.x - 20.0,
        pill.origin.y + pill.size.y / 2.0,
        viewport_w,
        viewport_h
    ));
}

#[test]
fn press_and_release_on_each_segment_switches_the_device() {
    let _guard = test_lock();
    let mut host = host_with_doc(offscreen_phone_doc());
    let (viewport_w, viewport_h) = SWITCHER_VIEWPORT;
    assert!(host.enter_preview((viewport_w, viewport_h)));
    // The desktop runner asks `editor_window_drag_at` only while Home is
    // hidden, then hands the press to `apply_press`; follow that order.
    assert!(!host.home_visible());
    // Phone last: the session starts on the inferred Phone, so every press
    // is a real switch.
    let mut segments = switcher_segment_centres(&host);
    segments.sort_by_key(|(kind, _, _)| *kind == PreviewDeviceKind::Phone);
    for (kind, x, y) in segments {
        assert!(!host.editor_window_drag_at(x, y, viewport_w, viewport_h));
        assert!(host.apply_press(x, y, viewport_w, viewport_h));
        assert!(host.apply_release_with_viewport(viewport_w, viewport_h));
        assert_eq!(host.editor_state.editor_ui.preview.device, Some(kind));
        assert_eq!(
            host.preview_device_frame.is_some(),
            kind != PreviewDeviceKind::Canvas
        );
    }
}
