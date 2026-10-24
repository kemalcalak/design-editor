use super::{
    begin_frame, generation_deadline_ms, mark_animating, mark_pending, on_screen, OFFSCREEN_TICK_MS,
};
use crate::Rect;
use de_editor_core::agent_indicators as ind;

const FRAME: u64 = 16;

/// A finishing run whose single reveal started at 1 000 ms.
fn finishing_run_with_reveal_at(start: u64) {
    ind::clear();
    let epoch = ind::begin();
    ind::add_reveal(epoch, "a", start);
    ind::finish_if_epoch(epoch);
}

#[test]
fn an_off_screen_reveal_only_ticks_slowly() {
    let _guard = crate::agent_indicator_test_support::lock();
    finishing_run_with_reveal_at(1_000);
    begin_frame(false);
    assert_eq!(
        generation_deadline_ms(1_100),
        Some(1_100 + OFFSCREEN_TICK_MS)
    );
    ind::clear();
}

#[test]
fn an_on_screen_reveal_keeps_the_frame_cadence() {
    let _guard = crate::agent_indicator_test_support::lock();
    finishing_run_with_reveal_at(1_000);
    begin_frame(false);
    mark_animating();
    assert_eq!(generation_deadline_ms(1_100), Some(1_100 + FRAME));
    ind::clear();
}

#[test]
fn a_visible_pending_reveal_wakes_at_its_start() {
    let _guard = crate::agent_indicator_test_support::lock();
    finishing_run_with_reveal_at(1_300);
    begin_frame(false);
    mark_pending();
    assert_eq!(generation_deadline_ms(1_100), Some(1_300));
    // Off screen the same reveal waits for the slow tick.
    begin_frame(false);
    assert_eq!(
        generation_deadline_ms(1_100),
        Some(1_100 + OFFSCREEN_TICK_MS)
    );
    ind::clear();
}

#[test]
fn the_settle_point_drains_and_owes_one_erase_frame_off_screen() {
    let _guard = crate::agent_indicator_test_support::lock();
    finishing_run_with_reveal_at(1_000);
    begin_frame(false);
    // Near the end of the window the drain point comes before the tick.
    assert_eq!(generation_deadline_ms(1_900), Some(2_000));
    // Past it the finished run drains; the erase frame is owed even though
    // nothing of the overlay was on screen.
    assert_eq!(generation_deadline_ms(2_100), Some(2_100 + FRAME));
    // The paint path consumes the erase; then nothing is scheduled.
    assert!(ind::snapshot_at_if_active(2_116).is_none());
    assert_eq!(generation_deadline_ms(2_116), None);
    ind::clear();
}

#[test]
fn a_live_cursor_off_screen_ticks_and_on_screen_animates() {
    let _guard = crate::agent_indicator_test_support::lock();
    ind::clear();
    let epoch = ind::begin();
    ind::confirm_cursor_agent(epoch, "#4ECDC4", "Mochi");
    begin_frame(false);
    assert_eq!(
        generation_deadline_ms(5_000),
        Some(5_000 + OFFSCREEN_TICK_MS)
    );
    mark_animating();
    assert_eq!(generation_deadline_ms(5_000), Some(5_000 + FRAME));
    ind::end_if_epoch(epoch);
    ind::clear();
}

#[test]
fn a_partial_repaint_keeps_the_last_full_frame_record() {
    let _guard = crate::agent_indicator_test_support::lock();
    begin_frame(false);
    mark_animating();
    begin_frame(true);
    finishing_run_with_reveal_at(1_000);
    assert_eq!(generation_deadline_ms(1_100), Some(1_100 + FRAME));
    ind::clear();
}

#[test]
fn an_off_screen_frame_lets_a_late_burst_start_on_schedule() {
    let _guard = crate::agent_indicator_test_support::lock();
    let starts = |in_view: bool| {
        ind::clear();
        let epoch = ind::begin();
        for i in 0..5_u64 {
            ind::add_reveal(epoch, &format!("n{i}"), 1_000 + i * 20);
        }
        ind::finish_if_epoch(epoch);
        // One snapshot before the queue, one after a slow wake.
        let _ = ind::snapshot_at_if_active(900);
        begin_frame(false);
        if in_view {
            mark_animating();
        }
        begin_frame(false);
        let snap = ind::snapshot_at_if_active(1_500).expect("reveals still live");
        let mut starts: Vec<u64> = snap.reveals.values().copied().collect();
        starts.sort_unstable();
        starts
    };
    // Off screen: the five starts that fell between the wakes stay put.
    assert_eq!(starts(false), vec![1_000, 1_020, 1_040, 1_060, 1_080]);
    // In view: the burst is spread out from the current frame.
    assert!(starts(true).iter().any(|start| *start >= 1_500));
    ind::set_burst_smoothing(true);
    ind::clear();
}

#[test]
fn on_screen_is_an_overlap_test() {
    let viewport = Rect::xywh(0.0, 0.0, 100.0, 100.0);
    assert!(on_screen(Rect::xywh(90.0, 90.0, 50.0, 50.0), viewport));
    assert!(!on_screen(Rect::xywh(120.0, 0.0, 10.0, 10.0), viewport));
    assert!(!on_screen(Rect::xywh(0.0, -30.0, 10.0, 10.0), viewport));
}
