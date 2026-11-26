//! Scale tool (#394): scale nodes and everything inside them by one
//! factor about an anchor point, as one undo step. Shared by the canvas
//! handle drag, the property panel's Scale section and MCP `scale_nodes`.
//!
//! The property rules live in [`crate::node_scale`], the subtree walk and
//! instance overrides in [`crate::node_scale_tree`]. This module validates
//! a [`ScaleRequest`] (pre-validate, then mutate) and writes the scaled
//! subtrees back.
//!
//! A request carries each target's pivot in its parent's space: callers
//! resolve the anchor point on the layout-resolved bounds (a Fill / Hug
//! node has no authored size to read it from) and convert it with
//! [`ScaleTarget::from_resolved`].

use jian_ops_schema::node::PenNode;

use crate::geometry::DocRect;
use crate::node_id::NodeId;
use crate::node_scale::{PropScaler, ScaleReport, VariableMode};
use crate::node_scale_tree::{Position, TreeScaler};
use crate::scale_error::ScaleError;
use crate::state::EditorState;
use crate::walkers::{descendant_contains, find_node, find_node_mut};

/// Smallest and largest factor one scale accepts.
pub const MIN_SCALE: f64 = 0.01;
pub const MAX_SCALE: f64 = 100.0;

/// The point that stays put: one of the nine points of the bounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScaleAnchor {
    #[default]
    TopLeft,
    Top,
    TopRight,
    Left,
    Center,
    Right,
    BottomLeft,
    Bottom,
    BottomRight,
}

impl ScaleAnchor {
    /// Row by row, the order of the panel's 3 × 3 grid.
    pub const ALL: [ScaleAnchor; 9] = [
        Self::TopLeft,
        Self::Top,
        Self::TopRight,
        Self::Left,
        Self::Center,
        Self::Right,
        Self::BottomLeft,
        Self::Bottom,
        Self::BottomRight,
    ];

    /// Where the anchor sits across and down the bounds (0, 0.5 or 1).
    pub fn fractions(self) -> (f64, f64) {
        let i = Self::ALL.iter().position(|a| *a == self).unwrap_or(0);
        ((i % 3) as f64 * 0.5, (i / 3) as f64 * 0.5)
    }

    /// The anchor's point on `rect`.
    pub fn point_in(self, rect: DocRect) -> (f64, f64) {
        let (fx, fy) = self.fractions();
        (rect.x + rect.w * fx, rect.y + rect.h * fy)
    }

    /// The wire / test spelling (`top-left`, `center`, …).
    pub fn ident(self) -> &'static str {
        match self {
            Self::TopLeft => "top-left",
            Self::Top => "top",
            Self::TopRight => "top-right",
            Self::Left => "left",
            Self::Center => "center",
            Self::Right => "right",
            Self::BottomLeft => "bottom-left",
            Self::Bottom => "bottom",
            Self::BottomRight => "bottom-right",
        }
    }

    /// Parse [`Self::ident`]; `_` / spaces for `-` and `centre` accepted.
    pub fn parse(raw: &str) -> Option<Self> {
        let norm = raw.trim().to_ascii_lowercase().replace(['_', ' '], "-");
        let norm = if norm == "centre" {
            "center".to_owned()
        } else {
            norm
        };
        Self::ALL.into_iter().find(|a| a.ident() == norm)
    }
}

/// One node to scale and the point it scales about, in its parent's
/// coordinate space. `None` keeps its `x` / `y` (they scale from its own
/// origin).
#[derive(Debug, Clone, PartialEq)]
pub struct ScaleTarget {
    pub node_id: NodeId,
    pub pivot: Option<(f64, f64)>,
}

impl ScaleTarget {
    /// A target whose pivot is `pivot_abs` in page space. `node_origin_abs`
    /// is the node's resolved page-space origin and `authored` its own
    /// `x` / `y`: their difference is its parent's origin.
    pub fn from_resolved(
        node_id: NodeId,
        pivot_abs: (f64, f64),
        node_origin_abs: (f64, f64),
        authored: (Option<f64>, Option<f64>),
    ) -> Self {
        let parent_x = node_origin_abs.0 - authored.0.unwrap_or(0.0);
        let parent_y = node_origin_abs.1 - authored.1.unwrap_or(0.0);
        Self {
            node_id,
            pivot: Some((pivot_abs.0 - parent_x, pivot_abs.1 - parent_y)),
        }
    }
}

/// A complete scale: the targets, one factor, what to do with bound
/// values.
#[derive(Debug, Clone, PartialEq)]
pub struct ScaleRequest {
    pub targets: Vec<ScaleTarget>,
    pub factor: f64,
    pub variables: VariableMode,
}

/// The factor that brings `current` to `target` (a W or H field).
pub fn factor_for_size(current: f64, target: f64) -> Result<f64, ScaleError> {
    if !target.is_finite() || target <= 0.0 {
        return Err(ScaleError::InvalidSize(target));
    }
    if !current.is_finite() || current <= 0.0 {
        return Err(ScaleError::EmptyBounds);
    }
    Ok(target / current)
}

/// Check a factor on its own.
pub fn check_factor(factor: f64) -> Result<(), ScaleError> {
    if !factor.is_finite() || !(MIN_SCALE..=MAX_SCALE).contains(&factor) {
        return Err(ScaleError::FactorOutOfRange(factor));
    }
    if (factor - 1.0).abs() < 1e-9 {
        return Err(ScaleError::Unchanged);
    }
    Ok(())
}

impl EditorState {
    /// Check `request` against the active page without writing.
    pub fn check_scale(&self, request: &ScaleRequest) -> Result<(), ScaleError> {
        check_factor(request.factor)?;
        if request.targets.is_empty() {
            return Err(ScaleError::NoTargets);
        }
        let roots = self.active_children();
        for target in &request.targets {
            let id = &target.node_id;
            if !id.is_real() {
                return Err(ScaleError::NodeNotFound(id.as_str().to_owned()));
            }
            if find_node(roots, id).is_none() {
                if crate::instance_anchor::locate_anchor(&self.doc, id).is_some() {
                    return Err(ScaleError::InstanceChild(id.as_str().to_owned()));
                }
                return Err(ScaleError::NodeNotFound(id.as_str().to_owned()));
            }
            if !self.is_editable(id) {
                return Err(ScaleError::NotEditable(id.as_str().to_owned()));
            }
        }
        Ok(())
    }

    /// Scale every target and its subtree. No history: see
    /// [`scale_with_history`]. A target inside another target scales with
    /// that ancestor, once.
    pub fn scale_nodes(&mut self, request: &ScaleRequest) -> Result<ScaleReport, ScaleError> {
        self.check_scale(request)?;
        let (scaled, report) = scaled_targets(self, request);
        for (id, node) in scaled {
            if let Some(slot) = find_node_mut(self.active_children_mut(), &id) {
                *slot = node;
            }
        }
        self.mark_document_changed();
        Ok(report)
    }

    /// What [`Self::scale_nodes`] would report, without writing (MCP
    /// warnings are computed on the snapshot).
    pub fn preview_scale(&self, request: &ScaleRequest) -> Result<ScaleReport, ScaleError> {
        self.check_scale(request)?;
        Ok(scaled_targets(self, request).1)
    }
}

/// The scaled copy of every outermost target, and the report.
fn scaled_targets(
    state: &EditorState,
    request: &ScaleRequest,
) -> (Vec<(NodeId, PenNode)>, ScaleReport) {
    let roots = state.active_children();
    let theme =
        crate::variables_resolve::effective_theme(&state.doc, &state.ui.variables.active_theme);
    let vars = state.doc.variables.as_ref();
    let resolve = |token: &str| crate::variables_resolve::resolve_numeric_ref(token, vars, &theme);
    let scaler = TreeScaler::new(
        PropScaler {
            factor: request.factor,
            mode: request.variables,
            resolve: &resolve,
        },
        &state.doc,
    );
    let mut report = ScaleReport::default();
    let mut out = Vec::new();
    for target in &request.targets {
        let id = &target.node_id;
        let inside_other = request.targets.iter().any(|other| {
            other.node_id != *id
                && find_node(roots, &other.node_id).is_some_and(|n| descendant_contains(n, id))
        });
        if inside_other || out.iter().any(|(done, _)| done == id) {
            continue;
        }
        let Some(mut node) = find_node(roots, id).cloned() else {
            continue;
        };
        let position = match target.pivot {
            Some(_) if crate::drag_mutators::selected_is_flow_child(roots, id) => Position::Keep,
            Some((px, py)) => Position::Pivot(px, py),
            None => Position::Keep,
        };
        scaler.scale_node(&mut node, position, &mut report);
        out.push((id.clone(), node));
    }
    (out, report)
}

/// Scale as one undo step. Returns the report, or why nothing changed.
pub fn scale_with_history(
    state: &mut EditorState,
    request: &ScaleRequest,
) -> Result<ScaleReport, ScaleError> {
    state.check_scale(request)?;
    let snapshot = state.snapshot_for_history();
    let report = state.scale_nodes(request)?;
    state.history_push_past(snapshot);
    Ok(report)
}

/// The ids a scale of the current selection covers: real, editable
/// nodes on the active page, in selection order.
pub fn scalable_selection(state: &EditorState) -> Vec<NodeId> {
    state
        .selection
        .set
        .iter()
        .filter(|id| id.is_real() && find_node(state.active_children(), id).is_some())
        .filter(|id| state.is_editable(id))
        .cloned()
        .collect()
}

#[cfg(test)]
#[path = "selection_scale_tests.rs"]
mod tests;
