//! `instanceId/childId` paths: the address MCP readers and writers use for a
//! node inside a component instance (#169, pen.dev's `Get` / `Update` path
//! syntax).
//!
//! The canvas names an expanded instance child with the render-only virtual
//! id `refId__childId` (`ref_resolve::instance_child_virtual_id`). Paths are
//! the public spelling of the same thing: `read_nodes` with
//! `resolveInstances` reports them, and `update_node` / `batch_design`'s
//! `U()` accept them and route the write as an instance override on the
//! authored `Ref` (`descendants[childId]`), the way the property panel does.
//!
//! Writes take exactly two segments. A nested instance child
//! (`outer/inner/leaf`) has no single authored `Ref` to receive the override,
//! so it is reported but stays read-only, like the canvas
//! (`split_instance_child_anchor`).

use crate::instance_override::split_instance_child_anchor;
use crate::node_id::NodeId;
use crate::pen_node_ext::PenNodeExt;
use crate::ref_resolve::{instance_child_virtual_id, INSTANCE_CHILD_SEPARATOR};
use crate::state::EditorState;
use crate::walkers;
use jian_ops_schema::node::PenNode;
use jian_ops_schema::PenDocument;
use std::collections::HashSet;

/// Separator between the segments of an instance path.
pub const INSTANCE_PATH_SEPARATOR: char = '/';

/// Patch keys an instance-path write refuses: they would rename, retype or
/// restructure the swapped-in display node, and the override router could
/// then not find it again to restore the `Ref`.
const STRUCTURAL_PATCH_KEYS: &[&str] = &["id", "type", "children", "ref", "descendants"];

/// The canvas virtual anchor (`refId__childId`) for a two-segment instance
/// path whose first segment is an authored `Ref` and whose second is a real
/// descendant of its component. `None` for anything else, including nested
/// paths and ids that merely contain a slash.
pub fn instance_path_anchor(doc: &PenDocument, path: &str) -> Option<NodeId> {
    let (ref_id, child_id) = path.split_once(INSTANCE_PATH_SEPARATOR)?;
    if ref_id.is_empty() || child_id.is_empty() || child_id.contains(INSTANCE_PATH_SEPARATOR) {
        return None;
    }
    let anchor = NodeId::new_opt(instance_child_virtual_id(ref_id, child_id))?;
    split_instance_child_anchor(&anchor, doc).map(|_| anchor)
}

/// Whether `patch_json` (a JSON object) carries a key an instance-path
/// write must refuse.
pub fn patch_touches_structure(patch: &serde_json::Map<String, serde_json::Value>) -> bool {
    STRUCTURAL_PATCH_KEYS
        .iter()
        .any(|key| patch.contains_key(*key))
}

impl EditorState {
    /// The virtual anchor `node_id` names when it is an instance path and
    /// not an authored id on the active page. Write commands call this
    /// before their own lookup.
    pub(crate) fn instance_path_write_anchor(&self, node_id: &NodeId) -> Option<NodeId> {
        let raw = node_id.as_str();
        if !raw.contains(INSTANCE_PATH_SEPARATOR)
            || walkers::find_node(self.active_children(), node_id).is_some()
        {
            return None;
        }
        instance_path_anchor(&self.doc, raw)
    }
}

/// Expand every component instance in `nodes` into its full subtree (the
/// canvas expander, `ref_resolve::resolve_refs_for_canvas_roots`, against
/// the whole `doc`) and name each expanded node by its instance path:
/// `card__title` becomes `card/title`, a nested `card__badge__label` becomes
/// `card/badge/label`. Nodes outside any instance keep their ids.
pub fn expand_instances_with_paths(nodes: &[PenNode], doc: &PenDocument) -> Vec<PenNode> {
    // Every authored Ref id in the document: instance roots in `nodes`, and
    // the Refs inside components that expand into nested instances.
    let mut refs = HashSet::new();
    if let Some(pages) = doc.pages.as_ref() {
        for page in pages {
            collect_ref_ids(&page.children, &mut refs);
        }
    }
    collect_ref_ids(&doc.children, &mut refs);
    collect_ref_ids(nodes, &mut refs);
    let mut expanded = crate::ref_resolve::resolve_refs_for_canvas_roots(nodes, doc);
    for node in &mut expanded {
        rename_node(node, None, &refs);
    }
    expanded
}

fn collect_ref_ids(nodes: &[PenNode], out: &mut HashSet<String>) {
    for node in nodes {
        if matches!(node, PenNode::Ref(_)) {
            out.insert(node.id_str().to_string());
        }
        if let Some(children) = node.children() {
            collect_ref_ids(children, out);
        }
    }
}

/// `scope` is the innermost enclosing instance: its virtual id and its path.
/// A node is an instance root when its authored id (the whole id outside an
/// instance, the part after the scope's prefix inside one) names an authored
/// `Ref`.
fn rename_node(node: &mut PenNode, scope: Option<(&str, &str)>, refs: &HashSet<String>) {
    let virtual_id = node.id_str().to_string();
    let authored = match scope {
        Some((scope_virtual, _)) => virtual_id
            .strip_prefix(scope_virtual)
            .and_then(|rest| rest.strip_prefix(INSTANCE_CHILD_SEPARATOR)),
        None => Some(virtual_id.as_str()),
    };
    let path = match (scope, authored) {
        (Some((_, scope_path)), Some(rest)) => {
            format!("{scope_path}{INSTANCE_PATH_SEPARATOR}{rest}")
        }
        _ => virtual_id.clone(),
    };
    let is_instance = authored.is_some_and(|id| refs.contains(id));
    if path != virtual_id {
        node.base_mut().id = path.clone();
    }
    let Some(children) = node.children_mut() else {
        return;
    };
    let child_scope = if is_instance {
        Some((virtual_id.as_str(), path.as_str()))
    } else {
        scope
    };
    for child in children {
        rename_node(child, child_scope, refs);
    }
}

#[cfg(test)]
#[path = "instance_path_tests.rs"]
mod tests;
