//! `EditorState` operations of vector edit mode.
//!
//! History: one undo step per user action. Keyboard / inspector actions
//! go through [`EditorState::vector_commit`] (snapshot, edit, push only
//! when the geometry changed). Pointer gestures snapshot at press and
//! push on the first real move (host side, like the anchor drag); their
//! per-move writes use [`EditorState::vector_write`], which never pushes.

use super::geometry::{delete_points, join_endpoints};
use super::source::node_subpaths;
use super::write::write_subpaths;
use super::{PointRef, Pt, Subpath, VectorEditSession};
use crate::node_id::NodeId;
use crate::path_edit::{
    apply_mirrored_point_type, build_default_handles, infer_path_anchor_point_type,
};
use crate::pen::PathHandleSide;
use crate::state::EditorState;
use crate::walkers::{find_node, find_node_mut, remove_from_children};
use jian_ops_schema::node::{PenNode, PenPathHandle, PenPathPointType};
use std::collections::BTreeSet;

impl EditorState {
    /// The live session, when it is still valid: its node exists on the
    /// active page, is a path and is the selection anchor.
    pub fn vector_edit(&self) -> Option<&VectorEditSession> {
        let session = self.ui.vector_edit.as_ref()?;
        let valid = self.selection.anchor == session.node_id
            && matches!(
                find_node(self.active_children(), &session.node_id),
                Some(PenNode::Path(_))
            );
        valid.then_some(session)
    }

    /// True while a valid session runs.
    pub fn vector_edit_active(&self) -> bool {
        self.vector_edit().is_some()
    }

    /// Drop a session the document or selection moved away from (node
    /// deleted, page switched, another node selected). True when one
    /// was dropped.
    pub fn prune_vector_edit(&mut self) -> bool {
        if self.ui.vector_edit.is_some() && self.vector_edit().is_none() {
            self.ui.vector_edit = None;
            return true;
        }
        false
    }

    /// Whether `id` can enter vector edit mode: an editable path whose
    /// geometry parses.
    pub fn can_enter_vector_edit(&self, id: &NodeId) -> bool {
        self.is_editable(id)
            && find_node(self.active_children(), id).is_some_and(|n| node_subpaths(n).is_some())
    }

    /// Enter vector edit mode on `id`. The document is not touched — an
    /// imported `d` stays byte-identical until the first edit.
    pub fn enter_vector_edit(&mut self, id: &NodeId) -> bool {
        if !self.can_enter_vector_edit(id) {
            return false;
        }
        // An in-flight pen session would fight over the same pointer.
        let _ = self.finish_pen_path();
        self.set_single_selection(id.clone());
        self.ui.vector_edit = Some(VectorEditSession::new(id.clone()));
        if !matches!(self.tool, crate::tool::Tool::Pen) {
            self.tool = crate::tool::Tool::Select;
        }
        true
    }

    /// Leave vector edit mode. Lone pen anchors (a subpath of one point,
    /// invisible) are dropped on the way out, inside the undo step that
    /// placed them. True when a session was active.
    pub fn exit_vector_edit(&mut self) -> bool {
        if self.vector_edit().is_none() {
            return self.ui.vector_edit.take().is_some();
        }
        if let Some(subs) = self.vector_subpaths() {
            if subs.iter().any(|s| s.anchors.len() < 2) {
                let kept: Vec<Subpath> =
                    subs.into_iter().filter(|s| s.anchors.len() >= 2).collect();
                self.vector_write(&kept);
            }
        }
        self.ui.vector_edit = None;
        true
    }

    /// The edited path's subpaths in its parent-local frame.
    pub fn vector_subpaths(&self) -> Option<Vec<Subpath>> {
        let session = self.vector_edit()?;
        node_subpaths(find_node(self.active_children(), &session.node_id)?)
    }

    /// Write `subs` to the edited path without touching history. An
    /// empty list deletes the path and ends the session.
    pub fn vector_write(&mut self, subs: &[Subpath]) -> bool {
        let Some(id) = self.vector_edit().map(|s| s.node_id.clone()) else {
            return false;
        };
        if !self.is_editable(&id) {
            return false;
        }
        if subs.iter().all(|s| s.anchors.is_empty()) {
            remove_from_children(self.active_children_mut(), &id);
            self.ui.vector_edit = None;
            self.clear_selection();
            self.mark_document_changed();
            return true;
        }
        let Some(node) = find_node_mut(self.active_children_mut(), &id) else {
            return false;
        };
        let wrote = write_subpaths(node, subs);
        if wrote {
            self.mark_document_changed();
        }
        wrote
    }

    /// Run one undoable action: `f` edits a copy of the geometry and the
    /// session; the result is written and ONE history entry pushed only
    /// when the geometry changed. Returns `f`'s verdict.
    pub fn vector_commit(
        &mut self,
        f: impl FnOnce(&mut Vec<Subpath>, &mut VectorEditSession) -> bool,
    ) -> bool {
        self.vector_apply(true, f)
    }

    /// [`Self::vector_commit`] with history optional: `record = false`
    /// is for callers that own the undo step (the inspector's commit).
    fn vector_apply(
        &mut self,
        record: bool,
        f: impl FnOnce(&mut Vec<Subpath>, &mut VectorEditSession) -> bool,
    ) -> bool {
        let (Some(mut subs), Some(mut session)) =
            (self.vector_subpaths(), self.vector_edit().cloned())
        else {
            return false;
        };
        let before = subs.clone();
        let ok = f(&mut subs, &mut session);
        self.ui.vector_edit = Some(session);
        if ok && subs != before {
            let snap = record.then(|| self.snapshot_for_history());
            if self.vector_write(&subs) {
                if let Some(snap) = snap {
                    self.history_push_past(snap);
                }
            }
        }
        ok
    }

    /// Select `p` (Shift = toggle it in the current selection).
    pub fn vector_select(&mut self, p: PointRef, additive: bool) {
        self.vector_select_many([p], additive);
    }

    /// Replace (or, `additive`, toggle) the point selection.
    pub fn vector_select_many(&mut self, pts: impl IntoIterator<Item = PointRef>, additive: bool) {
        let Some(session) = self.ui.vector_edit.as_mut() else {
            return;
        };
        if !additive {
            session.selected.clear();
        }
        for p in pts {
            if additive && session.selected.contains(&p) {
                session.selected.remove(&p);
            } else {
                session.selected.insert(p);
            }
        }
    }

    /// Cmd+A inside the mode — every point. True when a session ran.
    pub fn vector_select_all(&mut self) -> bool {
        let Some(subs) = self.vector_subpaths() else {
            return false;
        };
        let all: Vec<PointRef> = subs
            .iter()
            .enumerate()
            .flat_map(|(s, sub)| (0..sub.anchors.len()).map(move |i| PointRef::new(s, i)))
            .collect();
        self.vector_select_many(all, false);
        true
    }

    /// Selected points that still exist in `subs`.
    fn valid_selection(subs: &[Subpath], selected: &BTreeSet<PointRef>) -> BTreeSet<PointRef> {
        selected
            .iter()
            .copied()
            .filter(|p| subs.get(p.sub).is_some_and(|s| p.idx < s.anchors.len()))
            .collect()
    }

    /// Move `pts` by `(dx, dy)` starting from the drag-start geometry
    /// `base` (so a drag never accumulates rounding). No history.
    pub fn vector_move_points_from(
        &mut self,
        base: &[Subpath],
        pts: &BTreeSet<PointRef>,
        dx: f64,
        dy: f64,
    ) -> bool {
        let mut subs = base.to_vec();
        for p in Self::valid_selection(&subs, pts) {
            let a = &mut subs[p.sub].anchors[p.idx];
            a.x += dx;
            a.y += dy;
        }
        self.vector_write(&subs)
    }

    /// Arrow-key nudge of the selected points — one undo step.
    pub fn vector_nudge(&mut self, dx: f64, dy: f64) -> bool {
        self.vector_commit(|subs, session| {
            let pts = Self::valid_selection(subs, &session.selected);
            for p in &pts {
                let a = &mut subs[p.sub].anchors[p.idx];
                a.x += dx;
                a.y += dy;
            }
            !pts.is_empty()
        })
    }

    /// Delete / Backspace — remove the selected points and heal the
    /// curve. Deleting every point deletes the path. One undo step.
    pub fn vector_delete_selected(&mut self) -> bool {
        self.vector_commit(|subs, session| {
            let pts = Self::valid_selection(subs, &session.selected);
            if pts.is_empty() {
                return false;
            }
            *subs = delete_points(subs, &pts);
            session.selected.clear();
            session.pen_continue = None;
            true
        })
    }

    /// Cmd+J — join the two selected open end points (one subpath's two
    /// ends close it). One undo step.
    pub fn vector_join_selected(&mut self) -> bool {
        self.vector_commit(|subs, session| {
            let pts: Vec<PointRef> = Self::valid_selection(subs, &session.selected)
                .into_iter()
                .collect();
            let [a, b] = pts.as_slice() else {
                return false;
            };
            if join_endpoints(subs, *a, *b, false).is_none() {
                return false;
            }
            session.selected.clear();
            session.pen_continue = None;
            true
        })
    }

    /// Alt-click / double-click a point: corner ↔ smooth (mirrored
    /// tangent handles). One undo step.
    pub fn vector_toggle_smooth(&mut self, p: PointRef) -> bool {
        self.vector_commit(|subs, _| {
            let Some(sub) = subs.get_mut(p.sub) else {
                return false;
            };
            let Some(anchor) = sub.anchors.get(p.idx) else {
                return false;
            };
            let next = match infer_path_anchor_point_type(anchor) {
                PenPathPointType::Corner => PenPathPointType::Mirrored,
                _ => PenPathPointType::Corner,
            };
            set_point_type(sub, p.idx, next);
            true
        })
    }

    /// Inspector point-type control: set every selected point's type.
    /// History is the caller's (the property-panel dispatch).
    pub fn vector_set_point_type(&mut self, point_type: PenPathPointType) -> bool {
        self.vector_apply(false, |subs, session| {
            let pts = Self::valid_selection(subs, &session.selected);
            for p in &pts {
                set_point_type(&mut subs[p.sub], p.idx, point_type.clone());
            }
            !pts.is_empty()
        })
    }

    /// Shared point type of the selection (`None` when mixed / empty).
    pub fn vector_selection_point_type(&self) -> Option<PenPathPointType> {
        let subs = self.vector_subpaths()?;
        let session = self.vector_edit()?;
        let mut kinds = Self::valid_selection(&subs, &session.selected)
            .into_iter()
            .map(|p| {
                let a = &subs[p.sub].anchors[p.idx];
                a.point_type
                    .clone()
                    .unwrap_or_else(|| infer_path_anchor_point_type(a))
            });
        let first = kinds.next()?;
        kinds.all(|k| k == first).then_some(first)
    }

    /// Top-left of the selected points (parent-local) — the inspector's
    /// X / Y while points are selected.
    pub fn vector_selection_origin(&self) -> Option<Pt> {
        let subs = self.vector_subpaths()?;
        let session = self.vector_edit()?;
        let pts = Self::valid_selection(&subs, &session.selected);
        let mut it = pts.iter().map(|p| {
            let a = &subs[p.sub].anchors[p.idx];
            (a.x, a.y)
        });
        let first = it.next()?;
        Some(it.fold(first, |m, p| (m.0.min(p.0), m.1.min(p.1))))
    }

    /// Inspector X / Y edit: move the selected points so their top-left
    /// lands on `x` / `y` (each optional). History is the caller's (the
    /// property commit).
    pub fn vector_set_selection_origin(&mut self, x: Option<f64>, y: Option<f64>) -> bool {
        let Some(origin) = self.vector_selection_origin() else {
            return false;
        };
        let dx = x.map_or(0.0, |x| x - origin.0);
        let dy = y.map_or(0.0, |y| y - origin.1);
        self.vector_apply(false, |subs, session| {
            for p in Self::valid_selection(subs, &session.selected) {
                let a = &mut subs[p.sub].anchors[p.idx];
                a.x += dx;
                a.y += dy;
            }
            true
        })
    }

    /// Drag a handle of `p` to `offset` (anchor-relative), starting from
    /// the drag-start geometry `base`. A mirrored point keeps its
    /// opposite handle symmetric unless `break_symmetry` (Alt), which
    /// turns it independent. No history.
    pub fn vector_set_handle_from(
        &mut self,
        base: &[Subpath],
        p: PointRef,
        side: PathHandleSide,
        offset: Pt,
        break_symmetry: bool,
    ) -> bool {
        let mut subs = base.to_vec();
        let Some(a) = subs.get_mut(p.sub).and_then(|s| s.anchors.get_mut(p.idx)) else {
            return false;
        };
        let resolved = a
            .point_type
            .clone()
            .unwrap_or_else(|| infer_path_anchor_point_type(a));
        let h = Some(PenPathHandle {
            x: offset.0,
            y: offset.1,
        });
        let mirror = Some(PenPathHandle {
            x: -offset.0,
            y: -offset.1,
        });
        match side {
            PathHandleSide::In => a.handle_in = h,
            PathHandleSide::Out => a.handle_out = h,
        }
        if break_symmetry {
            a.point_type = Some(PenPathPointType::Independent);
        } else if resolved == PenPathPointType::Mirrored {
            match side {
                PathHandleSide::In => a.handle_out = mirror,
                PathHandleSide::Out => a.handle_in = mirror,
            }
            a.point_type = Some(PenPathPointType::Mirrored);
        }
        self.vector_write(&subs)
    }

    /// Insert a point on segment `seg` of subpath `sub` at `t`, keeping
    /// the curve (de Casteljau). The new point becomes the selection.
    /// One undo step.
    pub fn vector_insert_point(&mut self, sub: usize, seg: usize, t: f64) -> Option<PointRef> {
        let mut inserted = None;
        self.vector_commit(|subs, session| {
            let Some(s) = subs.get_mut(sub) else {
                return false;
            };
            let Some(at) = super::split_segment(s, seg, t) else {
                return false;
            };
            // Later points of this subpath shift by one.
            session.selected.clear();
            session.selected.insert(PointRef::new(sub, at));
            if let Some(pc) = session.pen_continue.as_mut() {
                if pc.sub == sub {
                    session.pen_continue = None;
                }
            }
            inserted = Some(PointRef::new(sub, at));
            true
        });
        inserted
    }
}

/// Apply a point type to `sub.anchors[idx]` with the same semantics as
/// the anchor context menu (`set_path_anchor_point_type_ts`).
fn set_point_type(sub: &mut Subpath, idx: usize, point_type: PenPathPointType) {
    let closed = sub.closed;
    match point_type {
        PenPathPointType::Corner => {
            let a = &mut sub.anchors[idx];
            a.handle_in = None;
            a.handle_out = None;
            a.point_type = Some(PenPathPointType::Corner);
        }
        PenPathPointType::Mirrored => {
            let defaults = build_default_handles(&sub.anchors, idx, closed);
            apply_mirrored_point_type(&mut sub.anchors[idx], defaults);
        }
        PenPathPointType::Independent => {
            let defaults = build_default_handles(&sub.anchors, idx, closed);
            let a = &mut sub.anchors[idx];
            if a.handle_in.is_none() {
                a.handle_in = defaults.0;
            }
            if a.handle_out.is_none() {
                a.handle_out = defaults.1;
            }
            a.point_type = Some(PenPathPointType::Independent);
        }
    }
}
