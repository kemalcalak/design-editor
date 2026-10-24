//! Canvas pointer for pointer-reactive shader fills.
//!
//! A shader fill whose SkSL declares `uniform float2 mouse;` (or `half2`)
//! receives the pointer in the node's local pixels: the same space as the
//! `fragCoord` argument of `main`, so `distance(p, mouse)` is the distance
//! from the fragment to the pointer. Rotation, flips and the `size`
//! normalisation the backends apply to `fragCoord` apply to `mouse` too.
//!
//! When the pointer is not over the canvas (over chrome, outside the
//! window, or in an export / screenshot / thumbnail) the shader gets
//! [`NO_POINTER`], a point far outside any node, so hover effects rest in
//! their idle state.
//!
//! The canvas viewport sets the screen-space pointer once per painted frame
//! ([`begin_frame`]); the node walker maps it through each node transform it
//! pushes ([`enter_node_transform`]). Painting such a shader marks the frame,
//! and hosts call [`track_pointer`] on every cursor move so a moving pointer
//! repaints only while one is on screen.

use std::cell::Cell;

use crate::{Point2D, Rect};

/// Uniform name the canvas binds to the node-local pointer.
pub const MOUSE_UNIFORM: &str = "mouse";

/// Value bound to [`MOUSE_UNIFORM`] while there is no pointer on the canvas.
/// Far enough out that a distance-based hover effect reads as idle, small
/// enough to stay finite in `half` precision.
pub const NO_POINTER: [f32; 2] = [-10_000.0, -10_000.0];

thread_local! {
    /// The pointer in the paint pass's current coordinate space, or `None`
    /// outside a canvas frame / when the pointer is off the canvas.
    static LOCAL_POINTER: Cell<Option<Point2D>> = const { Cell::new(None) };
    static POINTER_PAINTED: Cell<bool> = const { Cell::new(false) };
}

/// Start a canvas frame with the pointer in screen space (`None` when it is
/// off the canvas) and clear the flag the previous frame left behind.
pub fn begin_frame(pointer: Option<Point2D>) {
    LOCAL_POINTER.with(|local| local.set(pointer.filter(|p| p.is_finite())));
    POINTER_PAINTED.with(|painted| painted.set(false));
}

/// End a canvas frame: paints outside the canvas (thumbnails, exports) see
/// no pointer. The painted flag stays for the host to read.
pub fn end_frame() {
    LOCAL_POINTER.with(|local| local.set(None));
}

/// Whether a pointer-reading shader was painted since the last
/// [`begin_frame`].
pub fn pointer_shader_painted() -> bool {
    POINTER_PAINTED.with(Cell::get)
}

/// Record the canvas pointer a host just saw (`None` = not over the canvas).
/// Returns `true` when it changed while a pointer-reading shader is on
/// screen, i.e. when the host owes a repaint.
pub fn track_pointer(stored: &mut Option<Point2D>, next: Option<Point2D>) -> bool {
    if *stored == next {
        return false;
    }
    *stored = next;
    pointer_shader_painted()
}

/// Run `paint` with the pointer pinned to `pointer`, then restore the canvas
/// frame's pointer and painted flag. Exports and screenshots pin `None` so
/// their pixels never depend on where the pointer was.
pub fn with_fixed_pointer<R>(pointer: Option<Point2D>, paint: impl FnOnce() -> R) -> R {
    let saved_pointer = LOCAL_POINTER.with(|local| local.replace(pointer));
    let saved_painted = POINTER_PAINTED.with(Cell::get);
    let result = paint();
    LOCAL_POINTER.with(|local| local.set(saved_pointer));
    POINTER_PAINTED.with(|painted| painted.set(saved_painted));
    result
}

/// One node's paint transform, in the order `paint_node_inner` pushes it
/// onto the backend: reveal-pop scale, then the mirror, then the rotation,
/// all about `pivot`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct NodeTransform {
    pub(crate) pop_scale: Option<f32>,
    pub(crate) flip_x: bool,
    pub(crate) flip_y: bool,
    pub(crate) rotation: f32,
    pub(crate) pivot: Point2D,
}

/// Map the pointer into a node's coordinate space as its transform is
/// pushed. Returns the outer pointer for [`leave_node_transform`].
pub(crate) fn enter_node_transform(transform: NodeTransform) -> Option<Point2D> {
    LOCAL_POINTER.with(|local| {
        let outer = local.get();
        local.set(outer.and_then(|point| inverse_node_transform(point, transform)));
        outer
    })
}

/// Restore the pointer [`enter_node_transform`] returned.
pub(crate) fn leave_node_transform(outer: Option<Point2D>) {
    LOCAL_POINTER.with(|local| local.set(outer));
}

/// Skia pre-concatenates, so a point drawn at `p` lands at
/// `pivot + S_pop · S_flip · R · (p - pivot)`. Undo it right to left.
fn inverse_node_transform(point: Point2D, t: NodeTransform) -> Option<Point2D> {
    let mut d = point - t.pivot;
    if let Some(scale) = t.pop_scale {
        if scale.abs() <= f32::EPSILON {
            return None;
        }
        d /= scale;
    }
    if t.flip_x {
        d.x = -d.x;
    }
    if t.flip_y {
        d.y = -d.y;
    }
    if t.rotation.abs() > f32::EPSILON {
        let (sin, cos) = t.rotation.sin_cos();
        d = Point2D::new(d.x * cos + d.y * sin, -d.x * sin + d.y * cos);
    }
    let local = t.pivot + d;
    local.is_finite().then_some(local)
}

/// Whether `sksl` declares the mouse uniform as a 2-vector
/// (`uniform float2 mouse;` / `uniform half2 mouse;`, any whitespace).
pub fn declares_mouse_uniform(sksl: &str) -> bool {
    let mut previous: [&str; 2] = ["", ""];
    for token in sksl
        .split(|c: char| c.is_whitespace() || c == ';')
        .filter(|token| !token.is_empty())
    {
        if previous[0] == "uniform"
            && matches!(previous[1], "float2" | "half2")
            && token == MOUSE_UNIFORM
        {
            return true;
        }
        previous = [previous[1], token];
    }
    false
}

/// The value to bind to [`MOUSE_UNIFORM`] for a shader painted into `rect`,
/// or `None` when `sksl` does not declare it. `uniforms` are the bindings the
/// backend receives: like the backends, the first 2-component `size` rescales
/// the box to document pixels. A declaring shader marks the frame so the host
/// repaints on pointer moves.
pub fn mouse_uniform_value(
    sksl: &str,
    rect: Rect,
    uniforms: &[(&str, &[f32])],
) -> Option<[f32; 2]> {
    if !declares_mouse_uniform(sksl) {
        return None;
    }
    POINTER_PAINTED.with(|painted| painted.set(true));
    let Some(pointer) = LOCAL_POINTER.with(Cell::get) else {
        return Some(NO_POINTER);
    };
    let mut local = pointer - rect.origin;
    let size = uniforms
        .iter()
        .find(|(name, values)| *name == "size" && values.len() == 2)
        .map(|(_, values)| Point2D::new(values[0], values[1]));
    if let Some(size) = size {
        if size.is_finite() && size.x > 0.0 && size.y > 0.0 {
            if rect.size.x.abs() <= f32::EPSILON || rect.size.y.abs() <= f32::EPSILON {
                return Some(NO_POINTER);
            }
            local = local * size / rect.size;
        }
    }
    Some(if local.is_finite() {
        [local.x, local.y]
    } else {
        NO_POINTER
    })
}

#[cfg(test)]
#[path = "canvas_shader_pointer_tests.rs"]
mod tests;
