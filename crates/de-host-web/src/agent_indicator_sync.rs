//! Poll the daemon's agent-indicator registry and mirror it locally.
//!
//! Design runs execute inside the serve-web daemon, so the
//! process-global `agent_indicators` registry the shared canvas paint
//! reads is populated in the DAEMON process — the browser's wasm
//! registry would stay empty and the agent borders / badges / reveal
//! animations would never show on web. This module polls
//! `GET /api/mcp/indicators` on the live-sync cadence, mirrors each
//! snapshot into the local registry (`agent_indicators::apply_remote`),
//! and drives a self-terminating rAF pump while anything animates so
//! the entrance/breathing animations actually play between DOM events.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::live_sync;
use crate::repaint_ctx::RepaintContext;

/// Poll cadence — matches the live-sync document pull tick; the idle
/// body is a tiny `{"active":false,…}` so the steady-state cost is one
/// small same-origin GET per tick.
const INDICATOR_POLL_INTERVAL_MS: i32 = 400;

/// Wire the indicator relay onto the mounted shell. Called once from
/// `mount()`; the interval runs for the page lifetime.
pub(crate) fn start<C: RepaintContext + 'static>(inner: &Rc<RefCell<C>>) {
    // One request in flight at a time; ticks observing a pending fetch skip.
    let busy = Rc::new(Cell::new(false));
    let pump_running = Rc::new(Cell::new(false));
    let inner = inner.clone();
    let tick: Rc<dyn Fn()> = Rc::new(move || {
        if busy.get() {
            return;
        }
        busy.set(true);
        let url = crate::daemon_base::daemon_url("/api/mcp/indicators");
        let busy_cb = busy.clone();
        let inner_cb = inner.clone();
        let pump_running_cb = pump_running.clone();
        let ok = live_sync::get(
            &url,
            Rc::new(move |body: String| {
                busy_cb.set(false);
                let Some(remote) = de_editor_core::agent_indicators::parse_relay_json(&body) else {
                    return;
                };
                if de_editor_core::agent_indicators::apply_remote(&remote) {
                    crate::repaint_coalescer::request();
                    ensure_pump(&inner_cb, &pump_running_cb);
                }
            }),
        );
        if !ok {
            busy.set(false);
        }
    });
    let _ = live_sync::start_interval(INDICATOR_POLL_INTERVAL_MS, tick);
}

/// Keep repainting on animation frames while indicators are active.
/// The web host normally repaints on DOM events only, so without this
/// pump the reveal / breathing animations would freeze between events.
/// The pump ends itself (and can be re-armed by a later poll) once the
/// registry drains.
fn ensure_pump<C: RepaintContext + 'static>(inner: &Rc<RefCell<C>>, running: &Rc<Cell<bool>>) {
    if running.get() {
        return;
    }
    running.set(true);
    let inner = inner.clone();
    let running = running.clone();
    // Next instant the overlay needs a frame. The pump keeps ticking on
    // every animation frame but repaints only when that is due: 60 fps for
    // overlays the last frame showed on screen, state changes otherwise
    // (#17, `canvas_generation_visibility`).
    let due = Rc::new(Cell::new(0_u64));
    crate::raf_pump::start(Rc::new(move || {
        let Ok(mut inner_mut) = inner.try_borrow_mut() else {
            // Another borrow is mid-flight (e.g. a DOM event) — that
            // path repaints itself; try again next frame.
            return true;
        };
        // Advance the host clock so the paint pass sees time move —
        // reveal entrances and the breathing border are time-driven.
        let now = crate::listener::now_ms_perf();
        // Re-read every tick: a DOM-event repaint (a pan bringing a reveal
        // into view) may have moved the overlay on screen since.
        due.set(due.get().min(next_overlay_frame(now)));
        if now >= due.get() {
            let unix = crate::listener::now_unix_secs();
            inner_mut.host_mut().set_clocks(now, unix);
            let _ = inner_mut.repaint();
            due.set(next_overlay_frame(now));
        }
        let alive = de_editor_core::agent_indicators::is_active();
        if !alive {
            running.set(false);
        }
        alive
    }));
}

/// The overlay's next frame, or a slow tick while indicators linger with
/// nothing scheduled (badges only).
fn next_overlay_frame(now_ms: u64) -> u64 {
    crate::widget_host::generation_overlay_deadline_ms(now_ms)
        .unwrap_or(now_ms + crate::widget_host::OVERLAY_IDLE_TICK_MS)
}
