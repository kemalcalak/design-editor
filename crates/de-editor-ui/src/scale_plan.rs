//! Scale tool (#394): turn "these nodes, this factor, this anchor" into a
//! core [`ScaleRequest`] using the layout-resolved scene, shared by the
//! canvas handle drag, the property panel's Scale section and MCP
//! `scale_nodes`.
//!
//! The anchor point sits on the union of the targets' resolved bounds (a
//! Fill / Hug node has no authored size to read it from), and each target
//! gets that point in its own parent's space.

use de_editor_core::geometry::DocRect;
use de_editor_core::node_id::NodeId;
use de_editor_core::{
    EditorState, PenNodeExt, ScaleAnchor, ScaleRequest, ScaleTarget, VariableMode,
};

use crate::layout_scene::LayoutScene;
use crate::widgets::SelectionHandle;
use de_editor_core::history::EditorSnapshot;
use jian_ops_schema::node::PenNode;

/// One target: its id, resolved page-space origin and authored `x` / `y`.
#[derive(Debug, Clone, PartialEq)]
struct PlannedTarget {
    id: NodeId,
    origin: (f64, f64),
    authored: (Option<f64>, Option<f64>),
}

/// The resolved geometry a scale of some nodes reads.
#[derive(Debug, Clone, PartialEq)]
pub struct ScalePlan {
    /// Union of the targets' resolved bounds, page space.
    pub bounds: DocRect,
    targets: Vec<PlannedTarget>,
}

impl ScalePlan {
    /// The plan for `ids` on the active page, or `None` when none of them
    /// is laid out with a size.
    pub fn new(state: &EditorState, scene: &LayoutScene, ids: &[NodeId]) -> Option<Self> {
        let page = scene.active_page()?;
        let mut targets = Vec::new();
        let (mut min_x, mut min_y, mut max_x, mut max_y) = (
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        );
        for id in ids {
            let Some(scene_node) = page.find(id.as_str()) else {
                continue;
            };
            let Some(node) = de_editor_core::walkers::find_node(state.active_children(), id) else {
                continue;
            };
            let agg = scene_node.aggregate_bounds();
            min_x = min_x.min(f64::from(agg.origin.x));
            min_y = min_y.min(f64::from(agg.origin.y));
            max_x = max_x.max(f64::from(agg.origin.x + agg.size.x));
            max_y = max_y.max(f64::from(agg.origin.y + agg.size.y));
            let base = node.base();
            targets.push(PlannedTarget {
                id: id.clone(),
                origin: (
                    f64::from(scene_node.bounds.origin.x),
                    f64::from(scene_node.bounds.origin.y),
                ),
                authored: (base.x, base.y),
            });
        }
        if targets.is_empty() || !(max_x > min_x || max_y > min_y) {
            return None;
        }
        Some(Self {
            bounds: DocRect {
                x: min_x,
                y: min_y,
                w: max_x - min_x,
                h: max_y - min_y,
            },
            targets,
        })
    }

    /// The plan for the editable selection.
    pub fn for_selection(state: &EditorState, scene: &LayoutScene) -> Option<Self> {
        let ids = de_editor_core::selection_scale::scalable_selection(state);
        Self::new(state, scene, &ids)
    }

    /// Scale by `factor` keeping `anchor` of the bounds in place.
    pub fn request(
        &self,
        factor: f64,
        anchor: ScaleAnchor,
        variables: VariableMode,
    ) -> ScaleRequest {
        self.request_about(factor, anchor.point_in(self.bounds), variables)
    }

    /// Scale by `factor` about the page-space point `pivot`.
    pub fn request_about(
        &self,
        factor: f64,
        pivot: (f64, f64),
        variables: VariableMode,
    ) -> ScaleRequest {
        ScaleRequest {
            targets: self
                .targets
                .iter()
                .map(|t| ScaleTarget::from_resolved(t.id.clone(), pivot, t.origin, t.authored))
                .collect(),
            factor,
            variables,
        }
    }

    /// The bounds after scaling by `factor` about `anchor`.
    pub fn scaled_bounds(&self, factor: f64, anchor: ScaleAnchor) -> DocRect {
        let (px, py) = anchor.point_in(self.bounds);
        DocRect {
            x: px + (self.bounds.x - px) * factor,
            y: py + (self.bounds.y - py) * factor,
            w: self.bounds.w * factor,
            h: self.bounds.h * factor,
        }
    }

    /// The target ids, in order.
    pub fn ids(&self) -> impl Iterator<Item = &NodeId> {
        self.targets.iter().map(|t| &t.id)
    }
}

/// The anchor a handle drag keeps in place: the opposite point.
pub fn opposite_anchor(handle: SelectionHandle) -> ScaleAnchor {
    match handle {
        SelectionHandle::TopLeft => ScaleAnchor::BottomRight,
        SelectionHandle::Top => ScaleAnchor::Bottom,
        SelectionHandle::TopRight => ScaleAnchor::BottomLeft,
        SelectionHandle::Right => ScaleAnchor::Left,
        SelectionHandle::BottomRight => ScaleAnchor::TopLeft,
        SelectionHandle::Bottom => ScaleAnchor::Top,
        SelectionHandle::BottomLeft => ScaleAnchor::TopRight,
        SelectionHandle::Left => ScaleAnchor::Right,
    }
}

/// The factor a handle dragged by `(dx, dy)` document px asks for: the
/// axis the handle moves, or for a corner the axis that changed most.
/// Clamped to the core's range.
pub fn drag_factor(handle: SelectionHandle, bounds: DocRect, dx: f64, dy: f64) -> f64 {
    let sx = if handle.moves_left_edge() {
        -1.0
    } else if handle.resizes_width() {
        1.0
    } else {
        0.0
    };
    let sy = if handle.moves_top_edge() {
        -1.0
    } else if handle.resizes_height() {
        1.0
    } else {
        0.0
    };
    let fx = (sx != 0.0 && bounds.w > 0.0).then(|| (bounds.w + sx * dx) / bounds.w);
    let fy = (sy != 0.0 && bounds.h > 0.0).then(|| (bounds.h + sy * dy) / bounds.h);
    let factor = match (fx, fy) {
        (Some(a), Some(b)) if (a - 1.0).abs() >= (b - 1.0).abs() => a,
        (Some(_), Some(b)) => b,
        (Some(a), None) | (None, Some(a)) => a,
        (None, None) => 1.0,
    };
    factor.clamp(
        de_editor_core::selection_scale::MIN_SCALE,
        de_editor_core::selection_scale::MAX_SCALE,
    )
}

/// A live Scale-tool handle drag: the selection as it was at press, so
/// every move rescales from the original rather than compounding.
#[derive(Debug, Clone)]
pub struct ScaleDrag {
    pub handle: SelectionHandle,
    pub start_screen: (f32, f32),
    plan: ScalePlan,
    originals: Vec<(NodeId, PenNode)>,
    before: EditorSnapshot,
    /// The last factor applied (1 until the pointer moves).
    pub factor: f64,
}

impl ScaleDrag {
    /// Start a drag on `handle` over the editable selection.
    pub fn begin(
        state: &EditorState,
        scene: &LayoutScene,
        handle: SelectionHandle,
        start_screen: (f32, f32),
    ) -> Option<Self> {
        let plan = ScalePlan::for_selection(state, scene)?;
        let originals = plan
            .ids()
            .filter_map(|id| {
                de_editor_core::walkers::find_node(state.active_children(), id)
                    .map(|n| (id.clone(), n.clone()))
            })
            .collect();
        Some(Self {
            handle,
            start_screen,
            plan,
            originals,
            before: state.snapshot_for_history(),
            factor: 1.0,
        })
    }

    fn put_back(&self, state: &mut EditorState) {
        for (id, node) in &self.originals {
            if let Some(slot) =
                de_editor_core::walkers::find_node_mut(state.active_children_mut(), id)
            {
                *slot = node.clone();
            }
        }
    }

    /// Rescale for a pointer `(dx, dy)` document px from the press.
    /// Returns whether the document changed.
    pub fn update(&mut self, state: &mut EditorState, dx: f64, dy: f64) -> bool {
        let factor = drag_factor(self.handle, self.plan.bounds, dx, dy);
        if (factor - self.factor).abs() < 1e-9 {
            return false;
        }
        self.factor = factor;
        self.put_back(state);
        let anchor = opposite_anchor(self.handle);
        let request = self.plan.request(factor, anchor, VariableMode::KeepBound);
        if state.scale_nodes(&request).is_err() {
            // Back at 1 (or refused): the originals are already back.
            state.mark_document_changed();
        }
        true
    }

    /// End the drag: one undo step when anything changed.
    pub fn finish(self, state: &mut EditorState) -> bool {
        if (self.factor - 1.0).abs() < 1e-9 {
            return false;
        }
        state.history_push_past(self.before);
        true
    }

    /// Abandon the drag and restore the selection as it was.
    pub fn cancel(self, state: &mut EditorState) {
        if (self.factor - 1.0).abs() >= 1e-9 {
            self.put_back(state);
            state.mark_document_changed();
        }
    }
}

#[cfg(test)]
#[path = "scale_plan_tests.rs"]
mod tests;
