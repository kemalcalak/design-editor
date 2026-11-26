//! Finding sets and variant masters, and picking the variant an instance
//! shows.
//!
//! A requested combination resolves to the master whose `variantValues`
//! match it on every axis (an axis a master leaves out matches any value).
//! When no master matches, the nearest one wins: the most matching axes,
//! then the one matching the earlier axes in the set's axis order, then
//! the first in the set. The choice is deterministic and reported as a
//! fallback.

use std::collections::HashMap;

use jian_ops_schema::component_set::{ComponentSet, VariantValues};
use jian_ops_schema::node::PenNode;
use jian_ops_schema::PenDocument;

use crate::pen_node_ext::PenNodeExt;

/// The set a frame defines, if it is a component set.
pub fn component_set_of(node: &PenNode) -> Option<&ComponentSet> {
    match node {
        PenNode::Frame(frame) => frame.component_set.as_ref(),
        _ => None,
    }
}

/// A variant master's own values (`variantValues`).
pub fn variant_values_of(node: &PenNode) -> Option<&VariantValues> {
    match node {
        PenNode::Frame(frame) => frame.variant_values.as_ref(),
        _ => None,
    }
}

fn is_master(node: &PenNode) -> bool {
    matches!(node, PenNode::Frame(frame) if frame.reusable == Some(true))
}

/// The variant masters of a set: its top-level reusable descendants (a
/// reusable frame's own children are its content, not variants), in
/// document order.
pub fn variant_masters(set: &PenNode) -> Vec<&PenNode> {
    fn walk<'a>(nodes: &'a [PenNode], out: &mut Vec<&'a PenNode>) {
        for node in nodes {
            if is_master(node) {
                out.push(node);
            } else if component_set_of(node).is_none() && !matches!(node, PenNode::Ref(_)) {
                if let Some(children) = node.children() {
                    walk(children, out);
                }
            }
        }
    }
    let mut out = Vec::new();
    if let Some(children) = set.children() {
        walk(children, &mut out);
    }
    out
}

/// Which set holds each variant master, built in one document walk.
#[derive(Debug, Clone, Default)]
pub struct SetIndex {
    set_of_master: HashMap<String, String>,
}

impl SetIndex {
    /// Index every component set of `doc`, on every page.
    pub fn build(doc: &PenDocument) -> Self {
        let mut index = Self::default();
        let mut visit = |node: &PenNode| {
            if component_set_of(node).is_some() {
                for master in variant_masters(node) {
                    index
                        .set_of_master
                        .insert(master.id_str().to_string(), node.id_str().to_string());
                }
            }
        };
        for_each_node(doc, &mut visit);
        index
    }

    /// The set holding master `master_id`.
    pub fn set_of(&self, master_id: &str) -> Option<&str> {
        self.set_of_master.get(master_id).map(String::as_str)
    }

    /// Whether any set exists.
    pub fn is_empty(&self) -> bool {
        self.set_of_master.is_empty()
    }
}

/// Visit every page-tree node of `doc`, depth first.
pub(crate) fn for_each_node<'a>(doc: &'a PenDocument, visit: &mut dyn FnMut(&'a PenNode)) {
    fn walk<'a>(nodes: &'a [PenNode], visit: &mut dyn FnMut(&'a PenNode)) {
        for node in nodes {
            visit(node);
            if let Some(children) = node.children() {
                walk(children, visit);
            }
        }
    }
    for page in doc.pages.iter().flatten() {
        walk(&page.children, visit);
    }
    walk(&doc.children, visit);
}

/// The set holding master `master_id`, on any page.
pub fn set_containing<'a>(doc: &'a PenDocument, master_id: &str) -> Option<&'a PenNode> {
    let mut found = None;
    for_each_node(doc, &mut |node| {
        if found.is_none()
            && component_set_of(node).is_some()
            && variant_masters(node)
                .iter()
                .any(|master| master.id_str() == master_id)
        {
            found = Some(node);
        }
    });
    found
}

/// The full combination an instance asks for: `requested` where it names
/// a value of the axis, else `current` (the values it shows now), else
/// the set's default. Values outside the axes are ignored here; writes
/// refuse them earlier with [`super::VariantError`].
pub fn full_values(
    set: &ComponentSet,
    requested: Option<&VariantValues>,
    current: Option<&VariantValues>,
) -> VariantValues {
    let mut out = VariantValues::new();
    for (axis, _) in set.axes.iter() {
        let valid = |values: Option<&VariantValues>| {
            values
                .and_then(|values| values.get(axis))
                .filter(|value| set.axes.has_value(axis, value))
                .cloned()
        };
        let value = valid(requested)
            .or_else(|| valid(current))
            .or_else(|| set.default_value(axis).map(str::to_string));
        if let Some(value) = value {
            out.insert(axis.to_string(), value);
        }
    }
    out
}

/// The master a full combination resolves to.
#[derive(Debug, Clone, PartialEq)]
pub struct VariantPick<'a> {
    pub master: &'a PenNode,
    /// The combination asked for (every axis named).
    pub values: VariantValues,
    /// True when the master matches every axis; false for a fallback.
    pub exact: bool,
}

/// Pick the master of `set_node` for `values` (see the module docs for
/// the fallback order). `None` when the set holds no master.
pub fn pick_variant<'a>(set_node: &'a PenNode, values: &VariantValues) -> Option<VariantPick<'a>> {
    let set = component_set_of(set_node)?;
    let axes: Vec<&str> = set.axes.names().collect();
    let mut best: Option<(&PenNode, usize, Vec<bool>)> = None;
    for master in variant_masters(set_node) {
        let own = variant_values_of(master);
        let matches: Vec<bool> = axes
            .iter()
            .map(|axis| match own.and_then(|own| own.get(*axis)) {
                None => true,
                Some(value) => values.get(*axis) == Some(value),
            })
            .collect();
        let count = matches.iter().filter(|hit| **hit).count();
        let better = match &best {
            None => true,
            Some((_, best_count, best_matches)) => {
                count > *best_count || (count == *best_count && matches > *best_matches)
            }
        };
        if better {
            best = Some((master, count, matches));
        }
    }
    best.map(|(master, count, _)| VariantPick {
        master,
        values: values.clone(),
        exact: count == axes.len(),
    })
}

/// Name a variant master the way Figma does: `axis=value, axis=value` in
/// the set's axis order.
pub fn variant_name(set: &ComponentSet, values: &VariantValues) -> String {
    set.axes
        .names()
        .filter_map(|axis| values.get(axis).map(|value| format!("{axis}={value}")))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Parse a `key=value, key=value` master name. `None` unless every part
/// is a non-empty `key=value` pair with distinct keys.
pub fn parse_variant_name(name: &str) -> Option<Vec<(String, String)>> {
    let mut out: Vec<(String, String)> = Vec::new();
    for part in name.split(',') {
        let (key, value) = part.split_once('=')?;
        let (key, value) = (key.trim(), value.trim());
        if key.is_empty() || value.is_empty() || out.iter().any(|(k, _)| k == key) {
            return None;
        }
        out.push((key.to_string(), value.to_string()));
    }
    (!out.is_empty()).then_some(out)
}
