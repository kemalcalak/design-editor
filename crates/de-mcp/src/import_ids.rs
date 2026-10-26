//! Id-collision handling shared by the import tools that keep their
//! source's ids (`import_pen_dev`, `import_pptx`).

use std::collections::{HashMap, HashSet};

use de_editor_core::{NodeId, PenNodeExt};
use jian_ops_schema::node::PenNode;

/// Rename every incoming id the document already uses (`id` → `id-2`, …),
/// then rewrite `ref` targets and `descendants` id paths with the same map so
/// instances keep their components. Returns how many ids were renamed.
pub(crate) fn rename_colliding_ids(nodes: &mut [PenNode], existing: &HashSet<NodeId>) -> usize {
    let mut incoming = HashSet::new();
    collect_ids(nodes, &mut incoming);
    if !incoming
        .iter()
        .any(|id| existing.contains(&NodeId::new(id)))
    {
        return 0;
    }
    let mut renames: HashMap<String, String> = HashMap::new();
    let mut taken: HashSet<String> = incoming.clone();
    taken.extend(existing.iter().map(|id| id.as_str().to_owned()));
    for id in &incoming {
        if !existing.contains(&NodeId::new(id)) {
            continue;
        }
        let fresh = (2..)
            .map(|n| format!("{id}-{n}"))
            .find(|candidate| !taken.contains(candidate))
            .expect("an unused suffix always exists");
        taken.insert(fresh.clone());
        renames.insert(id.clone(), fresh);
    }
    if !renames.is_empty() {
        apply_renames(nodes, &renames);
    }
    renames.len()
}

pub(crate) fn collect_ids(nodes: &[PenNode], out: &mut HashSet<String>) {
    for node in nodes {
        out.insert(node.id_str().to_owned());
        if let Some(children) = node.children() {
            collect_ids(children, out);
        }
    }
}

fn apply_renames(nodes: &mut [PenNode], renames: &HashMap<String, String>) {
    for node in nodes {
        if let Some(fresh) = renames.get(node.id_str()) {
            node.base_mut().id = fresh.clone();
        }
        if let PenNode::Ref(instance) = node {
            if let Some(fresh) = renames.get(&instance.target) {
                instance.target = fresh.clone();
            }
            if let Some(overrides) = instance.descendants.take() {
                instance.descendants = Some(
                    overrides
                        .into_iter()
                        .map(|(path, mut value)| {
                            rename_refs_in_value(&mut value, renames);
                            (rename_id_path(&path, renames), value)
                        })
                        .collect(),
                );
            }
        }
        if let Some(children) = node.children_mut() {
            apply_renames(children, renames);
        }
    }
}

/// Instances inside a replacement subtree or slot content (#166) keep
/// their components too: their `ref` targets and own override paths follow
/// the renames. The subtree's own ids render in the instance's id space and
/// stay as written.
fn rename_refs_in_value(value: &mut serde_json::Value, renames: &HashMap<String, String>) {
    let serde_json::Value::Object(map) = value else {
        return;
    };
    if map.get("type").and_then(serde_json::Value::as_str) == Some("ref") {
        if let Some(serde_json::Value::String(target)) = map.get_mut("ref") {
            if let Some(fresh) = renames.get(target.as_str()) {
                *target = fresh.clone();
            }
        }
        if let Some(serde_json::Value::Object(overrides)) = map.get_mut("descendants") {
            let renamed: serde_json::Map<String, serde_json::Value> = std::mem::take(overrides)
                .into_iter()
                .map(|(path, mut entry)| {
                    rename_refs_in_value(&mut entry, renames);
                    (rename_id_path(&path, renames), entry)
                })
                .collect();
            *overrides = renamed;
        }
    }
    if let Some(serde_json::Value::Array(children)) = map.get_mut("children") {
        for child in children {
            rename_refs_in_value(child, renames);
        }
    }
}

/// `descendants` keys are `/`-separated id paths into the component.
fn rename_id_path(path: &str, renames: &HashMap<String, String>) -> String {
    path.split('/')
        .map(|segment| renames.get(segment).map_or(segment, String::as_str))
        .collect::<Vec<_>>()
        .join("/")
}
