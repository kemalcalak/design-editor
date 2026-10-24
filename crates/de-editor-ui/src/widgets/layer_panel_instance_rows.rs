//! Layer rows for an instance's own content (#166): the subtrees it
//! replaced and the children it put into slots. Master content stays out
//! of the tree (as before); what the instance owns is listed under its row
//! so it can be selected, inspected and edited there. Rows carry the
//! canvas anchors (`inst__node`, `inst__nested__node`) the property panel
//! already resolves, and are never drop targets.

use de_editor_core::instance_descendants::{
    is_replacement, owns_children, replacement_root_id, PATH_SEPARATOR,
};
use de_editor_core::pen_node_ext::PenNodeExt;
use de_editor_core::NodeId;
use jian_ops_schema::node::{PenNode, RefNode};
use serde_json::Value;

use super::layer_panel::LayerItem;
use super::layer_panel_walkers::{icon_for_node, kind_label, WalkCx};

/// True when the instance owns content that gets rows of its own.
pub(super) fn has_owned_rows(reference: &RefNode) -> bool {
    reference
        .descendants
        .as_ref()
        .is_some_and(|d| d.values().any(|e| is_replacement(e) || owns_children(e)))
}

/// Append the rows of `reference`'s owned content at `depth`.
pub(super) fn push_owned_rows(
    reference: &RefNode,
    ref_node: &PenNode,
    cx: &WalkCx<'_>,
    depth: usize,
    out: &mut Vec<LayerItem>,
) {
    let Some(descendants) = reference.descendants.as_ref() else {
        return;
    };
    for (key, entry) in descendants {
        let prefix = virtual_prefix(&reference.base.id, key);
        if is_replacement(entry) {
            let mut root = entry.clone();
            if root.get("id").and_then(Value::as_str).is_none() {
                root["id"] = Value::String(replacement_root_id(entry, key).to_string());
            }
            if let Ok(node) = serde_json::from_value::<PenNode>(root) {
                push_subtree(&node, &prefix, cx, depth, out);
            }
        } else if owns_children(entry) {
            push_slot(ref_node, key, entry, &prefix, cx, depth, out);
        }
    }
}

/// The anchor prefix of the nodes an entry at `key` holds: the instance id,
/// plus one `__segment` per nested instance the key passes through.
fn virtual_prefix(ref_id: &str, key: &str) -> String {
    let mut prefix = ref_id.to_string();
    if let Some((nested, _)) = key.rsplit_once(PATH_SEPARATOR) {
        for segment in nested.split(PATH_SEPARATOR) {
            prefix = de_editor_core::ref_resolve::instance_child_virtual_id(&prefix, segment);
        }
    }
    prefix
}

/// A filled slot: the master's slot frame as a row, the instance's own
/// children under it.
fn push_slot(
    ref_node: &PenNode,
    key: &str,
    entry: &Value,
    prefix: &str,
    cx: &WalkCx<'_>,
    depth: usize,
    out: &mut Vec<LayerItem>,
) {
    let slot_id = key
        .rsplit_once(PATH_SEPARATOR)
        .map_or(key, |(_, last)| last);
    let anchor = de_editor_core::ref_resolve::instance_child_virtual_id(prefix, slot_id);
    let display =
        de_editor_core::instance_anchor::locate_path(cx.doc, ref_node, key).map(|hit| hit.display);
    let label = display
        .as_ref()
        .and_then(|node| node.base().name.clone())
        .unwrap_or_else(|| slot_id.to_string());
    let mut item = owned_item(display.as_ref(), anchor, label, cx, depth);
    item.has_children = true;
    let collapsed = item.collapsed;
    out.push(item);
    if collapsed {
        return;
    }
    let children = entry.get("children").and_then(Value::as_array);
    for child in children.into_iter().flatten() {
        if let Ok(node) = serde_json::from_value::<PenNode>(child.clone()) {
            push_subtree(&node, prefix, cx, depth + 1, out);
        }
    }
}

fn push_subtree(
    node: &PenNode,
    prefix: &str,
    cx: &WalkCx<'_>,
    depth: usize,
    out: &mut Vec<LayerItem>,
) {
    let anchor = de_editor_core::ref_resolve::instance_child_virtual_id(prefix, node.id_str());
    let label = node
        .base()
        .name
        .clone()
        .unwrap_or_else(|| kind_label(node).to_string());
    let item = owned_item(Some(node), anchor, label, cx, depth);
    let collapsed = item.collapsed;
    out.push(item);
    if collapsed {
        return;
    }
    for child in node.children().into_iter().flatten() {
        push_subtree(child, prefix, cx, depth + 1, out);
    }
}

fn owned_item(
    node: Option<&PenNode>,
    anchor: String,
    label: String,
    cx: &WalkCx<'_>,
    depth: usize,
) -> LayerItem {
    let id = NodeId::new(anchor);
    LayerItem {
        collapsed: cx.ui.collapsed_layers.contains(&id),
        node_id: id,
        label,
        kind_label: node.map_or("Frame", kind_label).to_string(),
        icon: node.map_or(super::icons::Icon::Hash, icon_for_node),
        depth,
        has_children: node
            .and_then(PenNodeExt::children)
            .is_some_and(|c| !c.is_empty()),
        hidden: node.is_some_and(|n| n.base().visible == Some(false)),
        locked: false,
        renaming: false,
        // Instance content moves only through the instance's overrides.
        is_container: false,
        is_reusable: false,
        is_instance: node.is_some_and(|n| matches!(n, PenNode::Ref(_))),
    }
}

#[cfg(test)]
#[path = "layer_panel_instance_rows_tests.rs"]
mod tests;
