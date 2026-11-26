//! The structural projection shared by the scan and the plan, and the
//! member → instance diff with its round-trip check.

use std::collections::HashMap;

use jian_ops_schema::node::PenNode;
use serde_json::{Map, Value};

use crate::instance_override::INSTANCE_DIRECT_PROPS;

/// Keys that never take part in a structural fingerprint: identity,
/// placement and per-copy content. They become overrides.
const NON_STRUCTURAL: &[&str] = &[
    "id", "name", "x", "y", "content", "src", "visible", "locked", "events", "explain", "children",
];

/// Keys whose values are never treated as node-id references when a
/// member's ids are mapped onto the master's.
const NOT_REFERENCES: &[&str] = &["id", "name", "content", "type"];

/// FNV-1a, 64 bit: stable across runs and builds (group ids are handed to
/// agents and read back).
pub(super) struct Fnv(u64);

impl Fnv {
    pub fn new() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }

    pub fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 ^= u64::from(*byte);
            self.0 = self.0.wrapping_mul(0x0100_0000_01b3);
        }
    }

    pub fn write_u64(&mut self, value: u64) {
        self.write(&value.to_le_bytes());
    }

    pub fn finish(&self) -> u64 {
        self.0
    }
}

pub(super) fn node_type(node: &Value) -> &str {
    node.get("type").and_then(Value::as_str).unwrap_or_default()
}

pub(super) fn node_id(node: &Value) -> &str {
    node.get("id").and_then(Value::as_str).unwrap_or_default()
}

pub(super) fn children(node: &Value) -> &[Value] {
    node.get("children")
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

/// The node's own structural fields: everything but [`NON_STRUCTURAL`],
/// a numeric width / height reduced to `"fixed"` (sizing mode, not size)
/// and image fill sources dropped. A nested instance (`ref`) keeps its
/// slot children, overrides and properties by value.
pub(super) fn projection(node: &Value) -> Map<String, Value> {
    let Value::Object(map) = node else {
        return Map::new();
    };
    let is_ref = node_type(node) == "ref";
    let mut out = Map::new();
    for (key, value) in map {
        if NON_STRUCTURAL.contains(&key.as_str()) && !(is_ref && key == "children") {
            continue;
        }
        let value = match key.as_str() {
            "width" | "height" if value.is_number() => Value::from("fixed"),
            "fill" => without_image_sources(value),
            _ => value.clone(),
        };
        out.insert(key.clone(), value);
    }
    out
}

fn without_image_sources(value: &Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(items.iter().map(without_image_sources).collect()),
        Value::Object(map) if map.get("type").and_then(Value::as_str) == Some("image") => {
            let mut map = map.clone();
            map.remove("url");
            map.remove("src");
            Value::Object(map)
        }
        other => other.clone(),
    }
}

/// The first structural difference between the master source and a
/// member, as a sentence naming the member's node.
pub(super) fn structure_difference(master: &Value, member: &Value) -> Option<String> {
    let at = |node: &Value| {
        let name = node.get("name").and_then(Value::as_str);
        match name {
            Some(name) => format!("node {} ({name})", node_id(node)),
            None => format!("node {}", node_id(node)),
        }
    };
    if node_type(master) != node_type(member) {
        return Some(format!(
            "{} is a {} where the master has a {}",
            at(member),
            node_type(member),
            node_type(master)
        ));
    }
    let (own_master, own_member) = (projection(master), projection(member));
    if own_master != own_member {
        let key = own_master
            .keys()
            .chain(own_member.keys())
            .find(|key| own_master.get(*key) != own_member.get(*key))
            .cloned()
            .unwrap_or_default();
        return Some(format!("{}: {key} differs from the master", at(member)));
    }
    if node_type(member) == "ref" {
        return None;
    }
    let (master_children, member_children) = (children(master), children(member));
    if master_children.len() != member_children.len() {
        return Some(format!(
            "{} has {} children where the master has {}",
            at(member),
            member_children.len(),
            master_children.len()
        ));
    }
    master_children
        .iter()
        .zip(member_children)
        .find_map(|(master, member)| structure_difference(master, member))
}

/// The member's node ids mapped onto the master's, by place (the roots
/// are not mapped: the instance keeps the member's root id).
fn pair_ids(master: &Value, member: &Value, out: &mut HashMap<String, String>) {
    if node_type(member) == "ref" {
        return;
    }
    for (master, member) in children(master).iter().zip(children(member)) {
        out.insert(node_id(member).to_string(), node_id(master).to_string());
        pair_ids(master, member, out);
    }
}

/// Rewrite every string that names one of the member's own nodes (its
/// ids, its internal `scroll_to` targets, …) to the master's node.
fn remap_strings(value: &mut Value, ids: &HashMap<String, String>, key: Option<&str>) {
    match value {
        Value::String(text) => {
            if key.is_none_or(|key| !NOT_REFERENCES.contains(&key) || key == "id") {
                if let Some(mapped) = ids.get(text.as_str()) {
                    *text = mapped.clone();
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                remap_strings(item, ids, key);
            }
        }
        Value::Object(map) => {
            for (key, item) in map.iter_mut() {
                remap_strings(item, ids, Some(key));
            }
        }
        _ => {}
    }
}

/// The fields of `member` that differ from `master`, as a patch (`null`
/// clears a field the master sets).
fn own_patch(master: &Value, member: &Value, skip: &[&str]) -> Map<String, Value> {
    let empty = Map::new();
    let master = master.as_object().unwrap_or(&empty);
    let member = member.as_object().unwrap_or(&empty);
    let mut patch = Map::new();
    for key in master.keys().chain(member.keys()) {
        if skip.contains(&key.as_str()) || patch.contains_key(key) {
            continue;
        }
        if master.get(key) != member.get(key) {
            patch.insert(key.clone(), member.get(key).cloned().unwrap_or(Value::Null));
        }
    }
    patch
}

fn diff_children(
    master: &Value,
    member: &Value,
    descendants: &mut Map<String, Value>,
) -> Result<(), String> {
    for (master, member) in children(master).iter().zip(children(member)) {
        let patch = own_patch(master, member, &["id", "children"]);
        if node_type(master) == "ref" {
            if let Some(key) = patch
                .keys()
                .find(|key| !INSTANCE_DIRECT_PROPS.contains(&key.as_str()))
            {
                return Err(format!(
                    "nested instance {}: {key} differs from the master",
                    node_id(master)
                ));
            }
        }
        if !patch.is_empty() {
            descendants.insert(node_id(master).to_string(), Value::Object(patch));
        }
        if node_type(master) != "ref" {
            diff_children(master, member, descendants)?;
        }
    }
    Ok(())
}

/// The instance that replaces one member.
pub(super) struct Instance {
    pub node: PenNode,
    pub overrides: usize,
    pub json_len: usize,
}

/// Build the `ref` that replaces `member` and check that it expands back
/// into `member`. `in_flow`: the member sits in an auto-layout parent that
/// places it, so a position the master carries cannot move it.
pub(super) fn build_instance(
    master: &Value,
    master_node: &PenNode,
    member: &Value,
    in_flow: bool,
) -> Result<Instance, String> {
    let mut ids = HashMap::new();
    pair_ids(master, member, &mut ids);
    let mut mapped = member.clone();
    remap_strings(&mut mapped, &ids, None);

    let mut descendants = Map::new();
    diff_children(master, &mapped, &mut descendants)?;

    let master_id = node_id(master);
    let empty = Map::new();
    let member_root = mapped.as_object().unwrap_or(&empty);
    let master_root = master.as_object().unwrap_or(&empty);
    let mut instance = Map::new();
    instance.insert("type".into(), Value::from("ref"));
    instance.insert("id".into(), Value::from(node_id(member)));
    instance.insert("ref".into(), Value::from(master_id));
    let mut top = Map::new();
    let skip = ["id", "children", "type", "reusable", "componentProperties"];
    for key in master_root.keys().chain(member_root.keys()) {
        let key = key.as_str();
        if skip.contains(&key) || instance.contains_key(key) || top.contains_key(key) {
            continue;
        }
        let direct = key == "events" || INSTANCE_DIRECT_PROPS.contains(&key);
        match member_root.get(key) {
            // Position and name always ride on the instance itself.
            Some(value) if direct => {
                instance.insert(key.into(), value.clone());
            }
            Some(value) if master_root.get(key) != Some(value) => {
                top.insert(key.into(), value.clone());
            }
            Some(_) => {}
            // The master sets a field the member does not.
            None if key == "name" || (in_flow && matches!(key, "x" | "y")) => {}
            None => {
                top.insert(key.into(), Value::Null);
            }
        }
    }
    if !top.is_empty() {
        descendants.insert(master_id.to_string(), Value::Object(top));
    }
    let overrides = descendants.len();
    if !descendants.is_empty() {
        instance.insert("descendants".into(), Value::Object(descendants));
    }
    let instance = Value::Object(instance);
    let json_len = instance.to_string().len();
    let node: PenNode = serde_json::from_value(instance)
        .map_err(|error| format!("its instance is not a valid node ({error})"))?;
    verify(master_node, &node, &mapped, in_flow)?;
    Ok(Instance {
        node,
        overrides,
        json_len,
    })
}

/// Expand `instance` the way detach does and compare it with the member
/// (ids mapped onto the master's), field by field.
fn verify(
    master: &PenNode,
    instance: &PenNode,
    member: &Value,
    in_flow: bool,
) -> Result<(), String> {
    let expanded = crate::ref_resolve::materialize_instance(instance, master)
        .ok_or_else(|| "its instance does not expand".to_string())?;
    let mut actual = serde_json::to_value(&expanded).map_err(|error| error.to_string())?;
    let mut expected = member.clone();
    if let Value::Object(map) = &mut expected {
        map.insert("id".into(), Value::from(crate::PenNodeExt::id_str(master)));
    }
    // Normalize the member through the typed model, as the instance was.
    let expected_node: PenNode =
        serde_json::from_value(expected).map_err(|error| error.to_string())?;
    let mut expected = serde_json::to_value(&expected_node).map_err(|error| error.to_string())?;
    for value in [&mut actual, &mut expected] {
        if let Value::Object(map) = value {
            map.remove("reusable");
            map.remove("componentProperties");
        }
    }
    if let (Value::Object(actual), Some(member)) = (&mut actual, member.as_object()) {
        if !member.contains_key("name") {
            actual.remove("name");
        }
        for key in ["x", "y"] {
            if in_flow && !member.contains_key(key) {
                actual.remove(key);
            }
        }
    }
    match first_difference(&expected, &actual, "") {
        None => Ok(()),
        Some(path) => Err(format!("its instance would change {path}")),
    }
}

/// The path of the first value that differs, `None` when equal.
fn first_difference(expected: &Value, actual: &Value, path: &str) -> Option<String> {
    match (expected, actual) {
        (Value::Object(left), Value::Object(right)) => {
            left.keys()
                .chain(right.keys())
                .find_map(|key| match (left.get(key), right.get(key)) {
                    (Some(left), Some(right)) => {
                        first_difference(left, right, &format!("{path}/{key}"))
                    }
                    _ => Some(format!("{path}/{key}")),
                })
        }
        (Value::Array(left), Value::Array(right)) if left.len() == right.len() => left
            .iter()
            .zip(right)
            .enumerate()
            .find_map(|(index, (left, right))| {
                first_difference(left, right, &format!("{path}/{index}"))
            }),
        _ if expected == actual => None,
        _ if path.is_empty() => Some("/".into()),
        _ => Some(path.to_string()),
    }
}
