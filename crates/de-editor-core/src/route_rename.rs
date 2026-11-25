//! Keep `navigate` actions pointing at a screen whose route moved (#371).
//!
//! Making a frame the start screen gives it `/` and moves the frame that
//! held `/` to a path of its own name (`screen_marker_edit`). Every
//! `push` / `replace` on the page that aimed at either old route follows
//! its screen, so the prototype's links keep going where they went.
//! Renames apply all at once, from the original routes, so a swap does
//! not chain. Instance overrides (`descendants[…].events`) are rewritten
//! too. A removed marker is not a rename: its links are reported as
//! broken by `list_interactions` instead.

use std::collections::BTreeMap;

use jian_ops_schema::events::{Action, EventHandlers};
use jian_ops_schema::node::PenNode;
use serde_json::Value;

use crate::interaction_edit::{events_patch_json, unquote_literal, NavigateVerb};
use crate::pen_node_ext::PenNodeExt;
use crate::{EditorCommand, NodeId};

/// The writes that move every navigate on `roots` (one page) from an old
/// route to its new one (`renames`: old → new).
pub fn route_rename_commands(
    roots: &[PenNode],
    renames: &BTreeMap<String, String>,
) -> Vec<EditorCommand> {
    let mut out = Vec::new();
    if renames.is_empty() {
        return out;
    }
    walk(roots, renames, &mut out);
    out
}

fn walk(nodes: &[PenNode], renames: &BTreeMap<String, String>, out: &mut Vec<EditorCommand>) {
    for node in nodes {
        if let Some(handlers) = node.events() {
            let mut handlers = handlers.clone();
            if rename_handlers(&mut handlers, renames) {
                if let Some(patch_json) = events_patch_json(&handlers) {
                    out.push(patch(node.id_str(), patch_json));
                }
            }
        }
        if let PenNode::Ref(reference) = node {
            if let Some(descendants) = reference.descendants.as_ref() {
                let mut descendants = serde_json::to_value(descendants).unwrap_or(Value::Null);
                if rename_in_value(&mut descendants, renames) {
                    let patch_json = serde_json::json!({ "descendants": descendants }).to_string();
                    out.push(patch(node.id_str(), patch_json));
                }
            }
        }
        if let Some(children) = node.children() {
            walk(children, renames, out);
        }
    }
}

fn patch(node_id: &str, patch_json: String) -> EditorCommand {
    EditorCommand::PatchNodeData {
        node_id: NodeId::new(node_id.to_owned()),
        patch_json,
        page_id: None,
    }
}

/// Rename the navigate targets in `handlers`; `true` when any changed.
fn rename_handlers(handlers: &mut EventHandlers, renames: &BTreeMap<String, String>) -> bool {
    let Ok(mut value) = serde_json::to_value(&*handlers) else {
        return false;
    };
    if !rename_actions_in_events(&mut value, renames) {
        return false;
    }
    match serde_json::from_value(value) {
        Ok(renamed) => {
            *handlers = renamed;
            true
        }
        Err(_) => false,
    }
}

/// Walk any JSON (a `descendants` map, replacement subtrees included) and
/// rename the navigate targets of every `events` block in it.
fn rename_in_value(value: &mut Value, renames: &BTreeMap<String, String>) -> bool {
    let mut changed = false;
    match value {
        Value::Object(map) => {
            for (key, child) in map.iter_mut() {
                changed |= if key == "events" {
                    rename_actions_in_events(child, renames)
                } else {
                    rename_in_value(child, renames)
                };
            }
        }
        Value::Array(items) => {
            for item in items {
                changed |= rename_in_value(item, renames);
            }
        }
        _ => {}
    }
    changed
}

fn rename_actions_in_events(events: &mut Value, renames: &BTreeMap<String, String>) -> bool {
    let Value::Object(handlers) = events else {
        return false;
    };
    let mut changed = false;
    for actions in handlers.values_mut() {
        let Value::Array(actions) = actions else {
            continue;
        };
        for action in actions {
            let Ok(parsed) = serde_json::from_value::<Action>(action.clone()) else {
                continue;
            };
            let Some((key, body)) = parsed.single() else {
                continue;
            };
            let Some(verb) = NavigateVerb::from_key(key) else {
                continue;
            };
            let Some(source) = body.as_str() else {
                continue;
            };
            let route = unquote_literal(source);
            if route == source && !source.starts_with('/') {
                continue;
            }
            if let Some(new_route) = renames.get(&route) {
                let renamed = crate::interaction_edit::navigate_action(verb, new_route);
                if let Ok(json) = serde_json::to_value(renamed) {
                    *action = json;
                    changed = true;
                }
            }
        }
    }
    changed
}

#[cfg(test)]
#[path = "route_rename_tests.rs"]
mod tests;
