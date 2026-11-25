//! `scroll_to` targets inside an expanded instance (#371).
//!
//! A master's `scroll_to` names a node of the master by its own id. Once
//! the instance expands, that node renders under the virtual id
//! `inst__child`, so Preview would look for an id that is not on the page.
//! [`remap_scroll_targets`] points every such target at the instance's
//! copy; a target outside the master (a section of the page) keeps its id.

use std::collections::HashMap;

use serde_json::{Map, Value};

/// The original-id → virtual-id map of one expanded instance's subtree
/// (`children` already remapped under `ref_id`).
pub(crate) fn virtual_ids(children: &[Value], ref_id: &str) -> HashMap<String, String> {
    let prefix = crate::ref_resolve::instance_child_virtual_id(ref_id, "");
    let mut out = HashMap::new();
    fn walk(nodes: &[Value], prefix: &str, out: &mut HashMap<String, String>) {
        for node in nodes {
            if let Some(id) = node.get("id").and_then(Value::as_str) {
                if let Some(original) = id.strip_prefix(prefix) {
                    out.insert(original.to_owned(), id.to_owned());
                }
            }
            if let Some(Value::Array(kids)) = node.get("children") {
                walk(kids, prefix, out);
            }
        }
    }
    walk(children, &prefix, &mut out);
    out
}

/// Rewrite the `scroll_to` targets of `node` and its subtree that name a
/// key of `ids`.
pub(crate) fn remap_scroll_targets(node: &mut Map<String, Value>, ids: &HashMap<String, String>) {
    if ids.is_empty() {
        return;
    }
    if let Some(Value::Object(events)) = node.get_mut("events") {
        for actions in events.values_mut() {
            let Value::Array(actions) = actions else {
                continue;
            };
            for action in actions {
                let Some(Value::Object(body)) = action.get_mut("scroll_to") else {
                    continue;
                };
                let mapped = body
                    .get("target")
                    .and_then(Value::as_str)
                    .and_then(|target| ids.get(target));
                if let Some(mapped) = mapped.cloned() {
                    body.insert("target".into(), Value::String(mapped));
                }
            }
        }
    }
    if let Some(Value::Array(children)) = node.get_mut("children") {
        for child in children {
            if let Value::Object(child) = child {
                remap_scroll_targets(child, ids);
            }
        }
    }
}
