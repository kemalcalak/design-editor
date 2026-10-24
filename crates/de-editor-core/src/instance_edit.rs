//! `EditorCommand::EditInstanceDescendant` (#166): write one node of a
//! component instance through its `descendants` map.
//!
//! The node is named by its id path inside the instance (`child`,
//! `nested/leaf`, or a node of instance-owned content), resolved with
//! [`crate::instance_anchor::locate_path`]. Patches and inserts land where
//! the panel's own writes land ([`DescendantTarget`]); a replacement stores
//! a complete node tree, a reset removes the override.

use crate::instance_anchor::{locate_path, DescendantTarget, InstanceDescendant};
use crate::instance_descendants::{self as desc, find_owned_node_mut};
use crate::node_id::NodeId;
use crate::pen_node_ext::PenNodeExt;
use crate::state::EditorState;
use crate::walkers::{find_node, find_node_mut};
use jian_ops_schema::node::PenNode;
use serde_json::{Map, Value};

/// What [`EditorCommand::EditInstanceDescendant`](crate::EditorCommand)
/// does to the descendant.
#[derive(Debug, Clone, PartialEq)]
pub enum InstanceEdit {
    /// Shallow-merge these JSON fields onto the node (pen.dev `Update`).
    Patch { patch_json: String },
    /// Swap the node and its subtree for `node` (pen.dev `Replace`). The
    /// node's ids are kept as given: the caller mints them.
    Replace { node: Box<PenNode> },
    /// Add children to a container descendant (pen.dev `Insert` into a
    /// slot). The first insert into a master node replaces the master's
    /// children with the instance's own; later inserts add to them.
    InsertChildren {
        nodes: Vec<PenNode>,
        index: Option<usize>,
    },
    /// Remove the override, restoring the master's node.
    Reset,
}

impl EditorState {
    /// Apply `EditorCommand::EditInstanceDescendant` on its page (any other
    /// command is a no-op `false`).
    pub(crate) fn cmd_instance_edit(&mut self, cmd: crate::EditorCommand) -> bool {
        let crate::EditorCommand::EditInstanceDescendant {
            instance_id,
            path,
            edit,
            page_id,
        } = cmd
        else {
            return false;
        };
        let Some(page_index) = crate::command_apply::command_page_index(self, page_id.as_deref())
        else {
            return false;
        };
        let original_page_index = self.ui.active_page_index;
        if page_id.is_some() {
            self.ui.active_page_index = page_index;
        }
        let changed = self.cmd_edit_instance_descendant(&instance_id, &path, edit);
        self.ui.active_page_index = original_page_index;
        changed
    }

    /// The instance and `descendants` key that hold an override for the
    /// node behind a canvas anchor, `None` when the node shows the master
    /// as it is (nothing to reset).
    pub fn instance_override_at_anchor(&self, anchor: &NodeId) -> Option<(NodeId, String)> {
        let hit = crate::instance_anchor::locate_anchor(&self.doc, anchor)?;
        let PenNode::Ref(reference) = find_node(self.active_children(), &hit.ref_id)? else {
            return None;
        };
        let key = hit.target.entry_key();
        reference
            .descendants
            .as_ref()?
            .contains_key(key)
            .then(|| (hit.ref_id.clone(), key.to_string()))
    }

    /// Reset the override of the selected instance descendant (one undo
    /// step), restoring the master's node. The selection stays on the
    /// anchor when it still renders, else moves to the instance.
    pub fn reset_selected_instance_override(&mut self) -> bool {
        let anchor = self.selection.anchor.clone();
        let Some((instance_id, key)) = self.instance_override_at_anchor(&anchor) else {
            return false;
        };
        let before = self.snapshot_for_history();
        let reset = self.apply(crate::EditorCommand::EditInstanceDescendant {
            instance_id: instance_id.clone(),
            path: key,
            edit: InstanceEdit::Reset,
            page_id: None,
        });
        if !reset {
            return false;
        }
        self.history_push_past(before);
        if crate::instance_anchor::locate_anchor(&self.doc, &anchor).is_none() {
            self.set_single_selection(instance_id);
        }
        true
    }

    /// Apply one instance-descendant edit on the active page. `false` when
    /// the instance or path does not resolve, the instance is locked or
    /// hidden, or the edit does not fit the node.
    pub(crate) fn cmd_edit_instance_descendant(
        &mut self,
        instance_id: &NodeId,
        path: &str,
        edit: InstanceEdit,
    ) -> bool {
        let Some(ref_node @ PenNode::Ref(_)) = find_node(self.active_children(), instance_id)
        else {
            return false;
        };
        let base = ref_node.base();
        if !base.visible.unwrap_or(true) || base.locked.unwrap_or(false) {
            return false;
        }
        let hit = locate_path(&self.doc, ref_node, path);
        let Ok(Value::Object(mut ref_map)) = serde_json::to_value(ref_node) else {
            return false;
        };
        let descendants = desc::descendants_mut(&mut ref_map);
        let changed = match (edit, hit) {
            (InstanceEdit::Reset, hit) => {
                let key = hit
                    .as_ref()
                    .map_or(path.to_string(), |hit| hit.target.entry_key().to_string());
                desc::reset_entry(descendants, &key)
            }
            (_, None) => false,
            (InstanceEdit::Patch { patch_json }, Some(hit)) => {
                match serde_json::from_str::<Value>(&patch_json) {
                    Ok(Value::Object(patch)) if !patch.is_empty() => {
                        hit.target.write_patch(descendants, patch);
                        true
                    }
                    _ => false,
                }
            }
            (InstanceEdit::Replace { node }, Some(hit)) => serde_json::to_value(node.as_ref())
                .is_ok_and(|value| replace_descendant(descendants, &hit.target, value)),
            (InstanceEdit::InsertChildren { nodes, index }, Some(hit)) => {
                let values: Option<Vec<Value>> = nodes
                    .iter()
                    .map(|node| serde_json::to_value(node).ok())
                    .collect();
                values.is_some_and(|values| insert_children(descendants, &hit, values, index))
            }
        };
        if !changed {
            return false;
        }
        if descendants.is_empty() {
            ref_map.remove("descendants");
        }
        let Ok(updated @ PenNode::Ref(_)) =
            serde_json::from_value::<PenNode>(Value::Object(ref_map))
        else {
            return false;
        };
        let Some(slot) = find_node_mut(self.active_children_mut(), instance_id) else {
            return false;
        };
        *slot = updated;
        true
    }
}

/// Store `node` as the replacement of the target: a master node's entry
/// becomes the node, an owned node is swapped inside its entry.
fn replace_descendant(
    descendants: &mut Map<String, Value>,
    target: &DescendantTarget,
    node: Value,
) -> bool {
    if !node.get("type").is_some_and(Value::is_string) {
        return false;
    }
    match target {
        DescendantTarget::Patch { key } => {
            descendants.insert(key.clone(), node);
            true
        }
        DescendantTarget::Owned { entry_key, node_id } => {
            let Some(entry) = descendants.get_mut(entry_key) else {
                return false;
            };
            let is_root = desc::is_replacement(entry)
                && desc::replacement_root_id(entry, entry_key) == node_id;
            if is_root {
                *entry = node;
                return true;
            }
            match find_owned_node_mut(entry, entry_key, node_id) {
                Some(slot) => {
                    *slot = node;
                    true
                }
                None => false,
            }
        }
    }
}

/// Append (or insert at `index`) children to a container descendant.
fn insert_children(
    descendants: &mut Map<String, Value>,
    hit: &InstanceDescendant,
    nodes: Vec<Value>,
    index: Option<usize>,
) -> bool {
    if nodes.is_empty() || hit.is_instance || !is_container(&hit.display) {
        return false;
    }
    let children = match &hit.target {
        DescendantTarget::Patch { key } => {
            let entry = descendants
                .entry(key.clone())
                .or_insert_with(|| Value::Object(Map::new()));
            let Value::Object(entry) = entry else {
                return false;
            };
            // The instance's own content replaces the master's children.
            entry
                .entry("children")
                .or_insert_with(|| Value::Array(Vec::new()))
        }
        DescendantTarget::Owned { entry_key, node_id } => {
            let Some(Value::Object(node)) = descendants
                .get_mut(entry_key)
                .and_then(|entry| find_owned_node_mut(entry, entry_key, node_id))
            else {
                return false;
            };
            node.entry("children")
                .or_insert_with(|| Value::Array(Vec::new()))
        }
    };
    let Value::Array(children) = children else {
        return false;
    };
    let at = index.unwrap_or(children.len()).min(children.len());
    children.splice(at..at, nodes);
    true
}

fn is_container(node: &PenNode) -> bool {
    matches!(
        node,
        PenNode::Frame(_) | PenNode::Group(_) | PenNode::Rectangle(_)
    )
}

#[cfg(test)]
#[path = "instance_edit_tests.rs"]
mod tests;
