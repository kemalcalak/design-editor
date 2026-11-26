//! [`PathCommand::Commit`](super::PathCommand::Commit): replace sources
//! with one host-computed path.
//!
//! The host hands over editable subpaths in document space (curves kept)
//! plus the document origin of the frame the result will live in; the
//! commit maps them into that frame, writes one path node in the first
//! source's slot (its parent, its index) and removes the sources. A
//! boolean or plain group source goes with its children (baking them is
//! the point); any other source holding children is refused, since its
//! outline is only its own box.

use crate::id_allocator::IdAllocError;
use crate::node_id::NodeId;
use crate::pen_node_ext::PenNodeExt;
use crate::state::EditorState;
use crate::vector_edit::{write_subpaths, Subpath};
use crate::walkers::{find_node, find_node_mut, find_parent_and_index, insert_into_parent};
use jian_ops_schema::node::{
    BooleanNode, PathFillRule, PathNode, PenNode, PenNodeBase, ScriptNode,
};
use jian_ops_schema::style::{PenEffect, PenFill, PenStroke};

/// What produced a committed path: names the result and decides its
/// paint and placement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathCommitKind {
    /// A destructive boolean (`boolean_op` with `mode: "flatten"`).
    Boolean,
    /// Flatten: shapes / groups / a boolean group baked into one path.
    Flatten,
    /// Outline stroke: the source's stroke as a filled path, painted with
    /// the stroke's paint. A source with a fill keeps it (stroke
    /// removed) under the new path.
    OutlineStroke,
    /// Offset path: the grown / shrunk outline, in the source's style.
    Offset,
}

impl PathCommitKind {
    pub fn verb(self) -> &'static str {
        match self {
            PathCommitKind::Boolean => "boolean",
            PathCommitKind::Flatten => "flatten",
            PathCommitKind::OutlineStroke => "outline stroke",
            PathCommitKind::Offset => "offset path",
        }
    }

    /// Layer name when several sources merge.
    fn default_name(self) -> &'static str {
        match self {
            PathCommitKind::Boolean => "Boolean Result",
            _ => "Vector",
        }
    }
}

/// A host-computed path to commit.
#[derive(Debug, Clone, PartialEq)]
pub struct PathCommit {
    /// Nodes the result replaces (paint order; the first gives the slot,
    /// name and style).
    pub source_ids: Vec<NodeId>,
    /// The result, document space.
    pub subpaths: Vec<Subpath>,
    /// Even-odd fill (skia's boolean results are even-odd).
    pub even_odd: bool,
    /// Document origin of the nearest frame around the first source
    /// (`(0, 0)` at the page root). Groups between it and the source are
    /// not in it: the commit adds their own `x` / `y`.
    pub origin: (f64, f64),
    /// Id for the new path.
    pub new_id: NodeId,
    /// Document revision the result was computed against; a document
    /// that moved on rejects the commit.
    pub revision: u64,
    pub kind: PathCommitKind,
}

#[derive(Default, Clone)]
pub(super) struct PathStyle {
    pub fill: Option<Vec<PenFill>>,
    pub stroke: Option<PenStroke>,
    pub effects: Option<Vec<PenEffect>>,
}

impl EditorState {
    /// Apply a [`PathCommit`]; `Ok(Some(id))` with the new path selected,
    /// `Ok(None)` when it does not apply (see the module docs).
    pub(super) fn commit_path_result(
        &mut self,
        commit: PathCommit,
    ) -> Result<Option<NodeId>, IdAllocError> {
        let PathCommit {
            source_ids,
            subpaths,
            even_odd,
            origin,
            new_id,
            revision,
            kind,
        } = commit;
        let usable: Vec<Subpath> = subpaths
            .into_iter()
            .filter(|s| s.anchors.len() >= 2)
            .collect();
        if self.revision != revision
            || usable.is_empty()
            || !new_id.is_real()
            || source_ids.is_empty()
            || self.collect_node_ids().contains(&new_id)
            || source_ids.iter().any(|id| {
                find_node(self.active_children(), id).is_none_or(|node| {
                    !node.is_group() && node.children().is_some_and(|kids| !kids.is_empty())
                })
            })
        {
            return Ok(None);
        }
        let first = find_node(self.active_children(), &source_ids[0]).cloned();
        let Some(first) = first else {
            return Ok(None);
        };
        let Some((parent, index)) = find_parent_and_index(self.active_children(), &source_ids[0])
        else {
            return Ok(None);
        };
        let source_style = node_paint_style(&first);
        let keep_source = kind == PathCommitKind::OutlineStroke
            && source_style.fill.as_ref().is_some_and(|f| !f.is_empty());
        let style = match kind {
            PathCommitKind::OutlineStroke => PathStyle {
                fill: source_style.stroke.as_ref().and_then(|s| s.fill.clone()),
                stroke: None,
                effects: source_style.effects.clone(),
            },
            _ => source_style,
        };
        let name = match source_ids.len() {
            1 => first
                .base()
                .name
                .clone()
                .unwrap_or_else(|| kind.default_name().to_string()),
            _ => kind.default_name().to_string(),
        };
        let groups = group_offset(self.active_children(), &source_ids[0]);
        let origin = (origin.0 + groups.0, origin.1 + groups.1);
        let local: Vec<Subpath> = usable
            .into_iter()
            .map(|mut s| {
                for a in &mut s.anchors {
                    a.x -= origin.0;
                    a.y -= origin.1;
                }
                s
            })
            .collect();
        let mut node = new_path_node(&new_id, name, style, even_odd);
        if !write_subpaths(&mut node, &local) {
            return Ok(None);
        }
        let snap = self.snapshot_for_history();
        let at = if keep_source { index + 1 } else { index };
        if !insert_into_parent(self.active_children_mut(), parent.as_ref(), Some(at), node) {
            return Ok(None);
        }
        if keep_source {
            if let Some(src) = find_node_mut(self.active_children_mut(), &source_ids[0]) {
                clear_stroke(src);
            }
        } else {
            for id in &source_ids {
                crate::walkers::remove_from_children(self.active_children_mut(), id);
            }
        }
        self.history_push_past(snap);
        self.set_single_selection(new_id.clone());
        Ok(Some(new_id))
    }
}

/// Sum of the `x` / `y` of the groups (and boolean groups) between `id`
/// and its nearest non-group ancestor: a group's offset moves its
/// children, which the frame origin the host measured does not include.
fn group_offset(children: &[PenNode], id: &NodeId) -> (f64, f64) {
    let mut offset = (0.0, 0.0);
    let mut current = id.clone();
    while let Some((Some(parent), _)) = find_parent_and_index(children, &current) {
        match find_node(children, &parent) {
            Some(node) if node.is_group() => {
                offset.0 += node.base().x.unwrap_or(0.0);
                offset.1 += node.base().y.unwrap_or(0.0);
                current = parent;
            }
            _ => break,
        }
    }
    offset
}

fn new_path_node(id: &NodeId, name: String, style: PathStyle, even_odd: bool) -> PenNode {
    PenNode::Path(PathNode {
        corner_radius: None,
        corner_smoothing: None,
        base: PenNodeBase {
            id: id.as_str().to_string(),
            name: Some(name),
            ..Default::default()
        },
        icon_id: None,
        d: None,
        anchors: None,
        closed: Some(true),
        fill_rule: even_odd.then_some(PathFillRule::Evenodd),
        mask: None,
        width: None,
        height: None,
        fill: style.fill,
        stroke: style.stroke,
        effects: style.effects,
        state: None,
        bindings: None,
        events: None,
        lifecycle: None,
        semantics: None,
        gestures: None,
        route: None,
        limits: Default::default(),
    })
}

/// A node's fill, stroke and effects, for the path that replaces it.
pub(super) fn node_paint_style(node: &PenNode) -> PathStyle {
    let container = |c: &jian_ops_schema::node::ContainerProps| PathStyle {
        fill: c.fill.clone(),
        stroke: c.stroke.clone(),
        effects: c.effects.clone(),
    };
    match node {
        PenNode::Frame(n) | PenNode::Script(ScriptNode { frame: n, .. }) => container(&n.container),
        PenNode::Group(n) | PenNode::Boolean(BooleanNode { group: n, .. }) => {
            container(&n.container)
        }
        PenNode::Rectangle(n) => container(&n.container),
        PenNode::Ellipse(n) => PathStyle {
            fill: n.fill.clone(),
            stroke: n.stroke.clone(),
            effects: n.effects.clone(),
        },
        PenNode::Polygon(n) => PathStyle {
            fill: n.fill.clone(),
            stroke: n.stroke.clone(),
            effects: n.effects.clone(),
        },
        PenNode::Path(n) => PathStyle {
            fill: n.fill.clone(),
            stroke: n.stroke.clone(),
            effects: n.effects.clone(),
        },
        PenNode::Line(n) => PathStyle {
            fill: None,
            stroke: n.stroke.clone(),
            effects: n.effects.clone(),
        },
        _ => PathStyle::default(),
    }
}

/// Drop a node's stroke (Outline stroke keeps a filled source's fill).
fn clear_stroke(node: &mut PenNode) {
    match node {
        PenNode::Frame(n) | PenNode::Script(ScriptNode { frame: n, .. }) => {
            n.container.stroke = None
        }
        PenNode::Group(n) | PenNode::Boolean(BooleanNode { group: n, .. }) => {
            n.container.stroke = None
        }
        PenNode::Rectangle(n) => n.container.stroke = None,
        PenNode::Ellipse(n) => n.stroke = None,
        PenNode::Polygon(n) => n.stroke = None,
        PenNode::Path(n) => n.stroke = None,
        PenNode::Line(n) => n.stroke = None,
        _ => {}
    }
}
