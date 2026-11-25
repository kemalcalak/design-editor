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
    let lookup = |id: &str| renames.get(id).cloned();
    let ids = Lookups {
        target: &lookup,
        path: &lookup,
    };
    rewrite_ids(nodes, &ids, Rewrite::Everything);
}

/// Point `ref` instances in `nodes` at masters inserted in the same
/// `batch_design` program, which name them by a binding (`ref: master` in a
/// script) or an id authored earlier; both become final ids only on insert
/// (#366). An instance whose target `lookup` does not know (a master already
/// in the document) is left exactly as written, `descendants` included: its
/// keys are that master's ids, not this program's. A retargeted instance's
/// `descendants` path segments go through `path` (the master is this
/// program's, so its keys are too). The nodes' own ids stay.
pub(crate) fn retarget_instances(nodes: &mut [PenNode], ids: &Lookups) {
    rewrite_ids(nodes, ids, Rewrite::RetargetedInstances);
}

/// How [`retarget_instances`] renames an instance's `ref` target and its
/// `descendants` path segments.
pub(crate) struct Lookups<'a> {
    pub(crate) target: &'a dyn Fn(&str) -> Option<String>,
    pub(crate) path: &'a dyn Fn(&str) -> Option<String>,
}

/// What [`rewrite_ids`] renames.
#[derive(Clone, Copy, PartialEq)]
enum Rewrite {
    /// Node ids, every `ref` target and every `descendants` path (an import
    /// renaming colliding ids).
    Everything,
    /// Only instances whose target is renamed, and then their `descendants`.
    RetargetedInstances,
}

fn rewrite_ids(nodes: &mut [PenNode], ids: &Lookups, mode: Rewrite) {
    for node in nodes {
        if mode == Rewrite::Everything {
            if let Some(fresh) = (ids.target)(node.id_str()) {
                node.base_mut().id = fresh;
            }
        }
        if let PenNode::Ref(instance) = node {
            let retargeted = (ids.target)(&instance.target);
            let rewrite_paths = mode == Rewrite::Everything || retargeted.is_some();
            if let Some(fresh) = retargeted {
                instance.target = fresh;
            }
            if let Some(overrides) = instance.descendants.take() {
                instance.descendants = Some(
                    overrides
                        .into_iter()
                        .map(|(path, mut value)| {
                            rename_refs_in_value(&mut value, ids, mode);
                            let path = if rewrite_paths {
                                rename_id_path(&path, ids.path)
                            } else {
                                path
                            };
                            (path, value)
                        })
                        .collect(),
                );
            }
        }
        if let Some(children) = node.children_mut() {
            rewrite_ids(children, ids, mode);
        }
    }
}

/// Instances inside a replacement subtree or slot content (#166) keep
/// their components too: their `ref` targets and own override paths follow
/// the renames. The subtree's own ids render in the instance's id space and
/// stay as written.
fn rename_refs_in_value(value: &mut serde_json::Value, ids: &Lookups, mode: Rewrite) {
    let serde_json::Value::Object(map) = value else {
        return;
    };
    if map.get("type").and_then(serde_json::Value::as_str) == Some("ref") {
        let mut retargeted = false;
        if let Some(serde_json::Value::String(target)) = map.get_mut("ref") {
            if let Some(fresh) = (ids.target)(target.as_str()) {
                *target = fresh;
                retargeted = true;
            }
        }
        let rewrite_paths = mode == Rewrite::Everything || retargeted;
        if let Some(serde_json::Value::Object(overrides)) = map.get_mut("descendants") {
            let renamed: serde_json::Map<String, serde_json::Value> = std::mem::take(overrides)
                .into_iter()
                .map(|(path, mut entry)| {
                    rename_refs_in_value(&mut entry, ids, mode);
                    let path = if rewrite_paths {
                        rename_id_path(&path, ids.path)
                    } else {
                        path
                    };
                    (path, entry)
                })
                .collect();
            *overrides = renamed;
        }
    }
    if let Some(serde_json::Value::Array(children)) = map.get_mut("children") {
        for child in children {
            rename_refs_in_value(child, ids, mode);
        }
    }
}

/// `descendants` keys are `/`-separated id paths into the component.
fn rename_id_path(path: &str, lookup: &dyn Fn(&str) -> Option<String>) -> String {
    path.split('/')
        .map(|segment| lookup(segment).unwrap_or_else(|| segment.to_owned()))
        .collect::<Vec<_>>()
        .join("/")
}
