//! Vector edit mode (#387) seen from the canvas: the edited path's
//! subpaths mapped into scene coordinates, which handles show, hit
//! testing, marquee / lasso membership and snap candidates.
//!
//! Shared by the overlay painter and the host's input arms, so paint
//! and hit-test agree on one mapping (the same rule as
//! `host_canvas_geometry`). Core keeps the geometry in the path's
//! parent-local frame; the scene frame is that plus a translation read
//! off the resolved scene node.

use crate::layout_scene::{LayoutScene, SceneNode};
use crate::Point2D;
use de_editor_core::pen::PathHandleSide;
use de_editor_core::pen_node_ext::PenNodeExt;
use de_editor_core::vector_edit::{
    nearest_on_segment, segment_controls, segment_count, PenContinue, PointRef, Pt, Subpath,
};
use de_editor_core::EditorState;
use std::collections::BTreeSet;

/// Grab radius for points and handles, screen px.
pub const POINT_HIT_PX: f32 = 8.0;
/// Grab distance for a segment, screen px.
pub const SEGMENT_HIT_PX: f32 = 5.0;
/// Snap distance, screen px.
pub const SNAP_PX: f32 = 6.0;
/// Upper bound on scene nodes visited for snap candidates.
const MAX_SNAP_NODES: usize = 4000;

/// What a press in the mode landed on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VectorHit {
    Handle(PointRef, PathHandleSide),
    Point(PointRef),
    Segment { sub: usize, seg: usize, t: f64 },
}

/// The edited path in scene coordinates.
#[derive(Debug, Clone, PartialEq)]
pub struct VectorEditView {
    pub node_id: String,
    /// Parent-local geometry (core's frame).
    pub subs: Vec<Subpath>,
    /// `scene = local + off`.
    pub off: Pt,
    /// The node's own rotation and its pivot (scene), for un-rotating
    /// the cursor the same way `path_anchor_hit` does.
    pub rotation: f32,
    pub pivot: Point2D,
    pub selected: BTreeSet<PointRef>,
    pub pen_continue: Option<PenContinue>,
}

impl VectorEditView {
    /// The live session's view, or `None` outside the mode.
    pub fn build(state: &EditorState, scene: &LayoutScene) -> Option<Self> {
        let session = state.vector_edit()?;
        let subs = state.vector_subpaths()?;
        let doc_node =
            de_editor_core::walkers::find_node(state.active_children(), &session.node_id)?;
        let scene_node = scene.active_page()?.find(session.node_id.as_str())?;
        let base = (
            doc_node.base().x.unwrap_or(0.0),
            doc_node.base().y.unwrap_or(0.0),
        );
        let off = (
            scene_node.bounds.origin.x as f64 - base.0,
            scene_node.bounds.origin.y as f64 - base.1,
        );
        let b = scene_node.aggregate_bounds();
        Some(Self {
            node_id: session.node_id.as_str().to_string(),
            subs,
            off,
            rotation: scene_node.rotation,
            pivot: Point2D::new(b.origin.x + b.size.x / 2.0, b.origin.y + b.size.y / 2.0),
            selected: session.selected.clone(),
            pen_continue: session.pen_continue,
        })
    }

    pub fn to_scene(&self, p: Pt) -> Point2D {
        Point2D::new((p.0 + self.off.0) as f32, (p.1 + self.off.1) as f32)
    }

    pub fn to_local(&self, p: Point2D) -> Pt {
        (p.x as f64 - self.off.0, p.y as f64 - self.off.1)
    }

    /// Scene point → the path's unrotated frame (still scene units).
    pub fn unrotate(&self, doc: Point2D) -> Point2D {
        if self.rotation.abs() > f32::EPSILON {
            crate::widgets::rotate_point(doc, self.pivot, -self.rotation)
        } else {
            doc
        }
    }

    pub fn point(&self, p: PointRef) -> Option<Pt> {
        let a = self.subs.get(p.sub)?.anchors.get(p.idx)?;
        Some((a.x, a.y))
    }

    /// Absolute (local) position of a handle, when set.
    pub fn handle(&self, p: PointRef, side: PathHandleSide) -> Option<Pt> {
        let a = self.subs.get(p.sub)?.anchors.get(p.idx)?;
        let h = match side {
            PathHandleSide::In => a.handle_in.as_ref()?,
            PathHandleSide::Out => a.handle_out.as_ref()?,
        };
        Some((a.x + h.x, a.y + h.y))
    }

    /// Handles on show: both of every selected point, plus the handle of
    /// each neighbour that faces it (the two segments it shapes).
    pub fn visible_handles(&self) -> Vec<(PointRef, PathHandleSide)> {
        let mut out = BTreeSet::new();
        for p in &self.selected {
            let Some(sub) = self.subs.get(p.sub) else {
                continue;
            };
            let n = sub.anchors.len();
            if p.idx >= n {
                continue;
            }
            out.insert((*p, 0u8));
            out.insert((*p, 1u8));
            if p.idx > 0 || sub.closed {
                out.insert((PointRef::new(p.sub, (p.idx + n - 1) % n), 1u8));
            }
            if p.idx + 1 < n || sub.closed {
                out.insert((PointRef::new(p.sub, (p.idx + 1) % n), 0u8));
            }
        }
        out.into_iter()
            .map(|(p, side)| {
                let side = if side == 0 {
                    PathHandleSide::In
                } else {
                    PathHandleSide::Out
                };
                (p, side)
            })
            .filter(|(p, side)| self.handle(*p, *side).is_some())
            .collect()
    }

    /// Hit test an UNROTATED scene point at `zoom`: handles, then
    /// points, then segments.
    pub fn hit(&self, doc: Point2D, zoom: f32) -> Option<VectorHit> {
        let zoom = zoom.max(0.0001) as f64;
        let local = self.to_local(doc);
        let r = POINT_HIT_PX as f64 / zoom;
        let dist = |a: Pt| (a.0 - local.0).hypot(a.1 - local.1);
        let mut best: Option<(f64, VectorHit)> = None;
        for (p, side) in self.visible_handles() {
            if let Some(h) = self.handle(p, side) {
                let d = dist(h);
                if d <= r && best.is_none_or(|(bd, _)| d < bd) {
                    best = Some((d, VectorHit::Handle(p, side)));
                }
            }
        }
        if best.is_some() {
            return best.map(|(_, h)| h);
        }
        for (si, sub) in self.subs.iter().enumerate() {
            for (i, a) in sub.anchors.iter().enumerate() {
                let d = dist((a.x, a.y));
                if d <= r && best.is_none_or(|(bd, _)| d < bd) {
                    best = Some((d, VectorHit::Point(PointRef::new(si, i))));
                }
            }
        }
        if best.is_some() {
            return best.map(|(_, h)| h);
        }
        let seg_r = SEGMENT_HIT_PX as f64 / zoom;
        for (si, sub) in self.subs.iter().enumerate() {
            for seg in 0..segment_count(sub) {
                let Some(c) = segment_controls(sub, seg) else {
                    continue;
                };
                if !near_box(c, local, seg_r) {
                    continue;
                }
                let (t, d) = nearest_on_segment(c, local);
                if d <= seg_r && best.is_none_or(|(bd, _)| d < bd) {
                    best = Some((d, VectorHit::Segment { sub: si, seg, t }));
                }
            }
        }
        best.map(|(_, h)| h)
    }

    /// Points inside a closed scene polygon (marquee corners or lasso).
    pub fn points_in_polygon(&self, poly: &[Point2D]) -> Vec<PointRef> {
        if poly.len() < 3 {
            return Vec::new();
        }
        let mut out = Vec::new();
        for (si, sub) in self.subs.iter().enumerate() {
            for (i, a) in sub.anchors.iter().enumerate() {
                if point_in_polygon(self.to_scene((a.x, a.y)), poly) {
                    out.push(PointRef::new(si, i));
                }
            }
        }
        out
    }

    /// Snap candidates in scene coordinates: this path's points except
    /// `exclude`, plus corners, edge midpoints and centres of the other
    /// visible nodes of the page.
    pub fn snap_candidates(&self, scene: &LayoutScene, exclude: &BTreeSet<PointRef>) -> Vec<Pt> {
        let mut out = Vec::new();
        for (si, sub) in self.subs.iter().enumerate() {
            for (i, a) in sub.anchors.iter().enumerate() {
                if !exclude.contains(&PointRef::new(si, i)) {
                    out.push((a.x + self.off.0, a.y + self.off.1));
                }
            }
        }
        if let Some(page) = scene.active_page() {
            let mut budget = MAX_SNAP_NODES;
            for node in &page.children {
                collect_node_points(node, &self.node_id, &mut out, &mut budget);
            }
        }
        out
    }
}

fn near_box(c: [Pt; 4], p: Pt, r: f64) -> bool {
    let xs = [c[0].0, c[1].0, c[2].0, c[3].0];
    let ys = [c[0].1, c[1].1, c[2].1, c[3].1];
    let min_x = xs.iter().copied().fold(f64::INFINITY, f64::min) - r;
    let max_x = xs.iter().copied().fold(f64::NEG_INFINITY, f64::max) + r;
    let min_y = ys.iter().copied().fold(f64::INFINITY, f64::min) - r;
    let max_y = ys.iter().copied().fold(f64::NEG_INFINITY, f64::max) + r;
    p.0 >= min_x && p.0 <= max_x && p.1 >= min_y && p.1 <= max_y
}

fn collect_node_points(node: &SceneNode, skip: &str, out: &mut Vec<Pt>, budget: &mut usize) {
    if *budget == 0 || node.hidden {
        return;
    }
    *budget -= 1;
    if node.id != skip {
        let b = node.bounds;
        if b.size.x > 0.0 || b.size.y > 0.0 {
            let (x0, y0) = (b.origin.x as f64, b.origin.y as f64);
            let (x1, y1) = (x0 + b.size.x as f64, y0 + b.size.y as f64);
            let (mx, my) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
            out.extend([
                (x0, y0),
                (x1, y0),
                (x0, y1),
                (x1, y1),
                (mx, y0),
                (mx, y1),
                (x0, my),
                (x1, my),
                (mx, my),
            ]);
        }
    }
    for child in &node.children {
        collect_node_points(child, skip, out, budget);
    }
}

/// Even-odd ray cast.
pub fn point_in_polygon(p: Point2D, poly: &[Point2D]) -> bool {
    let mut inside = false;
    let mut j = poly.len() - 1;
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[j]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
        j = i;
    }
    inside
}
