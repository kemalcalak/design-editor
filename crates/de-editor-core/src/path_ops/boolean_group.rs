//! Boolean groups: create one around same-parent nodes, switch its op.

use super::commit::node_paint_style;
use super::tree_position;
use crate::node_id::NodeId;
use crate::state::EditorState;
use crate::walkers::{find_node, find_node_mut, find_parent_and_index, insert_into_parent};
use crate::BooleanOp;
use jian_ops_schema::node::{BooleanNode, ContainerProps, GroupNode, PenNode, PenNodeBase};

/// The layer name a new boolean group gets.
pub fn boolean_group_name(op: BooleanOp) -> &'static str {
    match op {
        BooleanOp::Union => "Union",
        BooleanOp::Subtract => "Subtract",
        BooleanOp::Intersect => "Intersect",
        BooleanOp::Exclude => "Exclude",
    }
}

impl EditorState {
    /// Wrap `node_ids` (two or more, one parent, active page) in a new
    /// boolean group `new_id` at the bottom node's slot. The group takes
    /// the bottom node's fill, stroke and effects (its children keep
    /// theirs, unpainted while grouped). One undo step; the group ends up
    /// selected. `None` when the ids do not qualify or `new_id` is taken.
    pub(super) fn create_boolean_group(
        &mut self,
        node_ids: &[NodeId],
        op: BooleanOp,
        new_id: NodeId,
    ) -> Option<NodeId> {
        if node_ids.len() < 2 || !new_id.is_real() || self.collect_node_ids().contains(&new_id) {
            return None;
        }
        let mut placed: Vec<(Vec<usize>, NodeId)> = Vec::with_capacity(node_ids.len());
        let mut parent: Option<Option<NodeId>> = None;
        for id in node_ids {
            let (p, _) = find_parent_and_index(self.active_children(), id)?;
            if parent.as_ref().is_some_and(|q| *q != p) {
                return None;
            }
            parent = Some(p);
            placed.push((tree_position(self.active_children(), id)?, id.clone()));
        }
        placed.sort_by(|a, b| a.0.cmp(&b.0));
        placed.dedup_by(|a, b| a.1 == b.1);
        if placed.len() < 2 {
            return None;
        }
        let parent = parent.flatten();
        let first_index = *placed[0].0.last()?;
        let style = node_paint_style(find_node(self.active_children(), &placed[0].1)?);
        let snap = self.snapshot_for_history();
        let mut children = Vec::with_capacity(placed.len());
        for (_, id) in &placed {
            match crate::walkers::extract_node(self.active_children_mut(), id) {
                Some(node) => children.push(node),
                None => {
                    self.restore(snap);
                    return None;
                }
            }
        }
        let node = PenNode::Boolean(BooleanNode {
            group: GroupNode {
                base: PenNodeBase {
                    id: new_id.as_str().to_string(),
                    name: Some(boolean_group_name(op).to_string()),
                    ..Default::default()
                },
                container: ContainerProps {
                    fill: style.fill,
                    stroke: style.stroke,
                    effects: style.effects,
                    ..Default::default()
                },
                children: Some(children),
                state: None,
                bindings: None,
                events: None,
                lifecycle: None,
                semantics: None,
                gestures: None,
                route: None,
            },
            op,
        });
        if !insert_into_parent(
            self.active_children_mut(),
            parent.as_ref(),
            Some(first_index),
            node,
        ) {
            // The parent vanished mid-way: put the document back.
            self.restore(snap);
            return None;
        }
        self.history_push_past(snap);
        self.set_single_selection(new_id.clone());
        Some(new_id)
    }

    /// Set a boolean group's operation. False when `node_id` is not a
    /// boolean group or already uses `op`.
    pub(super) fn set_boolean_op(&mut self, node_id: &NodeId, op: BooleanOp) -> bool {
        let current = match find_node(self.active_children(), node_id) {
            Some(PenNode::Boolean(b)) => b.op,
            _ => return false,
        };
        if current == op {
            return false;
        }
        let snap = self.snapshot_for_history();
        if let Some(PenNode::Boolean(b)) = find_node_mut(self.active_children_mut(), node_id) {
            b.op = op;
            if b.group.base.name.as_deref() == Some(boolean_group_name(current)) {
                b.group.base.name = Some(boolean_group_name(op).to_string());
            }
        }
        self.history_push_past(snap);
        true
    }

    /// The operation of the boolean group `node_id`, if it is one.
    pub fn boolean_op_of(&self, node_id: &NodeId) -> Option<BooleanOp> {
        match find_node(self.active_children(), node_id)? {
            PenNode::Boolean(b) => Some(b.op),
            _ => None,
        }
    }

    /// True when `node_id` is a boolean group's child (on the active page).
    pub fn is_boolean_child(&self, node_id: &NodeId) -> bool {
        let Some((Some(parent), _)) = find_parent_and_index(self.active_children(), node_id) else {
            return false;
        };
        matches!(
            find_node(self.active_children(), &parent),
            Some(PenNode::Boolean(_))
        )
    }
}
