//! Minimize / restore (#302): Windows reports a 0×0 client area on
//! minimize. That size must not reach the viewport, the preview layout or
//! the persisted window size, nothing paints until the restore, and the
//! restore's real size paints again.

use super::*;
use winit::dpi::PhysicalSize;

fn app_at(width: u32, height: u32) -> DesktopApp {
    let mut app = DesktopApp::new(None);
    app.dpi = 2.0;
    assert!(app.apply_window_size(PhysicalSize::new(width, height)));
    app.redraw_pending = false;
    app.redraw_dirty = false;
    app
}

#[test]
fn zero_size_resize_keeps_the_last_viewport_and_saved_size() {
    let mut app = app_at(2560, 1600);
    for zero in [(0, 0), (0, 1600), (2560, 0)] {
        assert!(!app.apply_window_size(PhysicalSize::new(zero.0, zero.1)));
        assert_eq!((app.viewport_width, app.viewport_height), (1280.0, 800.0));
        assert_eq!(app.win_size, Some((2560, 1600)));
        assert!(app.surface_hidden(), "a 0×0 window is not painted");
        assert!(!app.redraw_pending, "a 0×0 resize schedules no frame");
    }
}

#[test]
fn restore_after_minimize_takes_the_real_size_and_repaints() {
    let mut app = app_at(2560, 1600);
    assert!(!app.apply_window_size(PhysicalSize::new(0, 0)));
    assert!(app.apply_window_size(PhysicalSize::new(2000, 1200)));
    assert!(!app.surface_hidden());
    assert_eq!((app.viewport_width, app.viewport_height), (1000.0, 600.0));
    assert_eq!(app.win_size, Some((2000, 1200)));
    assert!(app.redraw_pending && app.redraw_dirty, "the restore paints");
}

#[test]
fn a_hidden_window_skips_paint_but_keeps_the_frame_owed() {
    let mut app = app_at(2560, 1600);
    assert!(!app.apply_window_size(PhysicalSize::new(0, 0)));
    app.request_redraw(true);
    let should_paint = app.prepare_redraw();
    assert!(should_paint, "the frame was dirty");
    assert!(!app.hold_frame_while_hidden(should_paint));
    assert!(app.redraw_dirty && !app.redraw_pending);
    // The restore schedules the owed frame and it paints.
    assert!(app.apply_window_size(PhysicalSize::new(2560, 1600)));
    let should_paint = app.prepare_redraw();
    assert!(app.hold_frame_while_hidden(should_paint));
}

#[test]
fn occlusion_pauses_painting_and_the_reveal_repaints() {
    let mut app = app_at(2560, 1600);
    app.on_occluded(true);
    assert!(app.surface_hidden());
    assert!(!app.redraw_pending);
    app.on_occluded(false);
    assert!(!app.surface_hidden());
    assert!(app.redraw_pending && app.redraw_dirty);
    assert_eq!((app.viewport_width, app.viewport_height), (1280.0, 800.0));
}

#[test]
fn a_hidden_window_does_not_keep_an_animation_wake_due() {
    let mut app = app_at(2560, 1600);
    // Home's focused input blinks its caret: an animation deadline.
    app.host.editor_state_mut().editor_ui.home.visible = true;
    assert!(app.host.next_animation_deadline_ms().is_some());
    assert!(app.visible_animation_deadline_ms().is_some());
    assert!(!app.apply_window_size(PhysicalSize::new(0, 0)));
    // Asked of the deadline itself, not `resume_time_needs_redraw`: that
    // also reads process-wide image queues other tests fill in parallel.
    assert!(
        app.visible_animation_deadline_ms().is_none(),
        "only painted frames advance animation deadlines"
    );
}
