//! Document → subpaths in the parent-local frame.

use super::{parse::parse_path_data, Subpath};
use crate::path_bounds::path_bounds_from_anchors;
use crate::pen_node_ext::PenNodeExt;
use jian_ops_schema::node::{PenNode, PenPathHandle};

/// Union of the handle-aware bounds of every subpath:
/// `(min_x, min_y, width, height)`. Zero box for no anchors.
pub fn subpaths_bounds(subs: &[Subpath]) -> (f64, f64, f64, f64) {
    let mut min = (f64::INFINITY, f64::INFINITY);
    let mut max = (f64::NEG_INFINITY, f64::NEG_INFINITY);
    for s in subs.iter().filter(|s| !s.anchors.is_empty()) {
        let (x, y, w, h) = path_bounds_from_anchors(&s.anchors, s.closed);
        min = (min.0.min(x), min.1.min(y));
        max = (max.0.max(x + w), max.1.max(y + h));
    }
    if !min.0.is_finite() {
        return (0.0, 0.0, 0.0, 0.0);
    }
    (min.0, min.1, max.0 - min.0, max.1 - min.1)
}

/// Stretch `subs` so their handle-aware bounds fill a `w × h` box at the
/// origin: how the scene builder fits a path's `d` to its node box before
/// rounding vertices (#389), so a radius stays in node px. An axis whose
/// span is under 0.01 keeps its scale.
pub fn fit_subpaths_to_box(subs: &mut [Subpath], w: f64, h: f64) {
    let (bx, by, bw, bh) = subpaths_bounds(subs);
    if !(bx.is_finite() && by.is_finite()) {
        return;
    }
    let sx = if bw.abs() > 0.01 { w / bw } else { 1.0 };
    let sy = if bh.abs() > 0.01 { h / bh } else { 1.0 };
    for anchor in subs.iter_mut().flat_map(|s| s.anchors.iter_mut()) {
        anchor.x = (anchor.x - bx) * sx;
        anchor.y = (anchor.y - by) * sy;
        for handle in [&mut anchor.handle_in, &mut anchor.handle_out]
            .into_iter()
            .flatten()
        {
            handle.x *= sx;
            handle.y *= sy;
        }
    }
}

/// The path's geometry as subpaths in its parent-local frame, or `None`
/// for a non-path / empty / unparseable node.
///
/// Editor-authored `anchors` win (one subpath); otherwise `d` is
/// parsed. The native geometry is then fitted into the node's box —
/// `base.x / base.y` + `width / height`, scale = box / native span per
/// axis — which is exactly what the canvas painter does with `d`
/// (`fit_path_to_rect`) and the loader does with anchors, so the points
/// land on the pixels the user sees.
pub fn node_subpaths(node: &PenNode) -> Option<Vec<Subpath>> {
    let PenNode::Path(path) = node else {
        return None;
    };
    let mut subs = match path.anchors.as_ref().filter(|a| !a.is_empty()) {
        Some(anchors) => vec![Subpath {
            anchors: anchors.clone(),
            closed: path.closed.unwrap_or(false),
        }],
        None => parse_path_data(path.d.as_deref()?)?,
    };
    let (nx, ny, nw, nh) = subpaths_bounds(&subs);
    let bx = node.base().x.unwrap_or(0.0);
    let by = node.base().y.unwrap_or(0.0);
    let scale = |native: f64, boxed: Option<f64>| match boxed {
        Some(b) if native > 0.01 && b > 0.0 => b / native,
        _ => 1.0,
    };
    let sx = scale(nw, node.width_px());
    let sy = scale(nh, node.height_px());
    let identity = (sx - 1.0).abs() < 1e-12
        && (sy - 1.0).abs() < 1e-12
        && (bx - nx).abs() < 1e-12
        && (by - ny).abs() < 1e-12;
    if !identity {
        let map_h = |h: &mut Option<PenPathHandle>| {
            if let Some(h) = h.as_mut() {
                h.x *= sx;
                h.y *= sy;
            }
        };
        for s in &mut subs {
            for a in &mut s.anchors {
                a.x = bx + (a.x - nx) * sx;
                a.y = by + (a.y - ny) * sy;
                map_h(&mut a.handle_in);
                map_h(&mut a.handle_out);
            }
        }
    }
    Some(subs)
}
