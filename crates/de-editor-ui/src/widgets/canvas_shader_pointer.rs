//! Canvas pointer for pointer-reactive shader fills.
//!
//! A shader fill whose SkSL declares `uniform float2 mouse;` (or `half2`)
//! receives the pointer in the node's local pixels: the same space as the
//! `fragCoord` argument of `main`, so `distance(p, mouse)` is the distance
//! from the fragment to the pointer. Rotation, flips and the `size`
//! normalisation the backends apply to `fragCoord` apply to `mouse` too.
//!
//! When the pointer is not over the canvas (over the rails, outside the
//! window, Home or Play mode, or in an export / screenshot / thumbnail) the
//! shader gets [`NO_POINTER`], a point far outside any node, so hover effects
//! rest in their idle state.
//!
//! The canvas viewport sets the screen-space pointer once per painted frame
//! ([`begin_frame`]); the node walker maps it through each node transform it
//! pushes ([`enter_node_transform`]). Painting such a shader inside a canvas
//! frame marks the frame, and hosts call [`track_pointer`] on every cursor
//! move so a moving pointer repaints only while one is on screen.

use std::cell::{Cell, RefCell};

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
    /// Inside a canvas frame. Only canvas paints mark [`POINTER_PAINTED`]:
    /// preview, thumbnails and exports never see the pointer, so they must
    /// not make the host repaint on pointer moves.
    static IN_FRAME: Cell<bool> = const { Cell::new(false) };
    /// The node transforms pushed around the current paint, outermost
    /// first, so a painted box maps back to canvas space
    /// ([`map_to_canvas`], used by `canvas_animation_regions`).
    static TRANSFORMS: RefCell<Vec<NodeTransform>> = const { RefCell::new(Vec::new()) };
}

/// Start a canvas frame with the pointer in screen space (`None` when it is
/// off the canvas). A full frame clears the flag the previous frame left
/// behind; a partial repaint (`accumulate`, e.g. the pan cache's exposed
/// strips) keeps it, so one strip without the shader cannot hide another
/// strip's.
pub fn begin_frame(pointer: Option<Point2D>, accumulate: bool) {
    LOCAL_POINTER.with(|local| local.set(pointer.filter(|p| p.is_finite())));
    IN_FRAME.with(|in_frame| in_frame.set(true));
    TRANSFORMS.with(|stack| stack.borrow_mut().clear());
    if !accumulate {
        POINTER_PAINTED.with(|painted| painted.set(false));
    }
}

/// End a canvas frame: later paints (thumbnails, preview, exports) see no
/// pointer and do not mark the frame. The painted flag stays for the host.
pub fn end_frame() {
    LOCAL_POINTER.with(|local| local.set(None));
    IN_FRAME.with(|in_frame| in_frame.set(false));
    TRANSFORMS.with(|stack| stack.borrow_mut().clear());
}

/// Whether a canvas frame is painting (not an export, thumbnail or preview).
pub(crate) fn in_canvas_frame() -> bool {
    IN_FRAME.with(Cell::get)
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
    let saved_in_frame = IN_FRAME.with(|in_frame| in_frame.replace(false));
    let result = paint();
    LOCAL_POINTER.with(|local| local.set(saved_pointer));
    POINTER_PAINTED.with(|painted| painted.set(saved_painted));
    IN_FRAME.with(|in_frame| in_frame.set(saved_in_frame));
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
    TRANSFORMS.with(|stack| stack.borrow_mut().push(transform));
    LOCAL_POINTER.with(|local| {
        let outer = local.get();
        local.set(outer.and_then(|point| inverse_node_transform(point, transform)));
        outer
    })
}

/// Restore the pointer [`enter_node_transform`] returned.
pub(crate) fn leave_node_transform(outer: Option<Point2D>) {
    TRANSFORMS.with(|stack| stack.borrow_mut().pop());
    LOCAL_POINTER.with(|local| local.set(outer));
}

/// Bounding box, in canvas paint space, of `rect` painted under the node
/// transforms currently pushed. `None` when a transform is degenerate or
/// the result is not finite.
pub(crate) fn map_to_canvas(rect: Rect) -> Option<Rect> {
    TRANSFORMS.with(|stack| {
        let stack = stack.borrow();
        if stack.is_empty() {
            return (rect.origin.is_finite() && rect.size.is_finite()).then_some(rect);
        }
        let corners = [
            rect.origin,
            Point2D::new(rect.origin.x + rect.size.x, rect.origin.y),
            Point2D::new(rect.origin.x, rect.origin.y + rect.size.y),
            rect.origin + rect.size,
        ];
        let mut min = Point2D::new(f32::INFINITY, f32::INFINITY);
        let mut max = Point2D::new(f32::NEG_INFINITY, f32::NEG_INFINITY);
        for corner in corners {
            let mut point = corner;
            for transform in stack.iter().rev() {
                point = forward_node_transform(point, *transform);
            }
            if !point.is_finite() {
                return None;
            }
            min = min.min(point);
            max = max.max(point);
        }
        Some(Rect {
            origin: min,
            size: max - min,
        })
    })
}

/// Where a point drawn at `point` inside the node's transform lands:
/// `pivot + S_pop · S_flip · R · (point - pivot)` (the inverse of
/// [`inverse_node_transform`]).
fn forward_node_transform(point: Point2D, t: NodeTransform) -> Point2D {
    let mut d = point - t.pivot;
    if t.rotation.abs() > f32::EPSILON {
        let (sin, cos) = t.rotation.sin_cos();
        d = Point2D::new(d.x * cos - d.y * sin, d.x * sin + d.y * cos);
    }
    if t.flip_x {
        d.x = -d.x;
    }
    if t.flip_y {
        d.y = -d.y;
    }
    if let Some(scale) = t.pop_scale {
        d *= scale;
    }
    t.pivot + d
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
    super::canvas_shader_clock::declares_uniform(sksl, &["float2", "half2"], MOUSE_UNIFORM)
}

/// The value to bind to [`MOUSE_UNIFORM`] for a shader painted into `rect`,
/// or `None` when `sksl` does not declare it. `uniforms` are the bindings the
/// backend receives: the first 2-component `size` rescales the box to
/// document pixels, the same rule as native `runtime_shader_local_matrix`
/// (`de-host-native` skia `gradient.rs`) and the web
/// `opCkRuntimeEffectLocalMatrix` (`op_ck_bridge.js`); keep the three in
/// sync. Inside a canvas frame a declaring shader marks the frame so the host
/// repaints on pointer moves.
pub fn mouse_uniform_value(
    sksl: &str,
    rect: Rect,
    uniforms: &[(&str, &[f32])],
) -> Option<[f32; 2]> {
    if !super::canvas_shader_decls::shader_decls(sksl).mouse {
        return None;
    }
    if IN_FRAME.with(Cell::get) {
        POINTER_PAINTED.with(|painted| painted.set(true));
    }
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
