//! Which parts of the generation overlay the last canvas frame actually put
//! on screen (#17).
//!
//! The overlay (staggered reveals, the agent cursor, the generating-frame
//! scan) animates at 60 fps, but a batch landing off screen used to keep the
//! whole canvas repainting at that rate until its reveal queue drained,
//! with nothing visible changing. The canvas painter records here what it
//! drew inside the viewport; [`generation_deadline_ms`] keeps the fast
//! cadence only for that and otherwise wakes for state changes alone: the
//! start of a reveal that is on screen, the owed erase frame, the drain
//! point, and a slow tick while a run or reveal is still live (a sprite or
//! reveal that moves on screen without input is caught by that tick).
//!
//! Both hosts get it through `host_frame_bookkeeping::base_animation_deadline_ms`.

use std::cell::Cell;

use crate::Rect;

/// Wake interval while the overlay is live but nothing of it is on screen.
pub const OFFSCREEN_TICK_MS: u64 = 500;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Visibility {
    /// A reveal inside its window, the cursor, or the scan was painted.
    animating: bool,
    /// A reveal that has not started yet sits inside the viewport.
    pending: bool,
}

thread_local! {
    static LAST: Cell<Visibility> = const {
        Cell::new(Visibility {
            animating: false,
            pending: false,
        })
    };
}

/// Start a canvas frame. A culled partial repaint (pan-cache strips) adds to
/// the previous frame's record instead of resetting it.
pub(crate) fn begin_frame(partial: bool) {
    if !partial {
        // Spread a late burst of reveal starts only when the overlay was
        // in view; off screen the slow wakes would otherwise throttle the
        // queue to one start per wake.
        let last = LAST.with(|last| last.replace(Visibility::default()));
        de_editor_core::agent_indicators::set_burst_smoothing(last.animating || last.pending);
    }
}

/// An animated part of the overlay was painted inside the viewport.
pub(crate) fn mark_animating() {
    LAST.with(|last| {
        last.set(Visibility {
            animating: true,
            ..last.get()
        })
    });
}

/// A not-yet-started reveal lies inside the viewport.
pub(crate) fn mark_pending() {
    LAST.with(|last| {
        last.set(Visibility {
            pending: true,
            ..last.get()
        })
    });
}

/// Whether `rect` (screen space) touches `viewport`.
pub(crate) fn on_screen(rect: Rect, viewport: Rect) -> bool {
    rect.origin.x <= viewport.origin.x + viewport.size.x
        && rect.origin.x + rect.size.x >= viewport.origin.x
        && rect.origin.y <= viewport.origin.y + viewport.size.y
        && rect.origin.y + rect.size.y >= viewport.origin.y
}

/// The generation overlay's next wake, gated by what the last frame showed.
pub fn generation_deadline_ms(now_ms: u64) -> Option<u64> {
    use de_editor_core::agent_indicators::{next_generation_scan_deadline_ms, reveal_wake};
    let wake = reveal_wake(now_ms);
    let scan = next_generation_scan_deadline_ms(now_ms);
    let seen = LAST.with(Cell::get);
    let earliest = |a: Option<u64>, b: Option<u64>| match (a, b) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    };
    if seen.animating {
        return earliest(wake.earliest(), scan);
    }
    let mut next = wake.final_frame;
    if seen.pending {
        next = earliest(next, wake.next_start);
    }
    let tick = now_ms.saturating_add(OFFSCREEN_TICK_MS);
    if let Some(settle) = wake.settle {
        next = earliest(next, Some(settle.min(tick)));
    }
    if wake.animating.is_some() || wake.next_start.is_some() || scan.is_some() {
        next = earliest(next, Some(tick));
    }
    next
}

#[cfg(test)]
#[path = "canvas_generation_visibility_tests.rs"]
mod tests;
