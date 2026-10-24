//! Locate a node inside a component instance (#166).
//!
//! An instance's subtree exists only after expansion, so a node in it is
//! named two ways: by its canvas anchor (the render-only virtual id
//! `inst__child`, `inst__nested__leaf`, …) or by an id path
//! (`inst/child`, `inst/nested/leaf`, pen.dev's MCP spelling). Both resolve
//! here to the authored instance plus the [`DescendantTarget`] an edit
//! writes to, walking the same override rules the renderer applies
//! ([`crate::instance_descendants`]).

use std::collections::BTreeSet;

use crate::instance_descendants::{self as desc, PATH_SEPARATOR};
use crate::node_id::NodeId;
use crate::pen_node_ext::PenNodeExt;
use jian_ops_schema::node::PenNode;
use jian_ops_schema::PenDocument;
use serde_json::{Map, Value};

/// Where an edit of an instance descendant is stored on the authored
/// instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DescendantTarget {
    /// A patch in `descendants[key]` (a master node, a nested instance's
    /// root, or the slot node that holds instance-owned children).
    Patch { key: String },
    /// A node inside instance-owned content (a replacement subtree or slot
    /// children) stored in `descendants[entry_key]`; edits go into that
    /// entry in place.
    Owned { entry_key: String, node_id: String },
}

impl DescendantTarget {
    /// The `descendants` key that holds this node's override, the one a
    /// reset removes.
    pub fn entry_key(&self) -> &str {
        match self {
            Self::Patch { key } => key,
            Self::Owned { entry_key, .. } => entry_key,
        }
    }

    /// Store a patch for this node: merged into `descendants[key]` for a
    /// master node, written in place for a node of instance-owned content
    /// (falling back to a patch keyed by its path when the entry no longer
    /// holds it). A `null` value clears the key.
    pub fn write_patch(&self, descendants: &mut Map<String, Value>, patch: Map<String, Value>) {
        if let Self::Owned { entry_key, node_id } = self {
            if let Some(entry) = descendants.get_mut(entry_key) {
                if desc::patch_owned_node(entry, entry_key, node_id, &patch) {
                    return;
                }
            }
        }
        let entry = descendants
            .entry(self.path())
            .or_insert_with(|| Value::Object(Map::new()));
        if !entry.is_object() {
            *entry = Value::Object(Map::new());
        }
        if let Value::Object(existing) = entry {
            existing.extend(patch);
        }
    }

    /// The node's id path relative to the instance (`child`, `nested/leaf`).
    pub fn path(&self) -> String {
        match self {
            Self::Patch { key } => key.clone(),
            Self::Owned { entry_key, node_id } => match entry_key.rsplit_once(PATH_SEPARATOR) {
                Some((prefix, _)) => format!("{prefix}{PATH_SEPARATOR}{node_id}"),
                None => node_id.clone(),
            },
        }
    }
}

/// One located instance descendant.
#[derive(Debug, Clone)]
pub struct InstanceDescendant {
    /// The authored instance that stores the override.
    pub ref_id: NodeId,
    pub target: DescendantTarget,
    /// The canvas anchor (virtual id) of the node.
    pub anchor: NodeId,
    /// The effective node with original ids and every override applied;
    /// a nested instance is its merged display node.
    pub display: PenNode,
    /// True when the node is a nested instance (its children come from its
    /// own component, so it holds no children of its own).
    pub is_instance: bool,
}

/// How the walk recognises the node it looks for.
enum Wanted<'a> {
    Anchor(&'a str),
    Path(&'a str),
}

/// Walk state for one instance scope (the outer instance or a nested one).
struct Scope<'a> {
    doc: &'a PenDocument,
    /// The outer instance's own `descendants`: owned content is editable
    /// only when its entry lives there.
    outer: &'a Map<String, Value>,
    virtual_prefix: String,
    key_prefix: String,
    /// Owned entry the current subtree belongs to, if any.
    owned_by: Option<String>,
}

/// Resolve a canvas anchor (`inst__child`) to the node it names. Authored
/// nodes win: an anchor that is a real document id is not virtual.
pub fn locate_anchor(doc: &PenDocument, anchor: &NodeId) -> Option<InstanceDescendant> {
    let raw = anchor.as_str();
    if !anchor.is_real() || !raw.contains("__") || find_authored(doc, raw).is_some() {
        return None;
    }
    // Every adjacent underscore pair is a candidate boundary (`inst___icon`
    // splits as `inst_` + `icon`). An anchor naming nodes in two instances is
    // ambiguous and stays unresolved.
    let mut found: Option<InstanceDescendant> = None;
    for separator in raw
        .as_bytes()
        .windows(2)
        .enumerate()
        .filter_map(|(index, pair)| (pair == b"__").then_some(index))
    {
        let Some(ref_node @ PenNode::Ref(_)) = find_authored(doc, &raw[..separator]) else {
            continue;
        };
        let Some(hit) = locate_in(doc, ref_node, &Wanted::Anchor(raw)) else {
            continue;
        };
        if found.as_ref().is_some_and(|f| f.ref_id != hit.ref_id) {
            return None;
        }
        found = Some(hit);
    }
    found
}

/// Resolve `path` (`descendant` or `nested/leaf`) inside the authored
/// instance `ref_node`.
pub fn locate_path(
    doc: &PenDocument,
    ref_node: &PenNode,
    path: &str,
) -> Option<InstanceDescendant> {
    locate_in(doc, ref_node, &Wanted::Path(path))
}

/// Resolve an MCP id path whose first segment is an authored node and whose
/// rest walks into the instance it names: `inst/child`, `frame/inst/child`
/// (the leading segments are plain document children, as in
/// `find_node_by_path`), `inst/nested/leaf`. `None` when no segment is an
/// instance with segments left after it.
pub fn locate_document_path(
    roots: &[PenNode],
    doc: &PenDocument,
    path: &str,
) -> Option<InstanceDescendant> {
    let mut parts = path.split(PATH_SEPARATOR);
    let first = parts.next()?;
    let mut current = crate::walkers::find_node(roots, &NodeId::new_opt(first)?)?;
    let rest: Vec<&str> = parts.collect();
    for (index, part) in rest.iter().enumerate() {
        if matches!(current, PenNode::Ref(_)) {
            return locate_path(doc, current, &rest[index..].join("/"));
        }
        current = current.children()?.iter().find(|c| c.id_str() == *part)?;
    }
    None
}

fn locate_in(
    doc: &PenDocument,
    ref_node: &PenNode,
    wanted: &Wanted<'_>,
) -> Option<InstanceDescendant> {
    let PenNode::Ref(reference) = ref_node else {
        return None;
    };
    let ref_value = serde_json::to_value(ref_node).ok()?;
    let empty = Map::new();
    let outer = ref_value
        .get("descendants")
        .and_then(Value::as_object)
        .unwrap_or(&empty);
    let scope = Scope {
        doc,
        outer,
        virtual_prefix: reference.base.id.clone(),
        key_prefix: String::new(),
        owned_by: None,
    };
    let mut visited = BTreeSet::from([reference.target.clone()]);
    let (target, anchor, display, is_instance) =
        walk_instance(&scope, &ref_value, wanted, &mut visited)?;
    Some(InstanceDescendant {
        ref_id: NodeId::new_opt(&reference.base.id)?,
        target,
        anchor: NodeId::new_opt(&anchor)?,
        display,
        is_instance,
    })
}

type Hit = (DescendantTarget, String, PenNode, bool);

/// Walk the children an instance renders: its component's children, or its
/// own inline children when the master is an empty shell.
fn walk_instance(
    scope: &Scope<'_>,
    ref_value: &Value,
    wanted: &Wanted<'_>,
    visited: &mut BTreeSet<String>,
) -> Option<Hit> {
    let target = ref_value.get("ref").and_then(Value::as_str)?;
    let component = crate::ref_resolve::find_component_node(scope.doc, target)?;
    let component_value = serde_json::to_value(&component).ok()?;
    let children = match component_value.get("children") {
        Some(Value::Array(children)) if !children.is_empty() => children.clone(),
        _ => ref_value
            .get("children")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default(),
    };
    let overrides = ref_value.get("descendants").and_then(Value::as_object);
    walk_children(scope, &children, overrides, wanted, visited)
}

fn walk_children(
    scope: &Scope<'_>,
    children: &[Value],
    overrides: Option<&Map<String, Value>>,
    wanted: &Wanted<'_>,
    visited: &mut BTreeSet<String>,
) -> Option<Hit> {
    children
        .iter()
        .find_map(|child| walk_node(scope, child, overrides, wanted, visited))
}

fn walk_node(
    scope: &Scope<'_>,
    raw: &Value,
    overrides: Option<&Map<String, Value>>,
    wanted: &Wanted<'_>,
    visited: &mut BTreeSet<String>,
) -> Option<Hit> {
    let original_id = raw.get("id").and_then(Value::as_str)?;
    // This node's own entry only: the entries below it are applied for the
    // node that matches (a whole-subtree merge per node is quadratic).
    let effective = desc::apply_entry(raw, overrides);
    let id = effective.get("id").and_then(Value::as_str)?.to_string();
    let key = desc::join_key(&scope.key_prefix, original_id);
    let entry = scope.outer.get(&key).filter(|_| scope.owned_by.is_none());
    let replaced_here = entry.is_some_and(desc::is_replacement);
    let fills_here = entry.is_some_and(desc::owns_children);
    let target = match (&scope.owned_by, replaced_here) {
        (Some(entry_key), _) => DescendantTarget::Owned {
            entry_key: entry_key.clone(),
            node_id: id.clone(),
        },
        (None, true) => DescendantTarget::Owned {
            entry_key: key.clone(),
            node_id: id.clone(),
        },
        (None, false) => DescendantTarget::Patch { key: key.clone() },
    };
    let virtual_id = crate::ref_resolve::instance_child_virtual_id(&scope.virtual_prefix, &id);
    let is_instance = effective.get("type").and_then(Value::as_str) == Some("ref");
    let matched = match wanted {
        Wanted::Anchor(anchor) => *anchor == virtual_id,
        // A replacement root also answers to the key of the node it replaced.
        Wanted::Path(path) => *path == target.path() || (replaced_here && *path == key),
    };
    if matched {
        let full = desc::apply_descendants(raw, overrides, None);
        let display = display_node(scope.doc, &full, is_instance)?;
        return Some((target, virtual_id, display, is_instance));
    }
    if is_instance {
        let nested_target = effective.get("ref").and_then(Value::as_str)?.to_string();
        if !visited.insert(nested_target.clone()) {
            return None;
        }
        let nested = Scope {
            doc: scope.doc,
            outer: scope.outer,
            virtual_prefix: virtual_id,
            key_prefix: format!("{}{id}{PATH_SEPARATOR}", scope.key_prefix),
            // Owned content stays owned below a nested instance only through
            // path keys, which the outer map stores separately.
            owned_by: None,
        };
        let hit = walk_instance(&nested, &effective, wanted, visited);
        visited.remove(&nested_target);
        return hit;
    }
    let owned_by = match (&scope.owned_by, replaced_here || fills_here) {
        (Some(owner), _) => Some(owner.clone()),
        (None, true) => Some(key),
        (None, false) => None,
    };
    let kids = effective
        .get("children")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let child_scope = Scope {
        doc: scope.doc,
        outer: scope.outer,
        virtual_prefix: scope.virtual_prefix.clone(),
        key_prefix: scope.key_prefix.clone(),
        owned_by,
    };
    walk_children(&child_scope, &kids, overrides, wanted, visited)
}

fn display_node(doc: &PenDocument, effective: &Value, is_instance: bool) -> Option<PenNode> {
    let node: PenNode = serde_json::from_value(effective.clone()).ok()?;
    if is_instance {
        return crate::instance_override::resolve_instance_display_node(doc, &node);
    }
    Some(node)
}

pub(crate) fn find_authored<'a>(doc: &'a PenDocument, id: &str) -> Option<&'a PenNode> {
    fn walk<'a>(nodes: &'a [PenNode], id: &str) -> Option<&'a PenNode> {
        nodes.iter().find_map(|node| {
            if node.id_str() == id {
                Some(node)
            } else {
                node.children().and_then(|children| walk(children, id))
            }
        })
    }
    doc.pages
        .iter()
        .flatten()
        .find_map(|page| walk(&page.children, id))
        .or_else(|| walk(&doc.children, id))
}

#[cfg(test)]
#[path = "instance_anchor_tests.rs"]
mod tests;
