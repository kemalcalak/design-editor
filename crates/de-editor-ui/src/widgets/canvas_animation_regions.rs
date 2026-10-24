//! Where on the canvas a frame's self-animating content was painted.
//!
//! A time- or pointer-driven shader repaints the canvas every frame (see
//! [`super::canvas_shader_clock`] / [`super::canvas_shader_pointer`]). With
//! nothing else changing, only the pixels under those shaders differ from
//! one frame to the next, so a host that keeps the rest of the canvas in a
//! retained layer can repaint just these regions (#208: a document with 17
//! scene shaders repainted all ~2,100 nodes per frame, ~55 ms).
//!
//! While a canvas frame paints, every shader that binds `time` or `mouse`
//! records its box here, mapped through the node transforms around it into
//! the canvas paint space (screen coordinates for an on-window paint). The
//! answer is only trustworthy when every animated thing was recorded and no
//! painted node samples the pixels behind it, so the frame also notes:
//!
//! - animation without a box (the placeholder sweep, #16), and
//! - backdrop reads (background blur, backdrop shader passes, backdrop
//!   textures), whose output outside their own box could depend on an
//!   animated region.
//!
//! Either makes the frame [`AnimationRegions::located`] `false`, and the host
//! must repaint the whole canvas as before.
//!
//! Only paints inside a canvas frame record (exports, thumbnails and preview
//! never do), and [`begin_frame`] starts every canvas paint, partial ones
//! included, from an empty record: the host reads it after each paint.

use std::cell::{Cell, RefCell};

use crate::{Point2D, Rect};

thread_local! {
    static RECTS: RefCell<Vec<Rect>> = const { RefCell::new(Vec::new()) };
    static UNLOCATED: Cell<bool> = const { Cell::new(false) };
    static BACKDROP: Cell<bool> = const { Cell::new(false) };
}

/// What one canvas paint recorded.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AnimationRegions {
    /// Boxes of the animated shaders, in canvas paint space.
    pub rects: Vec<Rect>,
    /// Whether repainting only `rects` reproduces the next frame: no
    /// animation without a box, no backdrop reads.
    pub located: bool,
}

/// Start a canvas paint with an empty record.
pub fn begin_frame() {
    RECTS.with(|rects| rects.borrow_mut().clear());
    UNLOCATED.with(|flag| flag.set(false));
    BACKDROP.with(|flag| flag.set(false));
}

/// What the canvas paint since [`begin_frame`] recorded.
pub fn snapshot() -> AnimationRegions {
    AnimationRegions {
        rects: RECTS.with(|rects| rects.borrow().clone()),
        located: !UNLOCATED.with(Cell::get) && !BACKDROP.with(Cell::get),
    }
}

/// Record an animated shader painted into `rect` (current paint space).
pub(crate) fn record(rect: Rect) {
    if !super::canvas_shader_pointer::in_canvas_frame() {
        return;
    }
    let mapped = super::canvas_shader_pointer::map_to_canvas(rect);
    match mapped {
        Some(mapped) => RECTS.with(|rects| rects.borrow_mut().push(mapped)),
        None => mark_unlocated(),
    }
}

/// Note animated content whose box is not known.
pub(crate) fn mark_unlocated() {
    if super::canvas_shader_pointer::in_canvas_frame() {
        UNLOCATED.with(|flag| flag.set(true));
    }
}

/// Note a paint that samples the pixels behind it.
pub(crate) fn mark_backdrop() {
    if super::canvas_shader_pointer::in_canvas_frame() {
        BACKDROP.with(|flag| flag.set(true));
    }
}

/// Grow `rect` outward to whole device pixels at `dpi`, so a clip to it
/// covers every pixel it touches with full coverage.
pub fn snap_out(rect: Rect, dpi: f32) -> Rect {
    let dpi = if dpi.is_finite() && dpi > 0.0 { dpi } else { 1.0 };
    let x0 = (rect.origin.x * dpi).floor() / dpi;
    let y0 = (rect.origin.y * dpi).floor() / dpi;
    let x1 = ((rect.origin.x + rect.size.x) * dpi).ceil() / dpi;
    let y1 = ((rect.origin.y + rect.size.y) * dpi).ceil() / dpi;
    Rect {
        origin: Point2D::new(x0, y0),
        size: Point2D::new(x1 - x0, y1 - y0),
    }
}

/// Merge overlapping or touching rects, so one repaint pass covers each
/// cluster of nearby shaders.
pub fn merge_overlapping(mut rects: Vec<Rect>) -> Vec<Rect> {
    let touches = |a: &Rect, b: &Rect| {
        a.origin.x <= b.origin.x + b.size.x
            && b.origin.x <= a.origin.x + a.size.x
            && a.origin.y <= b.origin.y + b.size.y
            && b.origin.y <= a.origin.y + a.size.y
    };
    let union = |a: &Rect, b: &Rect| {
        let x0 = a.origin.x.min(b.origin.x);
        let y0 = a.origin.y.min(b.origin.y);
        let x1 = (a.origin.x + a.size.x).max(b.origin.x + b.size.x);
        let y1 = (a.origin.y + a.size.y).max(b.origin.y + b.size.y);
        Rect {
            origin: Point2D::new(x0, y0),
            size: Point2D::new(x1 - x0, y1 - y0),
        }
    };
    let mut merged = true;
    while merged {
        merged = false;
        'outer: for i in 0..rects.len() {
            for j in (i + 1)..rects.len() {
                if touches(&rects[i], &rects[j]) {
                    let joined = union(&rects[i], &rects[j]);
                    rects.swap_remove(j);
                    rects[i] = joined;
                    merged = true;
                    break 'outer;
                }
            }
        }
    }
    rects
}

#[cfg(test)]
#[path = "canvas_animation_regions_tests.rs"]
mod tests;
