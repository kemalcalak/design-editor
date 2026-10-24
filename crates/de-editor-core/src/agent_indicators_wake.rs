//! When the host has to repaint for the generation overlay (reveals, the
//! agent cursor, the generating-frame scan).
//!
//! [`reveal_wake`] splits the answer by what the frame needs it for, so the
//! host can keep the 60 fps cadence for overlays that are actually on
//! screen and only wake for state changes otherwise (#17: a 300-node batch
//! off screen kept the whole canvas repainting for most of a minute).
//! [`next_reveal_deadline_ms`] is the ungated combination.

use std::sync::atomic::{AtomicBool, Ordering};

use super::{
    drain_finished_run, rebase_external_clock_reveals, REGISTRY, REVEAL_DURATION_MS,
    REVEAL_FRAME_MS,
};

/// Whether a burst of reveals that became due together is spread out again
/// (`smooth_overdue_reveal_burst`). The smoother assumes ~60 fps snapshots;
/// while the overlay is off screen the host wakes slowly, and at one start
/// per snapshot it would stretch the queue to minutes. Nobody sees those
/// starts, so the canvas turns it off while the last frame showed no part
/// of the overlay.
static BURST_SMOOTHING: AtomicBool = AtomicBool::new(true);

/// Enable or disable burst smoothing (see [`BURST_SMOOTHING`]).
pub fn set_burst_smoothing(enabled: bool) {
    BURST_SMOOTHING.store(enabled, Ordering::Relaxed);
}

pub(super) fn burst_smoothing() -> bool {
    BURST_SMOOTHING.load(Ordering::Relaxed)
}

/// The overlay's wake-up needs at one instant (host-clock ms).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RevealWake {
    /// Next frame of a running animation (a reveal inside its window, or the
    /// live run's cursor): `now + REVEAL_FRAME_MS`.
    pub animating: Option<u64>,
    /// The earliest reveal that has not started yet.
    pub next_start: Option<u64>,
    /// When the last scheduled reveal leaves its window — the point a
    /// finishing run drains (`None` while the run is live: reveals stay
    /// parked until it finishes).
    pub settle: Option<u64>,
    /// The overlay was just cleared and one erase frame is owed:
    /// `now + REVEAL_FRAME_MS`. Never gated — the last painted cursor or
    /// badge must not stay on screen.
    pub final_frame: Option<u64>,
}

impl RevealWake {
    /// The ungated deadline: every need folded together.
    pub fn earliest(&self) -> Option<u64> {
        [self.animating, self.next_start, self.final_frame]
            .into_iter()
            .flatten()
            .min()
    }
}

/// The overlay's wake-up needs at `now_ms`. Runs the same maintenance as the
/// paint path (clock rebase, pruning, draining a finished run).
pub fn reveal_wake(now_ms: u64) -> RevealWake {
    let mut r = REGISTRY.lock().unwrap();
    rebase_external_clock_reveals(&mut r, now_ms);
    if !r.run_active {
        r.reveals
            .retain(|_, started| now_ms.saturating_sub(*started) <= REVEAL_DURATION_MS);
    }
    drain_finished_run(&mut r);
    let frame = now_ms.saturating_add(REVEAL_FRAME_MS);
    let next_start = r
        .reveals
        .values()
        .filter(|started| **started > now_ms)
        .min()
        .copied();
    let active_reveal = r
        .reveals
        .values()
        .any(|started| *started <= now_ms && now_ms.saturating_sub(*started) <= REVEAL_DURATION_MS);
    let active_cursor = r.run_active && r.cursor_agent.is_some();
    let settle = (!r.run_active)
        .then(|| r.reveals.values().max().copied())
        .flatten()
        .map(|started| started.saturating_add(REVEAL_DURATION_MS).max(frame));
    let animating = (active_reveal || active_cursor).then_some(frame);
    // Nothing animating — but if the drain just cleared the overlay, keep
    // asking for one more frame until the paint path consumes the erase
    // (otherwise the last-painted cursor stays on screen).
    let final_frame =
        (animating.is_none() && next_start.is_none() && r.needs_final_frame).then_some(frame);
    RevealWake {
        animating,
        next_start,
        settle,
        final_frame,
    }
}

/// Next host-clock millisecond needed for generated-node reveal animation,
/// whether or not anything is on screen.
pub fn next_reveal_deadline_ms(now_ms: u64) -> Option<u64> {
    reveal_wake(now_ms).earliest()
}

/// Next host-clock millisecond needed for the generating-frame scan animation.
pub fn next_generation_scan_deadline_ms(now_ms: u64) -> Option<u64> {
    let mut r = REGISTRY.lock().unwrap();
    rebase_external_clock_reveals(&mut r, now_ms);
    let reveal_runway_pending = r.reveals.values().any(|started_at| *started_at > now_ms);
    (!r.frames.is_empty() && (r.run_active || reveal_runway_pending))
        .then(|| now_ms.saturating_add(REVEAL_FRAME_MS))
}
