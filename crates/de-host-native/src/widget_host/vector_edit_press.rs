//! Vector edit mode (#387) pointer arms: press, drag, release.
//!
//! The geometry and its undo live in `de_editor_core::vector_edit`;
//! the scene mapping and hit test in
//! `de_editor_ui::widgets::vector_edit_view`. This module only routes
//! the pointer: what a press grabbed, how a drag moves it (with Shift
//! constraints and snapping) and what a release commits. One undo step
//! per gesture: the press snapshots, the first real move pushes.

use super::WidgetHostNative;
use de_editor_core::history::EditorSnapshot;
use de_editor_core::pen::PathHandleSide;
use de_editor_core::vector_edit::{
    constrain_45, join_endpoints, snap_point, snap_to_pixel_grid, PointRef, Pt, SnapGuide, Subpath,
};
use de_editor_core::Tool;
use de_editor_ui::widgets::host_canvas_geometry as canvas_geometry;
use de_editor_ui::widgets::vector_edit_view::{VectorEditView, VectorHit, SNAP_PX};
use de_editor_ui::Point2D;
use std::collections::BTreeSet;

/// Two presses within this window / distance on one point toggle it.
const DOUBLE_CLICK_MS: u64 = 400;
const DOUBLE_CLICK_PX: f32 = 4.0;
/// Screen px a press must travel before it counts as a drag.
const DRAG_SLOP_PX: f32 = 2.0;
/// Zoom from which points snap to whole pixels (Figma's pixel grid).
const PIXEL_GRID_ZOOM: f32 = 4.0;

/// What a vector-edit drag moves.
pub(in crate::widget_host) enum VectorDragKind {
    /// Selected points; `lead` is the grabbed one (snaps, joins).
    Points {
        moving: BTreeSet<PointRef>,
        lead: Option<PointRef>,
        /// Open end point the lead landed on, joined on release.
        drop_on: Option<PointRef>,
    },
    /// One bezier handle; positions are local at press.
    Handle {
        p: PointRef,
        side: PathHandleSide,
        anchor: Pt,
        grab: Pt,
    },
    /// The point the pen just placed: dragging pulls its handles.
    PenHandle { p: PointRef },
    /// Marquee (or lasso) selection, scene coordinates.
    Marquee {
        lasso: bool,
        additive: bool,
        path: Vec<Point2D>,
    },
}

/// A vector-edit pointer gesture in flight.
pub(in crate::widget_host) struct VectorDragState {
    pub(in crate::widget_host) kind: VectorDragKind,
    /// Press point, screen and unrotated scene.
    press_screen: (f32, f32),
    start_scene: Point2D,
    /// Geometry at press (drags never accumulate rounding).
    base: Vec<Subpath>,
    /// History snapshot pushed on the first real move; `None` when the
    /// press already recorded its own undo step.
    pre: Option<EditorSnapshot>,
    moved: bool,
    /// Snap candidates, scene coordinates, gathered once per gesture.
    candidates: Vec<Pt>,
}

/// Host-side vector edit state.
#[derive(Default)]
pub(in crate::widget_host) struct VectorEditHost {
    pub(in crate::widget_host) drag: Option<VectorDragState>,
    /// Last point press `(ms, x, y, point)`, for double-click toggles.
    last_click: Option<(u64, f32, f32, PointRef)>,
}

impl WidgetHostNative {
    fn vector_view(&self) -> Option<VectorEditView> {
        VectorEditView::build(&self.editor_state, &self.layout_scene)
    }

    /// Cursor → unrotated scene point of the edited path.
    fn vector_cursor(&self, view: &VectorEditView, x: f32, y: f32) -> Point2D {
        view.unrotate(canvas_geometry::canvas_doc_point_unclamped(
            &self.editor_state,
            x,
            y,
        ))
    }

    /// Shift constraint (around `origin`), then point / axis snapping,
    /// then the pixel grid — all suspended by Cmd / Ctrl except Shift.
    /// Scene coordinates in and out.
    fn vector_snap(
        &self,
        raw: Pt,
        origin: Option<Pt>,
        candidates: &[Pt],
    ) -> (Pt, bool, Vec<SnapGuide>) {
        let shift = self.shift_held && origin.is_some();
        let mut p = match origin {
            Some(o) if self.shift_held => constrain_45(o, raw),
            _ => raw,
        };
        if self.cmd_held {
            return (p, false, Vec::new());
        }
        let zoom = self.editor_state.viewport.zoom.max(0.0001);
        if !shift {
            let r = snap_point(p, candidates, (SNAP_PX / zoom) as f64);
            if r.snapped {
                return (r.point, r.on_point, r.guides);
            }
        }
        if zoom >= PIXEL_GRID_ZOOM && !self.editor_state.ui.pixel_grid_snap_off {
            p = snap_to_pixel_grid(p);
        }
        (p, false, Vec::new())
    }

    /// Canvas press while vector edit mode runs (Select or Pen tool).
    /// `None` when the mode is not active (the caller routes normally).
    pub(in crate::widget_host) fn apply_vector_edit_press(
        &mut self,
        x: f32,
        y: f32,
    ) -> Option<bool> {
        self.editor_state.prune_vector_edit();
        if !self.editor_state.vector_edit_active() {
            return None;
        }
        if !matches!(self.editor_state.tool, Tool::Select | Tool::Pen) {
            self.editor_state.exit_vector_edit();
            return None;
        }
        self.refresh_layout_scene();
        let view = self.vector_view()?;
        let scene = self.vector_cursor(&view, x, y);
        let hit = view.hit(scene, self.editor_state.viewport.zoom);
        let pen = matches!(self.editor_state.tool, Tool::Pen);
        let handled = match hit {
            Some(VectorHit::Handle(p, side)) => {
                self.start_vector_handle_drag(&view, x, y, scene, p, side)
            }
            Some(VectorHit::Point(p)) if pen => self.vector_pen_point_press(&view, x, y, scene, p),
            Some(VectorHit::Point(p)) => self.vector_point_press(&view, x, y, scene, p),
            Some(VectorHit::Segment { sub, seg, t }) if pen => {
                match self.editor_state.vector_insert_point(sub, seg, t) {
                    Some(p) => {
                        let view = self.vector_view()?;
                        self.start_vector_points_drag(&view, x, y, scene, Some(p), false)
                    }
                    None => false,
                }
            }
            Some(VectorHit::Segment { sub, seg, .. }) => {
                let n = view.subs.get(sub).map_or(0, |s| s.anchors.len());
                let ends = [
                    PointRef::new(sub, seg),
                    PointRef::new(sub, (seg + 1) % n.max(1)),
                ];
                self.editor_state.vector_select_many(ends, self.shift_held);
                self.start_vector_points_drag(&view, x, y, scene, None, true)
            }
            None if pen => self.vector_pen_add(&view, x, y, scene),
            None => self.start_vector_marquee(&view, x, y, scene),
        };
        self.mark_dirty();
        Some(handled)
    }

    fn vector_point_press(
        &mut self,
        view: &VectorEditView,
        x: f32,
        y: f32,
        scene: Point2D,
        p: PointRef,
    ) -> bool {
        let is_double = matches!(
            self.vector_edit_host.last_click,
            Some((t, px, py, q)) if q == p
                && self.now_ms.saturating_sub(t) < DOUBLE_CLICK_MS
                && (x - px).abs() < DOUBLE_CLICK_PX
                && (y - py).abs() < DOUBLE_CLICK_PX
        );
        self.vector_edit_host.last_click = Some((self.now_ms, x, y, p));
        if self.alt_held || is_double {
            self.vector_edit_host.last_click = None;
            return self.editor_state.vector_toggle_smooth(p);
        }
        if self.shift_held {
            self.editor_state.vector_select(p, true);
        } else if !view.selected.contains(&p) {
            self.editor_state.vector_select(p, false);
        }
        let selected = self
            .editor_state
            .vector_edit()
            .is_some_and(|s| s.selected.contains(&p));
        if !selected {
            return true;
        }
        self.start_vector_points_drag(view, x, y, scene, Some(p), true)
    }

    /// Pen on a point: close / connect while drawing, continue from an
    /// open end, otherwise behave like the Move tool.
    fn vector_pen_point_press(
        &mut self,
        view: &VectorEditView,
        x: f32,
        y: f32,
        scene: Point2D,
        p: PointRef,
    ) -> bool {
        let is_end = view.subs.get(p.sub).is_some_and(|s| s.is_endpoint(p.idx));
        if let Some(pc) = view.pen_continue {
            let len = view.subs.get(pc.sub).map_or(0, |s| s.anchors.len());
            let own_end = if pc.at_start {
                0
            } else {
                len.saturating_sub(1)
            };
            let far_end = if pc.at_start {
                len.saturating_sub(1)
            } else {
                0
            };
            if p.sub == pc.sub && p.idx == far_end && len > 2 {
                return self.editor_state.vector_pen_close();
            }
            if is_end && !(p.sub == pc.sub && p.idx == own_end) {
                return self.editor_state.vector_pen_connect(p);
            }
        } else if is_end {
            return self.editor_state.vector_pen_continue_from(p);
        }
        self.vector_point_press(view, x, y, scene, p)
    }

    fn vector_pen_add(&mut self, view: &VectorEditView, x: f32, y: f32, scene: Point2D) -> bool {
        let origin = self.editor_state.vector_pen_anchor().map(|a| {
            let s = view.to_scene(a);
            (s.x as f64, s.y as f64)
        });
        let candidates = view.snap_candidates(&self.layout_scene, &BTreeSet::new());
        let (snapped, _, _) =
            self.vector_snap((scene.x as f64, scene.y as f64), origin, &candidates);
        let local = view.to_local(Point2D::new(snapped.0 as f32, snapped.1 as f32));
        let Some(p) = self.editor_state.vector_pen_add_point(local) else {
            return false;
        };
        self.vector_edit_host.drag = Some(VectorDragState {
            kind: VectorDragKind::PenHandle { p },
            press_screen: (x, y),
            start_scene: scene,
            base: Vec::new(),
            pre: None,
            moved: false,
            candidates: Vec::new(),
        });
        true
    }

    fn start_vector_points_drag(
        &mut self,
        view: &VectorEditView,
        x: f32,
        y: f32,
        scene: Point2D,
        lead: Option<PointRef>,
        record: bool,
    ) -> bool {
        let Some(session) = self.editor_state.vector_edit() else {
            return false;
        };
        let moving = session.selected.clone();
        let base = self.editor_state.vector_subpaths().unwrap_or_default();
        let candidates = view.snap_candidates(&self.layout_scene, &moving);
        let pre = record.then(|| self.editor_state.snapshot_for_history());
        self.vector_edit_host.drag = Some(VectorDragState {
            kind: VectorDragKind::Points {
                moving,
                lead,
                drop_on: None,
            },
            press_screen: (x, y),
            start_scene: scene,
            base,
            pre,
            moved: false,
            candidates,
        });
        true
    }

    #[allow(clippy::too_many_arguments)]
    fn start_vector_handle_drag(
        &mut self,
        view: &VectorEditView,
        x: f32,
        y: f32,
        scene: Point2D,
        p: PointRef,
        side: PathHandleSide,
    ) -> bool {
        let (Some(anchor), Some(grab)) = (view.point(p), view.handle(p, side)) else {
            return false;
        };
        self.vector_edit_host.drag = Some(VectorDragState {
            kind: VectorDragKind::Handle {
                p,
                side,
                anchor,
                grab,
            },
            press_screen: (x, y),
            start_scene: scene,
            base: view.subs.clone(),
            pre: Some(self.editor_state.snapshot_for_history()),
            moved: false,
            candidates: view.snap_candidates(&self.layout_scene, &BTreeSet::new()),
        });
        true
    }

    fn start_vector_marquee(
        &mut self,
        view: &VectorEditView,
        x: f32,
        y: f32,
        scene: Point2D,
    ) -> bool {
        let lasso = self.alt_held || self.editor_state.vector_edit().is_some_and(|s| s.lasso);
        self.vector_edit_host.drag = Some(VectorDragState {
            kind: VectorDragKind::Marquee {
                lasso,
                additive: self.shift_held,
                path: vec![scene],
            },
            press_screen: (x, y),
            start_scene: scene,
            base: view.subs.clone(),
            pre: None,
            moved: false,
            candidates: Vec::new(),
        });
        true
    }

    /// Cursor move during a vector-edit gesture. False when none runs.
    pub(in crate::widget_host) fn apply_vector_edit_drag_move(&mut self, x: f32, y: f32) -> bool {
        let Some(mut drag) = self.vector_edit_host.drag.take() else {
            return false;
        };
        // The previous move rewrote the path: the scene must follow before
        // its offset is read again.
        self.refresh_layout_scene();
        let Some(view) = self.vector_view() else {
            return true;
        };
        let (px, py) = drag.press_screen;
        if !drag.moved && (x - px).hypot(y - py) < DRAG_SLOP_PX {
            self.vector_edit_host.drag = Some(drag);
            return true;
        }
        if !drag.moved {
            drag.moved = true;
            if let Some(pre) = drag.pre.take() {
                self.editor_state.history_push_past(pre);
            }
        }
        let scene = self.vector_cursor(&view, x, y);
        let delta = (
            (scene.x - drag.start_scene.x) as f64,
            (scene.y - drag.start_scene.y) as f64,
        );
        let mut guides = Vec::new();
        match &mut drag.kind {
            VectorDragKind::Points {
                moving,
                lead,
                drop_on,
            } => {
                let (mut dx, mut dy) = delta;
                if let Some(lead) = *lead {
                    let base_pt = drag
                        .base
                        .get(lead.sub)
                        .and_then(|s| s.anchors.get(lead.idx))
                        .map(|a| (a.x + view.off.0, a.y + view.off.1));
                    if let Some(b) = base_pt {
                        let raw = (b.0 + dx, b.1 + dy);
                        let (snapped, on_point, g) =
                            self.vector_snap(raw, Some(b), &drag.candidates);
                        dx = snapped.0 - b.0;
                        dy = snapped.1 - b.1;
                        guides = g;
                        *drop_on = if on_point && moving.len() == 1 {
                            endpoint_at(
                                &drag.base,
                                (snapped.0 - view.off.0, snapped.1 - view.off.1),
                                &lead,
                            )
                        } else {
                            None
                        };
                    }
                }
                self.editor_state
                    .vector_move_points_from(&drag.base, moving, dx, dy);
            }
            VectorDragKind::Handle {
                p,
                side,
                anchor,
                grab,
            } => {
                let raw = (grab.0 + view.off.0 + delta.0, grab.1 + view.off.1 + delta.1);
                let origin = (anchor.0 + view.off.0, anchor.1 + view.off.1);
                let (snapped, _, g) = self.vector_snap(raw, Some(origin), &drag.candidates);
                guides = g;
                let offset = (snapped.0 - origin.0, snapped.1 - origin.1);
                self.editor_state.vector_set_handle_from(
                    &drag.base,
                    *p,
                    *side,
                    offset,
                    self.alt_held,
                );
            }
            VectorDragKind::PenHandle { p } => {
                let mut cursor = (scene.x as f64, scene.y as f64);
                if let Some(a) = view.point(*p) {
                    let a = (a.0 + view.off.0, a.1 + view.off.1);
                    if self.shift_held {
                        cursor = constrain_45(a, cursor);
                    }
                }
                let local = (cursor.0 - view.off.0, cursor.1 - view.off.1);
                self.editor_state.vector_pen_drag_handle(*p, local);
            }
            VectorDragKind::Marquee { lasso, path, .. } => {
                if *lasso {
                    path.push(scene);
                } else {
                    let s = drag.start_scene;
                    *path = vec![
                        s,
                        Point2D::new(scene.x, s.y),
                        scene,
                        Point2D::new(s.x, scene.y),
                    ];
                }
                if let Some(session) = self.editor_state.ui.vector_edit.as_mut() {
                    session.marquee = path.iter().map(|p| (p.x as f64, p.y as f64)).collect();
                }
            }
        }
        if let Some(session) = self.editor_state.ui.vector_edit.as_mut() {
            session.guides = guides;
        }
        self.vector_edit_host.drag = Some(drag);
        self.mark_dirty();
        true
    }

    /// Pen hover inside the mode: the rubber band follows the (snapped)
    /// cursor. `None` when the pen is not drawing.
    pub(in crate::widget_host) fn apply_vector_pen_hover(
        &mut self,
        x: f32,
        y: f32,
    ) -> Option<bool> {
        if !matches!(self.editor_state.tool, Tool::Pen) || self.vector_edit_host.drag.is_some() {
            return None;
        }
        let anchor = self.editor_state.vector_pen_anchor()?;
        self.refresh_layout_scene();
        let view = self.vector_view()?;
        let scene = self.vector_cursor(&view, x, y);
        let a = view.to_scene(anchor);
        let (snapped, _, guides) = self.vector_snap(
            (scene.x as f64, scene.y as f64),
            Some((a.x as f64, a.y as f64)),
            &[],
        );
        let session = self.editor_state.ui.vector_edit.as_mut()?;
        session.pen_cursor = Some(snapped);
        session.guides = guides;
        self.mark_dirty();
        Some(true)
    }

    /// Release of a vector-edit gesture. False when none ran.
    pub(in crate::widget_host) fn apply_vector_edit_release(&mut self) -> bool {
        let Some(drag) = self.vector_edit_host.drag.take() else {
            return false;
        };
        if let Some(session) = self.editor_state.ui.vector_edit.as_mut() {
            session.guides.clear();
            session.marquee.clear();
        }
        self.refresh_layout_scene();
        match drag.kind {
            VectorDragKind::Points {
                lead: Some(lead),
                drop_on: Some(target),
                ..
            } if drag.moved => {
                // An end point dropped on an end point joins them, inside
                // the drag's own undo step.
                if let Some(mut subs) = self.editor_state.vector_subpaths() {
                    if join_endpoints(&mut subs, lead, target, true).is_some() {
                        self.editor_state.vector_write(&subs);
                        self.editor_state.vector_select_many([], false);
                    }
                }
            }
            VectorDragKind::Marquee { path, additive, .. } => {
                if drag.moved {
                    if let Some(view) = self.vector_view() {
                        let pts = view.points_in_polygon(&path);
                        self.editor_state.vector_select_many(pts, additive);
                    }
                } else {
                    self.vector_click_empty(drag.start_scene, &drag.base);
                }
            }
            _ => {}
        }
        self.mark_dirty();
        true
    }

    /// A click on empty canvas: inside the path's box it clears the
    /// point selection, outside it leaves the mode.
    fn vector_click_empty(&mut self, scene: Point2D, base: &[Subpath]) {
        let inside = self.vector_view().is_some_and(|view| {
            let (x, y, w, h) = de_editor_core::vector_edit::subpaths_bounds(base);
            let p = view.to_local(scene);
            p.0 >= x && p.0 <= x + w && p.1 >= y && p.1 <= y + h
        });
        let has_selection = self
            .editor_state
            .vector_edit()
            .is_some_and(|s| !s.selected.is_empty());
        if inside && has_selection {
            self.editor_state.vector_select_many([], false);
        } else {
            self.editor_state.exit_vector_edit();
        }
    }

    /// Pointer cancel: keep what the drag already did as one undo step.
    pub(in crate::widget_host) fn cancel_vector_drag(&mut self) -> bool {
        let had = self.vector_edit_host.drag.is_some();
        if had {
            let _ = self.apply_vector_edit_release();
        }
        had
    }
}

/// An open end point (other than `lead`) at `p` in `base`.
fn endpoint_at(base: &[Subpath], p: Pt, lead: &PointRef) -> Option<PointRef> {
    for (si, sub) in base.iter().enumerate() {
        for i in [0, sub.anchors.len().saturating_sub(1)] {
            let r = PointRef::new(si, i);
            if r == *lead || !sub.is_endpoint(i) {
                continue;
            }
            let a = &sub.anchors[i];
            if (a.x - p.0).abs() < 1e-6 && (a.y - p.1).abs() < 1e-6 {
                return Some(r);
            }
        }
    }
    None
}
