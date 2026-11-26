//! Moving an instance from one variant master to another.
//!
//! The instance keeps its `props`. Each `descendants` override keeps its
//! place when its target exists in the new variant: under the same id, else
//! under the same name path from the master root (variants copied from one
//! another get fresh ids, so the name path is what usually matches). The
//! override for the master root follows the new root. The rest are dropped
//! and reported.

use jian_ops_schema::component_set::VariantValues;
use jian_ops_schema::node::{PenNode, RefNode};
use serde_json::Value;

use crate::instance_ids::split_first_segment;
use crate::pen_node_ext::PenNodeExt;

/// A node's path of layer names below the master root (a nameless node
/// goes by its kind).
fn name_path(root: &PenNode, id: &str) -> Option<Vec<String>> {
    fn label(node: &PenNode) -> String {
        node.base()
            .name
            .clone()
            .unwrap_or_else(|| node.type_tag().to_string())
    }
    fn walk(node: &PenNode, id: &str, path: &mut Vec<String>) -> bool {
        for child in node.children().into_iter().flatten() {
            path.push(label(child));
            if child.id_str() == id || walk(child, id, path) {
                return true;
            }
            path.pop();
        }
        false
    }
    let mut path = Vec::new();
    walk(root, id, &mut path).then_some(path)
}

/// The node under `root` at a name path, first match at each level.
fn node_at_path<'a>(root: &'a PenNode, path: &[String]) -> Option<&'a PenNode> {
    let mut node = root;
    for name in path {
        node = node.children()?.iter().find(|child| {
            child
                .base()
                .name
                .as_deref()
                .map_or(child.type_tag() == name, |own| own == name)
        })?;
    }
    Some(node)
}

fn contains_id(root: &PenNode, id: &str) -> bool {
    root.children()
        .into_iter()
        .flatten()
        .any(|child| child.id_str() == id || contains_id(child, id))
}

/// Where an override key lands in the new variant, if anywhere.
fn remap_key(key: &str, old: &PenNode, new: &PenNode) -> Option<String> {
    let (first, rest) = split_first_segment(key);
    if first == old.id_str() {
        return Some(format!("{}{rest}", new.id_str()));
    }
    if contains_id(new, first) {
        return Some(key.to_string());
    }
    let path = name_path(old, first)?;
    let target = node_at_path(new, &path)?;
    Some(format!("{}{rest}", target.id_str()))
}

/// Point `reference` at `new_master` showing `values`. Returns the
/// override keys that had no target in the new variant (dropped).
pub(crate) fn retarget(
    reference: &mut RefNode,
    old_master: Option<&PenNode>,
    new_master: &PenNode,
    values: VariantValues,
) -> Vec<String> {
    let mut dropped = Vec::new();
    if reference.target != new_master.id_str() {
        if let Some(descendants) = reference.descendants.take() {
            let mut kept = std::collections::BTreeMap::new();
            for (key, value) in descendants {
                match old_master.and_then(|old| remap_key(&key, old, new_master)) {
                    Some(new_key) => merge(&mut kept, new_key, value),
                    None => dropped.push(key),
                }
            }
            reference.descendants = (!kept.is_empty()).then_some(kept);
        }
        reference.target = new_master.id_str().to_string();
    }
    reference.variant = (!values.is_empty()).then_some(values);
    dropped
}

fn merge(map: &mut std::collections::BTreeMap<String, Value>, key: String, value: Value) {
    match (map.get_mut(&key), value) {
        (Some(Value::Object(existing)), Value::Object(patch)) => {
            for (field, value) in patch {
                existing.entry(field).or_insert(value);
            }
        }
        (Some(_), _) => {}
        (None, value) => {
            map.insert(key, value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(json: &str) -> PenNode {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn overrides_follow_by_id_then_name_path_and_report_the_rest() {
        let old = node(
            r#"{"type":"frame","id":"a","reusable":true,"children":[
                {"type":"text","id":"a-label","name":"Label","content":"x"},
                {"type":"frame","id":"a-icon","name":"Icon","children":[
                    {"type":"rectangle","id":"a-dot","name":"Dot"}]},
                {"type":"text","id":"a-hint","name":"Hint","content":"h"}]}"#,
        );
        let new = node(
            r#"{"type":"frame","id":"b","reusable":true,"children":[
                {"type":"text","id":"b-label","name":"Label","content":"y"},
                {"type":"frame","id":"b-icon","name":"Icon","children":[
                    {"type":"rectangle","id":"b-dot","name":"Dot"}]}]}"#,
        );
        let mut reference: RefNode = serde_json::from_str(
            r##"{"id":"i","ref":"a","props":{"label":"Save"},"descendants":{
                "a":{"opacity":0.5},
                "a-label":{"content":"Hi"},
                "a-dot":{"fill":[{"type":"solid","color":"#ff0000"}]},
                "a-hint":{"content":"gone"}}}"##,
        )
        .unwrap();
        let values = VariantValues::from([("size".to_string(), "lg".to_string())]);
        let dropped = retarget(&mut reference, Some(&old), &new, values);
        assert_eq!(reference.target, "b");
        assert_eq!(dropped, ["a-hint"]);
        let descendants = reference.descendants.as_ref().unwrap();
        assert_eq!(descendants["b"]["opacity"], 0.5);
        assert_eq!(descendants["b-label"]["content"], "Hi");
        assert!(descendants.contains_key("b-dot"));
        assert_eq!(
            reference.props.as_ref().unwrap()["label"],
            "Save",
            "props stay"
        );
        assert_eq!(reference.variant.as_ref().unwrap()["size"], "lg");
    }
}
