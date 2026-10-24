//! Frame clock for time-driven shader fills.
//!
//! A shader fill whose SkSL declares `uniform float time;` (or `half time`)
//! receives the canvas clock in seconds, bound next to its authored uniforms
//! the same way the loader binds `size`. The canvas viewport sets the clock
//! once per painted frame; exports and screenshots pin it to `0` through
//! [`with_fixed_clock`], so their pixels never depend on when they ran.
//!
//! Painting such a shader marks the frame as animated. Both hosts repaint only
//! on demand, so the shared deadline chain folds in [`next_deadline_ms`] and the
//! browser host (which has no deadline scheduler) asks for its next frame
//! from the same deadline after each paint; without that the shader would
//! freeze on the first frame it was painted in. [`animated_shader_painted`]
//! (also read by the native pan cache) covers every kind of animated
//! content, not only shaders.
//!
//! Other self-animating canvas content (the in-progress sweep on placeholder
//! frames, #16) marks the frame through [`mark_animated`] with its own
//! cadence; the frame repaints at the fastest cadence any painter asked for.

use std::cell::Cell;

/// Uniform name the canvas binds to the elapsed seconds.
pub const TIME_UNIFORM: &str = "time";

/// Repaint cadence while a time-driven shader is on screen (~60 fps).
pub const FRAME_INTERVAL_MS: u64 = 16;

/// The clock wraps every hour so the `f32` keeps sub-millisecond precision.
const WRAP_MS: u64 = 60 * 60 * 1000;

thread_local! {
    static FRAME_SECONDS: Cell<f32> = const { Cell::new(0.0) };
    // Repaint interval the frame's animated content asked for; 0 = none.
    static ANIMATION_INTERVAL_MS: Cell<u64> = const { Cell::new(0) };
}

/// Start a canvas frame: set the shader clock from the host clock and clear
/// the animation interval the previous frame left behind.
pub fn begin_frame(now_ms: u64) {
    FRAME_SECONDS.with(|seconds| seconds.set((now_ms % WRAP_MS) as f32 / 1000.0));
    ANIMATION_INTERVAL_MS.with(|interval| interval.set(0));
}

/// Whether `sksl` declares the time uniform as a scalar
/// (`uniform float time;` / `uniform half time;`, any whitespace).
pub fn declares_time_uniform(sksl: &str) -> bool {
    declares_uniform(sksl, &["float", "half"], TIME_UNIFORM)
}

/// Whether `sksl` declares `uniform <type> <name>;` for one of `types`
/// (any whitespace). Shared by the uniforms the canvas binds itself.
/// Comments are skipped, as for shader image children.
pub(crate) fn declares_uniform(sksl: &str, types: &[&str], name: &str) -> bool {
    let mut previous: [&str; 2] = ["", ""];
    for token in de_editor_core::shader_images::sksl_tokens(sksl) {
        if previous[0] == "uniform" && types.contains(&previous[1]) && token == name {
            return true;
        }
        previous = [previous[1], token];
    }
    false
}

/// The value to bind to [`TIME_UNIFORM`] for `sksl`, or `None` when the
/// shader does not declare it. A declaring shader marks the current frame as
/// animated so the host schedules the next one.
pub fn time_uniform_value(sksl: &str) -> Option<f32> {
    if !declares_time_uniform(sksl) {
        return None;
    }
    mark_animated(FRAME_INTERVAL_MS);
    Some(FRAME_SECONDS.with(Cell::get))
}

/// Run `paint` with the clock pinned to `seconds`, then restore the canvas
/// frame's clock and animation interval. Exports and screenshots can run on the
/// UI thread between canvas frames; pinning keeps their pixels independent
/// of when they were taken and keeps them from touching the canvas loop.
pub fn with_fixed_clock<R>(seconds: f32, paint: impl FnOnce() -> R) -> R {
    let saved_seconds = FRAME_SECONDS.with(|clock| clock.replace(seconds));
    let saved_interval = ANIMATION_INTERVAL_MS.with(Cell::get);
    let result = paint();
    FRAME_SECONDS.with(|clock| clock.set(saved_seconds));
    ANIMATION_INTERVAL_MS.with(|interval| interval.set(saved_interval));
    result
}

/// Mark the current frame as animated: something painted in it changes
/// without input and wants the next frame within `interval_ms`.
pub(crate) fn mark_animated(interval_ms: u64) {
    let interval_ms = interval_ms.max(1);
    ANIMATION_INTERVAL_MS.with(|interval| {
        let current = interval.get();
        interval.set(if current == 0 {
            interval_ms
        } else {
            current.min(interval_ms)
        });
    });
}

/// Whether animated content (a time-driven shader, a placeholder sweep) was
/// painted since the last [`begin_frame`].
pub fn animated_shader_painted() -> bool {
    ANIMATION_INTERVAL_MS.with(Cell::get) != 0
}

/// The next repaint the animated content on screen needs, if any.
pub fn next_deadline_ms(now_ms: u64) -> Option<u64> {
    let interval = ANIMATION_INTERVAL_MS.with(Cell::get);
    (interval != 0).then(|| now_ms.saturating_add(interval))
}

#[cfg(test)]
#[path = "canvas_shader_clock_tests.rs"]
mod tests;
