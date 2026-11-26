//! Node-addressed interaction writes (#371): MCP `set_interaction` /
//! `remove_interaction` set one trigger's actions on any node, the way
//! the property panel's Interactions rows do for the selection.
//!
//! The write replaces only the named trigger and rebuilds the node's
//! whole `events` block (`interaction_edit` explains why a shallow patch
//! would drop sibling handlers). A node inside a component instance
//! (`instanceId/childId`, a nested path, or the canvas anchor
//! `inst__child`) and an instance root go through the panel's own
//! instance-write scope, so the write lands in the instance's
//! `descendants` and the master keeps working for every other instance.

use jian_ops_schema::events::{ActionList, EventHandlers};
use jian_ops_schema::node::PenNode;

use crate::interaction_edit::InteractionTrigger;
use crate::node_id::NodeId;
use crate::state::EditorState;
use crate::walkers::find_node;

/// One interaction write, applied through
/// [`EditorCommand::Interaction`](crate::EditorCommand).
#[derive(Debug, Clone, PartialEq)]
pub enum InteractionCommand {
    /// Replace `trigger`'s actions on `node_id`; `None` (or an empty
    /// list) removes them. Other triggers on the node stay.
    Set {
        /// An authored id, an `instanceId/childId` path, or a canvas
        /// anchor (`inst__child`).
        node_id: NodeId,
        trigger: InteractionTrigger,
        actions: Option<ActionList>,
        /// `None` writes on the active page; `Some` names the page id
        /// (or legacy index) the node lives on.
        page_id: Option<String>,
    },
}

impl EditorState {
    /// Apply an [`InteractionCommand`] on its page.
    pub(crate) fn apply_interaction_command(&mut self, command: InteractionCommand) -> bool {
        let InteractionCommand::Set {
            node_id,
            trigger,
            actions,
            page_id,
        } = command;
        let Some(page_index) = crate::command_apply::command_page_index(self, page_id.as_deref())
        else {
            return false;
        };
        let original_page_index = self.ui.active_page_index;
        self.ui.active_page_index = page_index;
        let changed = self.set_node_interaction(&node_id, trigger, actions);
        self.ui.active_page_index = original_page_index;
        changed
    }

    /// Replace `trigger`'s actions on the node `address` names on the
    /// active page (`None` removes them). `false` when the node does not
    /// resolve, is locked, or already holds exactly these actions.
    pub fn set_node_interaction(
        &mut self,
        address: &NodeId,
        trigger: InteractionTrigger,
        actions: Option<ActionList>,
    ) -> bool {
        let Some(anchor) = self.interaction_anchor(address) else {
            return false;
        };
        let actions = actions.filter(|list| !list.is_empty());
        let edit = move |handlers: &mut EventHandlers| {
            let slot = trigger.slot(handlers);
            if *slot == actions {
                return false;
            }
            *slot = actions;
            true
        };
        let authored = find_node(self.active_children(), &anchor);
        if matches!(authored, Some(node) if !matches!(node, PenNode::Ref(_))) {
            return self.edit_node_events(&anchor, edit);
        }
        let Some(scope) = self.begin_instance_write(&anchor) else {
            return false;
        };
        let changed = self.edit_node_events(&anchor, edit);
        let routed = self.finish_instance_write(scope);
        changed && routed
    }

    /// The id a write addresses on the active page: the authored id
    /// itself, or the canvas anchor of an instance descendant named by a
    /// path (`inst/child`, `frame/inst/nested/leaf`) or by its anchor.
    pub fn interaction_anchor(&self, address: &NodeId) -> Option<NodeId> {
        if !address.is_real() {
            return None;
        }
        if find_node(self.active_children(), address).is_some() {
            return Some(address.clone());
        }
        let raw = address.as_str();
        if raw.contains(crate::instance_descendants::PATH_SEPARATOR) {
            return crate::instance_anchor::locate_document_path(
                self.active_children(),
                &self.doc,
                raw,
            )
            .map(|hit| hit.anchor);
        }
        crate::instance_anchor::locate_anchor(&self.doc, address).map(|hit| hit.anchor)
    }
}

#[cfg(test)]
#[path = "interaction_write_tests.rs"]
mod tests;
