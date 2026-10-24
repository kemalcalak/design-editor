//! Streaming-reveal timing (wireframe ghost → placement pop) for the
//! canvas painter, split out of `canvas_viewport_paint.rs` to keep that
//! spine under the repository's 800-line cap.

use std::collections::HashMap;
/// Reveal timing for nodes that are being streamed onto the canvas.
#[derive(Clone, Copy)]
pub struct RevealSchedule<'a> {
    pub(crate) starts: &'a HashMap<String, u64>,
    pub(crate) now_ms: u64,
}

/// Scale-in pop window right after a node's reveal starts — the Stitch
/// placement "pop" that replaced the old dashed border fade.
pub(crate) const REVEAL_POP_MS: u64 = 180;

/// Wireframe-ghost window BEFORE the pop: a revealing node first paints as
/// a blue outlined box at its own rect (Pencil parity — its streamed
/// elements materialize as periwinkle wireframes sized like the coming
/// content, then resolve), and only then pops in for real.
pub(crate) const REVEAL_WIREFRAME_MS: u64 = 260;

/// Placement pop: 0.85 → ~1.02 overshoot → 1.0 across [`REVEAL_POP_MS`]
/// (ease-out-back). `None` once the pop has settled.
pub(crate) fn reveal_pop_scale(elapsed_ms: u64) -> Option<f32> {
    if elapsed_ms >= REVEAL_POP_MS {
        return None;
    }
    let t = elapsed_ms as f32 / REVEAL_POP_MS as f32;
    let s = 1.70158f32;
    let u = t - 1.0;
    let back = u * u * ((s + 1.0) * u + s) + 1.0;
    Some(0.85 + 0.15 * back)
}

#[derive(Clone, Copy)]
pub(super) enum RevealPaintState {
    Idle,
    Pending,
    Active { elapsed_ms: u64 },
}

pub(super) fn reveal_paint_state(schedule: RevealSchedule<'_>, node_id: &str) -> RevealPaintState {
    let Some(started_at) = schedule.starts.get(node_id) else {
        return RevealPaintState::Idle;
    };
    if schedule.now_ms < *started_at {
        return RevealPaintState::Pending;
    }
    let elapsed = schedule.now_ms.saturating_sub(*started_at);
    if elapsed > de_editor_core::agent_indicators::REVEAL_DURATION_MS {
        return RevealPaintState::Idle;
    }
    RevealPaintState::Active {
        elapsed_ms: elapsed,
    }
}

/// Record a revealing node that lies inside the viewport, so the host keeps
/// the 60 fps cadence only for reveals someone can see (#17). The box is the
/// node's untransformed rect; transforms only move it by a little.
pub(super) fn note_reveal_on_screen(
    state: RevealPaintState,
    node: &crate::layout_scene::SceneNode,
    origin: crate::Point2D,
    zoom: f32,
    cull: crate::Rect,
) {
    use crate::widgets::canvas_generation_visibility as visibility;
    if matches!(state, RevealPaintState::Idle) {
        return;
    }
    let rect = crate::Rect::xywh(
        origin.x + node.bounds.origin.x * zoom,
        origin.y + node.bounds.origin.y * zoom,
        node.bounds.size.x * zoom,
        node.bounds.size.y * zoom,
    );
    if !visibility::on_screen(rect, cull) {
        return;
    }
    match state {
        RevealPaintState::Pending => visibility::mark_pending(),
        _ => visibility::mark_animating(),
    }
}
