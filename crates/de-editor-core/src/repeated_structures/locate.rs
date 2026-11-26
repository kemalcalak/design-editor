//! Find the listed frames in the document JSON, with what the plan needs to
//! know about where they sit, and the references other nodes hold to the
//! nodes inside them.

use std::collections::{HashMap, HashSet};

use serde_json::Value;

use super::diff::{children, node_id, node_type};
use super::scan::is_master;
use crate::page_mutators::COMPONENTS_PAGE_NAME;

/// Why a node cannot be converted wherever it is, if anything.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Blocked {
    None,
    ComponentsPage,
    InsideMaster,
    InsideInstance,
}

/// A listed node as found.
pub(super) struct Found<'a> {
    pub value: &'a Value,
    pub blocked: Blocked,
    /// The parent places it (auto layout) and it does not opt out.
    pub in_flow: bool,
    pub ancestors: Vec<&'a str>,
}

struct Walk<'a, 'w> {
    wanted: &'w HashSet<&'w str>,
    masters: &'w HashSet<&'w str>,
    found: HashMap<String, Found<'a>>,
    stack: Vec<&'a str>,
}

fn is_auto_layout(node: &Value) -> bool {
    matches!(
        node.get("layout").and_then(Value::as_str),
        Some("vertical" | "horizontal" | "grid")
    )
}

fn keeps_absolute(node: &Value) -> bool {
    node.get("role").and_then(Value::as_str) == Some("overlay")
        || node
            .get("constraints")
            .is_some_and(|value| !value.is_null())
}

impl<'a> Walk<'a, '_> {
    fn node(&mut self, node: &'a Value, parent: Option<&'a Value>, blocked: Blocked) {
        let id = node_id(node);
        // The first use of an id wins, as every lookup by id does.
        if self.wanted.contains(id) && !self.found.contains_key(id) {
            self.found.insert(
                id.to_string(),
                Found {
                    value: node,
                    blocked,
                    in_flow: parent.is_some_and(is_auto_layout) && !keeps_absolute(node),
                    ancestors: self.stack.clone(),
                },
            );
        }
        let inner = if blocked != Blocked::None {
            blocked
        } else if node_type(node) == "ref" {
            Blocked::InsideInstance
        } else if is_master(node, self.masters) {
            Blocked::InsideMaster
        } else {
            Blocked::None
        };
        self.stack.push(id);
        for child in children(node) {
            self.node(child, Some(node), inner);
        }
        self.stack.pop();
    }
}

/// Locate every wanted id in the document JSON (pages in order, then the
/// root list).
pub(super) fn locate<'a>(
    doc: &'a Value,
    wanted: &HashSet<&str>,
    masters: &HashSet<&str>,
) -> HashMap<String, Found<'a>> {
    let mut walk = Walk {
        wanted,
        masters,
        found: HashMap::new(),
        stack: Vec::new(),
    };
    for page in doc
        .get("pages")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let blocked = if page.get("name").and_then(Value::as_str) == Some(COMPONENTS_PAGE_NAME) {
            Blocked::ComponentsPage
        } else {
            Blocked::None
        };
        for root in children(page) {
            walk.node(root, None, blocked);
        }
    }
    for root in children(doc) {
        walk.node(root, None, Blocked::None);
    }
    walk.found
}

/// Every node id inside a member (its root excluded) that a string outside
/// that member names (a link target, a `ref`, …), with the id of the node
/// holding the reference. Such a node would disappear into the instance.
pub(super) fn referenced_inner_ids(
    doc: &Value,
    members: &HashMap<&str, &Value>,
) -> HashMap<String, String> {
    let mut owner_of: HashMap<&str, &str> = HashMap::new();
    for (root, member) in members {
        collect_inner(member, root, &mut owner_of);
    }
    let mut referenced = HashMap::new();
    scan_strings(doc, None, "", members, &owner_of, &mut referenced, None);
    referenced
}

fn collect_inner<'a>(node: &'a Value, owner: &'a str, out: &mut HashMap<&'a str, &'a str>) {
    for child in children(node) {
        out.insert(node_id(child), owner);
        if node_type(child) != "ref" {
            collect_inner(child, owner, out);
        }
    }
}

/// Walk every string in `value`. `owner`: the member being walked;
/// `holder`: the nearest node id, for the message.
fn scan_strings<'a>(
    value: &'a Value,
    owner: Option<&'a str>,
    holder: &'a str,
    members: &HashMap<&str, &Value>,
    owner_of: &HashMap<&str, &str>,
    out: &mut HashMap<String, String>,
    key: Option<&str>,
) {
    match value {
        Value::String(text) => {
            if matches!(key, Some("id" | "name" | "content" | "type")) {
                return;
            }
            if let Some(target_owner) = owner_of.get(text.as_str()) {
                if owner != Some(*target_owner) {
                    out.entry(text.clone())
                        .or_insert_with(|| holder.to_string());
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                scan_strings(item, owner, holder, members, owner_of, out, key);
            }
        }
        Value::Object(map) => {
            let (mut owner, mut holder) = (owner, holder);
            if let Some(id) = map.get("id").and_then(Value::as_str) {
                if map.contains_key("type") {
                    holder = id;
                    if members.contains_key(id) {
                        owner = Some(id);
                    }
                }
            }
            for (key, item) in map {
                scan_strings(item, owner, holder, members, owner_of, out, Some(key));
            }
        }
        _ => {}
    }
}
