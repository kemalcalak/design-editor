//! Host wiring for the floating chrome's hover tooltip: hovering a chrome
//! button through the real cursor path starts the dwell clock, and the
//! dwell's expiry both schedules a wake and is still recognisable as owing
//! a paint once that wake advances the clock past it.

use super::*;
use de_editor_core::ChromeButton;
use de_editor_ui::widgets::chrome_tooltip::{button_anchor, TOOLTIP_DWELL_MS};
use de_editor_ui::Point2D;

const VIEWPORT_W: f32 = 1400.0;
const VIEWPORT_H: f32 = 900.0;

fn hosted() -> WidgetHostNative {
    let mut host = WidgetHostNative::new();
    host.editor_state.editor_ui.home.hide();
    host.last_viewport_w = VIEWPORT_W;
    host.last_viewport_h = VIEWPORT_H;
    host
}

/// Centre of a chrome button, in window coordinates.
fn button_centre(host: &WidgetHostNative, button: ChromeButton) -> Point2D {
    let rect = button_anchor(host.editor_state(), VIEWPORT_W, VIEWPORT_H, button)
        .expect("button paints in this build");
    Point2D::new(
        rect.origin.x + rect.size.x / 2.0,
        rect.origin.y + rect.size.y / 2.0,
    )
}

#[test]
fn hovering_a_chrome_button_starts_the_dwell_clock() {
    let mut host = hosted();
    host.set_now_ms(1_000);
    let point = button_centre(&host, ChromeButton::ToggleCodeView);
    host.apply_cursor_move(point.x, point.y);

    let ui = &host.editor_state().editor_ui;
    assert_eq!(
        ui.chrome_button_hover,
        Some(ChromeButton::ToggleCodeView),
        "the code button should be hovered at its own centre"
    );
    assert_eq!(ui.chrome_hover_since_ms, Some(1_000));
    assert!(
        !host.chrome_tooltip_showing(),
        "nothing should show the instant the cursor lands"
    );
}

#[test]
fn a_dwelling_tooltip_asks_the_scheduler_to_wake_for_it() {
    let mut host = hosted();
    host.set_now_ms(1_000);
    let point = button_centre(&host, ChromeButton::OpenExportMenu);
    host.apply_cursor_move(point.x, point.y);

    assert_eq!(
        host.next_animation_deadline_ms(),
        Some(1_000 + TOOLTIP_DWELL_MS),
        "the dwell's expiry has to reach the wake scheduler"
    );
}

/// The bug this pair exists to prevent: the runner advances the clock
/// BEFORE deciding whether the wake needs a paint. By then the dwell
/// deadline has retired, so a runner asking only about deadlines would
/// discard the very wake it scheduled. `chrome_tooltip_showing` is what
/// keeps that wake.
#[test]
fn the_dwell_wake_is_still_recognisable_after_the_clock_passes_it() {
    let mut host = hosted();
    host.set_now_ms(1_000);
    let point = button_centre(&host, ChromeButton::OpenExportMenu);
    host.apply_cursor_move(point.x, point.y);

    host.set_now_ms(1_000 + TOOLTIP_DWELL_MS);
    assert_eq!(
        host.next_animation_deadline_ms(),
        None,
        "the deadline retires the moment it is reached"
    );
    assert!(
        host.chrome_tooltip_showing(),
        "…so the tooltip itself must be what keeps the wake alive"
    );
}

#[test]
fn leaving_the_chrome_takes_the_tooltip_with_it() {
    let mut host = hosted();
    host.set_now_ms(1_000);
    let point = button_centre(&host, ChromeButton::OpenExportMenu);
    host.apply_cursor_move(point.x, point.y);
    host.set_now_ms(1_000 + TOOLTIP_DWELL_MS);
    assert!(host.chrome_tooltip_showing());

    // Down into the canvas.
    host.apply_cursor_move(VIEWPORT_W / 2.0, 400.0);
    assert_eq!(host.editor_state().editor_ui.chrome_button_hover, None);
    assert!(!host.chrome_tooltip_showing());
    assert_eq!(host.next_animation_deadline_ms(), None);
}

#[test]
fn sliding_along_the_group_swaps_labels_without_waiting_again() {
    let mut host = hosted();
    host.set_now_ms(1_000);
    let export = button_centre(&host, ChromeButton::OpenExportMenu);
    host.apply_cursor_move(export.x, export.y);
    host.set_now_ms(1_000 + TOOLTIP_DWELL_MS);
    assert!(host.chrome_tooltip_showing());

    let preview = button_centre(&host, ChromeButton::TogglePreview);
    host.apply_cursor_move(preview.x, preview.y);
    assert_eq!(
        host.editor_state().editor_ui.chrome_button_hover,
        Some(ChromeButton::TogglePreview)
    );
    assert!(
        host.chrome_tooltip_showing(),
        "a settled tooltip must follow the cursor along the group, not restart its wait"
    );
}
