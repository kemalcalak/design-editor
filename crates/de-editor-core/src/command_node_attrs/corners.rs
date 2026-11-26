//! `SetNodeCorners` (#389): the radius of each corner and corner smoothing
//! in one validated step. What a node can take is decided before anything
//! is written, so a rejected call leaves the document untouched.

use crate::node_id::NodeId;
use crate::state::EditorState;
use crate::walkers::{find_node, find_node_mut};
use jian_ops_schema::node::{BooleanNode, CornerRadius, PenNode, ScriptNode};

/// What a node kind stores for its corners.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CornerSupport {
    /// Four independent radii (else one radius for every corner).
    pub per_corner: bool,
    /// A `cornerSmoothing` field.
    pub smoothing: bool,
}

/// The corner fields `node` has, or `None` for a kind without corners.
pub fn corner_support(node: &PenNode) -> Option<CornerSupport> {
    let support = |per_corner, smoothing| {
        Some(CornerSupport {
            per_corner,
            smoothing,
        })
    };
    match node {
        PenNode::Frame(_) | PenNode::Script(_) | PenNode::Group(_) | PenNode::Rectangle(_) => {
            support(true, true)
        }
        // A boolean group paints its computed path; an image or a browser
        // has no smoothing field.
        PenNode::Boolean(_) | PenNode::Image(_) | PenNode::Browser(_) => support(true, false),
        PenNode::Polygon(_) | PenNode::Path(_) => support(false, true),
        PenNode::Ellipse(_) => support(false, false),
        _ => None,
    }
}

/// The node's corner smoothing (`0..=1`, absent = `0`), or `None` for a
/// kind without a smoothing field.
pub fn node_corner_smoothing(node: &PenNode) -> Option<f32> {
    let stored = match node {
        PenNode::Frame(n) | PenNode::Script(ScriptNode { frame: n, .. }) => {
            n.container.corner_smoothing
        }
        PenNode::Group(n) => n.container.corner_smoothing,
        PenNode::Rectangle(n) => n.container.corner_smoothing,
        PenNode::Polygon(n) => n.corner_smoothing,
        PenNode::Path(n) => n.corner_smoothing,
        _ => return None,
    };
    Some(crate::corner_geometry::clamp_smoothing(stored.unwrap_or(0.0)) as f32)
}

/// A smoothing factor as stored: `None` for none, else rounded to 1/10000
/// so a panel percentage does not write float noise into the document.
pub(crate) fn stored_smoothing(smoothing: f32) -> Option<f64> {
    let value = (f64::from(smoothing) * 10_000.0).round() / 10_000.0;
    (value > 0.0).then_some(value)
}

impl EditorState {
    /// `SetNodeCorners` — see [`crate::EditorCommand::SetNodeCorners`].
    pub(crate) fn cmd_set_node_corners(
        &mut self,
        node_id: &NodeId,
        radii: Option<[f32; 4]>,
        smoothing: Option<f32>,
    ) -> bool {
        if !node_id.is_real() || (radii.is_none() && smoothing.is_none()) {
            return false;
        }
        if radii.is_some_and(|r| r.iter().any(|v| !v.is_finite() || *v < 0.0)) {
            return false;
        }
        if smoothing.is_some_and(|s| !s.is_finite() || !(0.0..=1.0).contains(&s)) {
            return false;
        }
        let Some(support) = find_node(self.active_children(), node_id).and_then(corner_support)
        else {
            return false;
        };
        let uniform = radii.map(|r| r.iter().all(|v| (*v - r[0]).abs() < f32::EPSILON));
        if (uniform == Some(false) && !support.per_corner)
            || (smoothing.is_some() && !support.smoothing)
        {
            return false;
        }
        let Some(node) = find_node_mut(self.active_children_mut(), node_id) else {
            return false;
        };
        if let Some(r) = radii {
            let radius = if uniform == Some(true) {
                CornerRadius::Uniform(f64::from(r[0]))
            } else {
                CornerRadius::PerCorner(r.map(f64::from))
            };
            write_radius(node, radius);
        }
        if let Some(s) = smoothing {
            write_smoothing(node, stored_smoothing(s));
        }
        true
    }
}

fn write_radius(node: &mut PenNode, radius: CornerRadius) {
    let uniform = match &radius {
        CornerRadius::Uniform(r) => *r,
        CornerRadius::PerCorner(r) => r[0],
    };
    match node {
        PenNode::Frame(n) | PenNode::Script(ScriptNode { frame: n, .. }) => {
            n.container.corner_radius = Some(radius)
        }
        PenNode::Group(n) | PenNode::Boolean(BooleanNode { group: n, .. }) => {
            n.container.corner_radius = Some(radius)
        }
        PenNode::Rectangle(n) => n.container.corner_radius = Some(radius),
        PenNode::Image(n) => n.corner_radius = Some(radius),
        PenNode::Browser(n) => n.corner_radius = Some(radius),
        PenNode::Polygon(n) => n.corner_radius = Some(uniform),
        PenNode::Path(n) => n.vertex_radius = Some(uniform),
        PenNode::Ellipse(n) => n.corner_radius = Some(uniform),
        _ => {}
    }
}

fn write_smoothing(node: &mut PenNode, smoothing: Option<f64>) {
    match node {
        PenNode::Frame(n) | PenNode::Script(ScriptNode { frame: n, .. }) => {
            n.container.corner_smoothing = smoothing
        }
        PenNode::Group(n) => n.container.corner_smoothing = smoothing,
        PenNode::Rectangle(n) => n.container.corner_smoothing = smoothing,
        PenNode::Polygon(n) => n.corner_smoothing = smoothing,
        PenNode::Path(n) => n.corner_smoothing = smoothing,
        _ => {}
    }
}

#[cfg(test)]
#[path = "corners_tests.rs"]
mod tests;
