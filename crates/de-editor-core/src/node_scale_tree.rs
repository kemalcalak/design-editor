//! The Scale tool's subtree walk (#394): positions, the property rules of
//! [`crate::node_scale`] on every node, and component instances.
//!
//! An instance (`ref`) holds no geometry of its own: what it shows is its
//! master plus its `descendants` overrides. Scaling it scales that display
//! subtree and writes the difference back as overrides, the way the
//! property panel edits an instance: the root's changes go to
//! `descendants[ref]`, a child's to `descendants[childId]`, a nested
//! instance's to `nested/…` keys. The master is never touched.

use std::cell::Cell;

use jian_ops_schema::node::{PenNode, RefNode};
use jian_ops_schema::PenDocument;
use serde_json::{Map, Value};

use crate::node_scale::{PropScaler, ScaleReport};
use crate::pen_node_ext::PenNodeExt;

/// How a node's own `x` / `y` move.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Position {
    /// Leave them (a flow child, or an instance root whose position is
    /// the instance's).
    Keep,
    /// Scale about this point in the parent's space (a scaled root).
    Pivot(f64, f64),
    /// Multiply by the factor (a descendant: its parent's origin scaled
    /// with it).
    Multiply,
}

/// Nested instances deeper than this are left as they are (a cycle the
/// loader let through must not recurse forever).
const MAX_INSTANCE_DEPTH: u32 = 16;

pub(crate) struct TreeScaler<'a> {
    pub props: PropScaler<'a>,
    /// Where instance masters are looked up.
    pub doc: &'a PenDocument,
    depth: Cell<u32>,
}

impl<'a> TreeScaler<'a> {
    pub fn new(props: PropScaler<'a>, doc: &'a PenDocument) -> Self {
        Self {
            props,
            doc,
            depth: Cell::new(0),
        }
    }

    /// Scale `node` and everything inside it.
    pub fn scale_node(&self, node: &mut PenNode, position: Position, report: &mut ScaleReport) {
        let f = self.props.factor;
        let base = node.base_mut();
        match position {
            Position::Keep => {}
            Position::Pivot(px, py) => {
                base.x = Some(px + (base.x.unwrap_or(0.0) - px) * f);
                base.y = Some(py + (base.y.unwrap_or(0.0) - py) * f);
            }
            Position::Multiply => {
                base.x = base.x.map(|x| x * f);
                base.y = base.y.map(|y| y * f);
            }
        }
        if let PenNode::Ref(reference) = node {
            self.scale_instance(reference, report);
            // Slot content the instance supplies itself is ordinary nodes.
            for child in reference.children.iter_mut().flatten() {
                self.scale_node(child, Position::Multiply, report);
            }
            return;
        }
        let children = node.children_mut().map(std::mem::take);
        let id = node.id_str().to_owned();
        if let Ok(Value::Object(mut map)) = serde_json::to_value(&*node) {
            self.props
                .scale_own_in(&mut map, &id, report, self.depth.get() > 0);
            if let Ok(scaled) = serde_json::from_value::<PenNode>(Value::Object(map)) {
                *node = scaled;
            }
        }
        if let (Some(mut children), Some(slot)) = (children, node.children_mut()) {
            for child in &mut children {
                self.scale_node(child, Position::Multiply, report);
            }
            *slot = children;
        }
    }

    /// Scale what the instance shows and store the difference as
    /// overrides. Its position is the caller's ([`Self::scale_node`]).
    fn scale_instance(&self, reference: &mut RefNode, report: &mut ScaleReport) {
        let unresolved = |report: &mut ScaleReport| {
            report.unresolved_instances.push(reference.base.id.clone());
        };
        if self.depth.get() >= MAX_INSTANCE_DEPTH {
            return unresolved(report);
        }
        let Some(master) = crate::component_set::shown_master(self.doc, reference).cloned() else {
            return unresolved(report);
        };
        let as_node = PenNode::Ref(reference.clone());
        let Some(display) = crate::ref_resolve::materialize_instance(&as_node, &master) else {
            return unresolved(report);
        };
        let mut scaled = display.clone();
        self.depth.set(self.depth.get() + 1);
        self.scale_node(&mut scaled, Position::Keep, report);
        self.depth.set(self.depth.get() - 1);
        let (Ok(before), Ok(after)) = (
            serde_json::to_value(&display),
            serde_json::to_value(&scaled),
        ) else {
            return unresolved(report);
        };
        let mut patches = Vec::new();
        diff_display(&before, &after, Some(&reference.target), &mut patches);
        if patches.is_empty() {
            return;
        }
        let descendants = reference.descendants.get_or_insert_with(Default::default);
        for (key, patch) in patches {
            match descendants.get_mut(&key) {
                // A patch or a replacement already there: the scaled
                // values win key by key (a replacement is the node itself).
                Some(Value::Object(existing)) => existing.extend(patch),
                _ => {
                    descendants.insert(key, Value::Object(patch));
                }
            }
        }
    }
}

/// Keys that never become part of an override patch.
fn structural(key: &str) -> bool {
    matches!(key, "id" | "type" | "children" | "descendants")
}

/// Walk the display subtree before and after scaling and collect one
/// override patch per changed node, keyed the way `descendants` addresses
/// it. `root_key` is the instance's `ref` for its root (whose `x` / `y`
/// belong to the instance itself), `None` below it.
fn diff_display(
    before: &Value,
    after: &Value,
    root_key: Option<&str>,
    out: &mut Vec<(String, Map<String, Value>)>,
) {
    let (Some(b), Some(a)) = (before.as_object(), after.as_object()) else {
        return;
    };
    let id = b.get("id").and_then(Value::as_str).unwrap_or_default();
    let key = root_key.unwrap_or(id).to_owned();
    // The root's position is the instance's own, never an override.
    let owned_by_instance = |k: &str| root_key.is_some() && matches!(k, "x" | "y");
    let mut patch: Map<String, Value> = a
        .iter()
        .filter(|(k, v)| !structural(k) && !owned_by_instance(k) && b.get(k.as_str()) != Some(*v))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    if b.get("type").and_then(Value::as_str) == Some("ref") {
        // A nested instance: its own scaled overrides travel up as
        // `nested/…` entries; the ones for its root join its own patch
        // (the nested-root router sends them to its `descendants[ref]`).
        let target = a.get("ref").and_then(Value::as_str).unwrap_or_default();
        let empty = Map::new();
        let old = b
            .get("descendants")
            .and_then(Value::as_object)
            .unwrap_or(&empty);
        let new = a
            .get("descendants")
            .and_then(Value::as_object)
            .unwrap_or(&empty);
        for (entry_key, value) in new {
            if old.get(entry_key) == Some(value) {
                continue;
            }
            let Some(entry) = value.as_object() else {
                continue;
            };
            if entry_key == target {
                patch.extend(
                    entry
                        .iter()
                        .filter(|(k, _)| !structural(k))
                        .map(|(k, v)| (k.clone(), v.clone())),
                );
            } else {
                out.push((format!("{key}/{entry_key}"), entry.clone()));
            }
        }
        if !patch.is_empty() {
            out.push((key, patch));
        }
        return;
    }
    if !patch.is_empty() {
        out.push((key, patch));
    }
    if let (Some(Value::Array(bc)), Some(Value::Array(ac))) = (b.get("children"), a.get("children"))
    {
        for (child_before, child_after) in bc.iter().zip(ac) {
            diff_display(child_before, child_after, None, out);
        }
    }
}
