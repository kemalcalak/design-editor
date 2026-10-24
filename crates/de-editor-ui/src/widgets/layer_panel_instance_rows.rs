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
        let replaced = is_replacement(entry);
        if !replaced && !owns_children(entry) {
            continue;
        }
        // An entry for a node the master no longer has renders nothing, so
        // it gets no row either. The hit's anchor is the canvas's own.
        let Some(hit) = de_editor_core::instance_anchor::locate_path(cx.doc, ref_node, key) else {
            continue;
        };
        let own_id = if replaced {
            replacement_root_id(entry, key)
        } else {
            key.rsplit_once(PATH_SEPARATOR)
                .map_or(key.as_str(), |(_, last)| last)
        };
        let Some(prefix) = hit
            .anchor
            .as_str()
            .strip_suffix(own_id)
            .and_then(|rest| rest.strip_suffix("__"))
        else {
            continue;
        };
        if replaced {
            let mut root = entry.clone();
            if root.get("id").and_then(Value::as_str).is_none() {
                root["id"] = Value::String(own_id.to_string());
            }
            if let Ok(node) = serde_json::from_value::<PenNode>(root) {
                push_subtree(&node, prefix, cx, depth, out);
            }
        } else {
            push_slot(
                &hit.display,
                hit.anchor.as_str(),
                own_id,
                entry,
                prefix,
                cx,
                depth,
                out,
            );
        }
    }
}

/// A filled slot: the master's slot frame as a row, the instance's own
/// children under it.
#[allow(clippy::too_many_arguments)]
fn push_slot(
    display: &PenNode,
    anchor: &str,
    slot_id: &str,
    entry: &Value,
    prefix: &str,
    cx: &WalkCx<'_>,
    depth: usize,
    out: &mut Vec<LayerItem>,
) {
    let label = display
        .base()
        .name
        .clone()
        .unwrap_or_else(|| slot_id.to_string());
    let mut item = owned_item(display, anchor.to_string(), label, cx, depth);
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
    let item = owned_item(node, anchor, label, cx, depth);
    let collapsed = item.collapsed;
    out.push(item);
    if collapsed {
        return;
    }
    for child in node.children().into_iter().flatten() {
        push_subtree(child, prefix, cx, depth + 1, out);
    }
}

/// One row for a node an instance renders. Eye / lock clicks reach the
/// node through `toggle_node_hidden` / `toggle_node_locked`, which store the
/// flag as the instance's override.
fn owned_item(
    node: &PenNode,
    anchor: String,
    label: String,
    cx: &WalkCx<'_>,
    depth: usize,
) -> LayerItem {
    let id = NodeId::new(anchor);
    let base = node.base();
    LayerItem {
        collapsed: cx.ui.collapsed_layers.contains(&id),
        node_id: id,
        label,
        kind_label: kind_label(node).to_string(),
        icon: icon_for_node(node),
        depth,
        has_children: node.children().is_some_and(|c| !c.is_empty()),
        hidden: base.visible == Some(false),
        locked: base.locked.unwrap_or(false),
        renaming: false,
        // Instance content moves only through the instance's overrides.
        is_container: false,
        is_reusable: false,
        is_instance: matches!(node, PenNode::Ref(_)),
    }
}

#[cfg(test)]
#[path = "layer_panel_instance_rows_tests.rs"]
mod tests;
