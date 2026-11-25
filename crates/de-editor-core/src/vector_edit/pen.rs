//! The Pen inside vector edit mode: continue an open path from an end
//! point, add points, close, and connect to another end point.

use super::geometry::join_endpoints;
use super::{corner, PenContinue, PointRef, Pt, Subpath};
use crate::node_id::NodeId;
use crate::state::EditorState;
use jian_ops_schema::node::{PenPathHandle, PenPathPointType};

/// Press-drag distance (doc px) before a just-placed point grows
/// handles — the same threshold as the authoring pen.
const HANDLE_DRAG_MIN: f64 = 2.0;

impl EditorState {
    /// Pen press on an open end point: keep drawing from it. No
    /// document change. False when `p` is not an open end point.
    pub fn vector_pen_continue_from(&mut self, p: PointRef) -> bool {
        let Some(subs) = self.vector_subpaths() else {
            return false;
        };
        let Some(sub) = subs.get(p.sub).filter(|s| s.is_endpoint(p.idx)) else {
            return false;
        };
        let at_start = p.idx == 0 && sub.anchors.len() > 1;
        let Some(session) = self.ui.vector_edit.as_mut() else {
            return false;
        };
        session.pen_continue = Some(PenContinue {
            sub: p.sub,
            at_start,
        });
        session.selected.clear();
        session.selected.insert(p);
        true
    }

    /// Pen tool on the open end point `p` of the selected path, outside
    /// the mode: enter it and continue from that end.
    pub fn continue_path_with_pen(&mut self, id: &NodeId, p: PointRef) -> bool {
        self.enter_vector_edit(id) && self.vector_pen_continue_from(p)
    }

    /// The point the pen's rubber band starts from.
    pub fn vector_pen_anchor(&self) -> Option<Pt> {
        let pc = self.vector_edit()?.pen_continue?;
        let subs = self.vector_subpaths()?;
        let sub = subs.get(pc.sub)?;
        let a = if pc.at_start {
            sub.anchors.first()?
        } else {
            sub.anchors.last()?
        };
        Some((a.x, a.y))
    }

    /// Pen press on empty canvas inside the mode: append a corner point
    /// to the continued subpath, or start a new subpath. One undo step.
    pub fn vector_pen_add_point(&mut self, at: Pt) -> Option<PointRef> {
        let mut added = None;
        self.vector_commit(|subs, session| {
            let target = match session.pen_continue {
                Some(pc) if subs.get(pc.sub).is_some_and(|s| !s.closed) => pc,
                _ => {
                    subs.push(Subpath {
                        anchors: Vec::new(),
                        closed: false,
                    });
                    PenContinue {
                        sub: subs.len() - 1,
                        at_start: false,
                    }
                }
            };
            let sub = &mut subs[target.sub];
            let idx = if target.at_start {
                sub.anchors.insert(0, corner(at));
                0
            } else {
                sub.anchors.push(corner(at));
                sub.anchors.len() - 1
            };
            session.pen_continue = Some(target);
            session.selected.clear();
            let p = PointRef::new(target.sub, idx);
            session.selected.insert(p);
            added = Some(p);
            true
        });
        added
    }

    /// Pen press on the far end of the continued subpath: close it.
    /// One undo step.
    pub fn vector_pen_close(&mut self) -> bool {
        self.vector_commit(|subs, session| {
            let Some(pc) = session.pen_continue else {
                return false;
            };
            let Some(sub) = subs.get_mut(pc.sub).filter(|s| s.anchors.len() > 2) else {
                return false;
            };
            let last = sub.anchors.len() - 1;
            let (a, b) = if pc.at_start { (0, last) } else { (last, 0) };
            if join_endpoints(
                subs,
                PointRef::new(pc.sub, a),
                PointRef::new(pc.sub, b),
                false,
            )
            .is_none()
            {
                return false;
            }
            session.pen_continue = None;
            session.selected.clear();
            true
        })
    }

    /// Pen press on another open end point: connect the continued end
    /// to it with a straight segment. One undo step.
    pub fn vector_pen_connect(&mut self, target: PointRef) -> bool {
        self.vector_commit(|subs, session| {
            let Some(pc) = session.pen_continue else {
                return false;
            };
            let Some(sub) = subs.get(pc.sub) else {
                return false;
            };
            let from = if pc.at_start {
                0
            } else {
                sub.anchors.len().saturating_sub(1)
            };
            if join_endpoints(subs, PointRef::new(pc.sub, from), target, false).is_none() {
                return false;
            }
            session.pen_continue = None;
            session.selected.clear();
            true
        })
    }

    /// Cursor move while the pen press that placed `p` is held: mirrored
    /// handles following the drag (the drag direction is the drawing
    /// direction, so a prepended point gets it as its incoming handle).
    /// Inside 2 doc px the point stays a corner. No history.
    pub fn vector_pen_drag_handle(&mut self, p: PointRef, cursor: Pt) -> bool {
        let Some(mut subs) = self.vector_subpaths() else {
            return false;
        };
        let at_start = self
            .vector_edit()
            .and_then(|s| s.pen_continue)
            .is_some_and(|pc| pc.at_start && pc.sub == p.sub);
        let Some(a) = subs.get_mut(p.sub).and_then(|s| s.anchors.get_mut(p.idx)) else {
            return false;
        };
        let d = (cursor.0 - a.x, cursor.1 - a.y);
        if d.0.hypot(d.1) > HANDLE_DRAG_MIN {
            let fwd = Some(PenPathHandle { x: d.0, y: d.1 });
            let back = Some(PenPathHandle { x: -d.0, y: -d.1 });
            if at_start {
                a.handle_in = fwd;
                a.handle_out = back;
            } else {
                a.handle_out = fwd;
                a.handle_in = back;
            }
            a.point_type = Some(PenPathPointType::Mirrored);
        } else {
            a.handle_in = None;
            a.handle_out = None;
            a.point_type = None;
        }
        self.vector_write(&subs)
    }
}
