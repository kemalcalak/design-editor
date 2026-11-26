//! Find the listed frames in the document JSON, with what the plan needs to
//! know about where they sit, and the references other nodes hold to the
//! nodes inside them.

use std::collections::{HashMap, HashSet};

use serde_json::Value;

use super::diff::{children, node_id, node_type, NOT_REFERENCES};
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

/// Where the nodes inside the members are named from: by inner node id, the
/// id of the node holding the string.
#[derive(Default)]
pub(super) struct InnerLinks {
    /// Named from outside the copy (another node, another copy, a `ref`):
    /// the node would disappear into the instance.
    pub outside: HashMap<String, String>,
    /// Named from inside the same copy (a `scroll_to` to its own section):
    /// the instance's content renders under `instance__child` ids, so the
    /// link would land on the master's node instead.
    pub inside: HashMap<String, String>,
}

/// Every node id inside a member (its root excluded) that a string names.
pub(super) fn inner_links(doc: &Value, members: &HashMap<&str, &Value>) -> InnerLinks {
    let mut owner_of: HashMap<&str, &str> = HashMap::new();
    for (root, member) in members {
        collect_inner(member, root, &mut owner_of);
    }
    let mut links = InnerLinks::default();
    let scan = Scan {
        members,
        owner_of: &owner_of,
    };
    scan.strings(doc, None, "", &mut links, None);
    links
}

fn collect_inner<'a>(node: &'a Value, owner: &'a str, out: &mut HashMap<&'a str, &'a str>) {
    for child in children(node) {
        out.insert(node_id(child), owner);
        if node_type(child) != "ref" {
            collect_inner(child, owner, out);
        }
    }
}

struct Scan<'s> {
    members: &'s HashMap<&'s str, &'s Value>,
    owner_of: &'s HashMap<&'s str, &'s str>,
}

impl Scan<'_> {
    /// Walk every string in `value`. `owner`: the member being walked;
    /// `holder`: the nearest node id, for the message.
    fn strings<'a>(
        &self,
        value: &'a Value,
        owner: Option<&'a str>,
        holder: &'a str,
        links: &mut InnerLinks,
        key: Option<&str>,
    ) {
        match value {
            Value::String(text) => {
                if key.is_some_and(|key| NOT_REFERENCES.contains(&key)) {
                    return;
                }
                if let Some(target_owner) = self.owner_of.get(text.as_str()) {
                    let map = if owner == Some(*target_owner) {
                        &mut links.inside
                    } else {
                        &mut links.outside
                    };
                    map.entry(text.clone())
                        .or_insert_with(|| holder.to_string());
                }
            }
            Value::Array(items) => {
                for item in items {
                    self.strings(item, owner, holder, links, key);
                }
            }
            Value::Object(map) => {
                let (mut owner, mut holder) = (owner, holder);
                if let Some(id) = map.get("id").and_then(Value::as_str) {
                    if map.contains_key("type") {
                        holder = id;
                        if self.members.contains_key(id) {
                            owner = Some(id);
                        }
                    }
                }
                for (key, item) in map {
                    self.strings(item, owner, holder, links, Some(key));
                }
            }
            _ => {}
        }
    }
}
