//! `import_pen_dev` — insert a pen.dev `.pen` design into the open document.
//!
//! The file is converted by `de_pen_loader::pen_dev` (image files read from
//! its folder) and lands as one undoable batch: the design variables and
//! theme axes it needs, then its boards. Existing variables are never
//! overwritten; a name the document already uses is reported instead.
//!
//! Node ids are kept, not remapped: component instances (`ref`) point at
//! their component by id and `descendants` overrides are keyed by id path,
//! and the generic subtree insert rewrites neither. Only ids the document
//! already uses are renamed, with every `ref` and override path updated to
//! match, and the result goes in through the id-preserving authored insert.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::Path;

use de_editor_core::{EditorState, NodeId, PenNodeExt};
use jian_ops_schema::node::PenNode;
use jian_ops_schema::variable::VariableDefinition;

use super::import_common::{count_subtree_nodes, parse_import_placement};
use super::{EditorCommand, McpTool, ToolErrorCode, ToolOutcome};

pub struct ImportPenDev {
    existing_ids: HashSet<NodeId>,
    variable_names: BTreeSet<String>,
    themes: BTreeMap<String, Vec<String>>,
}

pub fn import_pen_dev_snapshot(state: &EditorState) -> ImportPenDev {
    ImportPenDev {
        existing_ids: state.collect_node_ids(),
        variable_names: state
            .doc
            .variables
            .as_ref()
            .map(|vars| vars.keys().cloned().collect())
            .unwrap_or_default(),
        themes: state.doc.themes.clone().unwrap_or_default(),
    }
}

impl McpTool for ImportPenDev {
    fn name(&self) -> &str {
        "import_pen_dev"
    }

    fn call(&self, args: &BTreeMap<String, String>) -> ToolOutcome {
        let Some(path) = args.get("penPath").or_else(|| args.get("pen_path")) else {
            return ToolOutcome::Err(ToolErrorCode::MissingArgument, "penPath is required".into());
        };
        let src = match std::fs::read_to_string(path) {
            Ok(src) => src,
            Err(error) => {
                return ToolOutcome::Err(
                    ToolErrorCode::ToolFailed,
                    format!("failed to read penPath {path:?}: {error}"),
                );
            }
        };
        if de_pen_loader::pen_dev::sniff_pen_dev_version(&src).is_none() {
            return ToolOutcome::Err(
                ToolErrorCode::InvalidArgument,
                format!("{path:?} is not a pen.dev document (schema 2.9 or later)"),
            );
        }
        let placement = match parse_import_placement(args) {
            Ok(placement) => placement,
            Err((code, message)) => return ToolOutcome::Err(code, message),
        };
        let assets = de_pen_loader::pen_dev::folder_assets(Path::new(path));
        let loaded = match de_pen_loader::payload::load_canonical_with_assets(&src, &assets) {
            Ok(loaded) => loaded,
            Err(error) => {
                return ToolOutcome::Err(
                    ToolErrorCode::InvalidArgument,
                    format!("import {path:?}: {error}"),
                );
            }
        };
        let report = loaded.compatibility.pen_dev.unwrap_or_default();
        let document = loaded.loaded.value;
        let mut nodes = document.children;
        if nodes.is_empty() {
            return ToolOutcome::Err(
                ToolErrorCode::InvalidArgument,
                format!("no importable content in {path:?}"),
            );
        }
        if placement.x != 0 || placement.y != 0 {
            for root in &mut nodes {
                let base = root.base_mut();
                base.x = Some(base.x.unwrap_or(0.0) + f64::from(placement.x));
                base.y = Some(base.y.unwrap_or(0.0) + f64::from(placement.y));
            }
        }
        let renamed = rename_colliding_ids(&mut nodes, &self.existing_ids);
        let dangling = drop_misbound_instances(&mut nodes, &self.existing_ids);

        let mut commands = Vec::new();
        let (variables, conflicts) =
            new_variables(document.variables.unwrap_or_default(), &self.variable_names);
        if !variables.is_empty() {
            commands.push(EditorCommand::SetVariables {
                variables,
                replace: false,
            });
        }
        let themes = merged_themes(document.themes.unwrap_or_default(), &self.themes);
        if !themes.is_empty() {
            commands.push(EditorCommand::SetThemes {
                themes,
                replace: false,
            });
        }
        let node_count = count_subtree_nodes(&nodes);
        commands.push(EditorCommand::InsertAuthoredSubtreePreservingRoots {
            nodes,
            parent_id: placement.parent_id,
            page_id: placement.page_id,
        });

        let mut output = BTreeMap::new();
        output.insert("wrote".into(), "true".into());
        output.insert("nodeCount".into(), node_count.to_string());
        output.insert("sourceVersion".into(), report.source_version.clone());
        if report.has_losses() {
            output.insert("notCarriedOver".into(), report.summary());
        }
        if !report.imports.is_empty() {
            // Inserting into an open document links nothing by itself: name
            // the files so the caller can `add_library` them (paths are
            // relative to the .pen file).
            let imports: Vec<String> = report
                .imports
                .iter()
                .map(|import| format!("{}={}", import.alias, import.path))
                .collect();
            output.insert("libraryImports".into(), imports.join(", "));
        }
        if !conflicts.is_empty() {
            output.insert("keptExistingVariables".into(), conflicts.join(", "));
        }
        if renamed > 0 {
            output.insert("renamedIds".into(), renamed.to_string());
        }
        if dangling.missing > 0 {
            output.insert(
                "instancesWithoutComponent".into(),
                dangling.missing.to_string(),
            );
        }
        if dangling.dropped > 0 {
            output.insert("droppedInstances".into(), dangling.dropped.to_string());
        }
        ToolOutcome::OkWithCommand(output, EditorCommand::Batch { commands })
    }
}

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

/// Instances whose component is not in the imported file (it lived in a
/// `.pen` library the converter cannot follow).
#[derive(Default)]
struct DanglingInstances {
    /// Kept: their target names nothing in the open document either, so
    /// they stay empty instead of borrowing a stranger.
    missing: usize,
    /// Removed: their target matches an unrelated node of the open
    /// document, which the instance would otherwise render as.
    dropped: usize,
}

fn drop_misbound_instances(
    nodes: &mut Vec<PenNode>,
    existing: &HashSet<NodeId>,
) -> DanglingInstances {
    let mut incoming = HashSet::new();
    collect_ids(nodes, &mut incoming);
    let mut found = DanglingInstances::default();
    retain_bound_instances(nodes, &incoming, existing, &mut found);
    found
}

fn retain_bound_instances(
    nodes: &mut Vec<PenNode>,
    incoming: &HashSet<String>,
    existing: &HashSet<NodeId>,
    found: &mut DanglingInstances,
) {
    nodes.retain(|node| {
        let PenNode::Ref(instance) = node else {
            return true;
        };
        if incoming.contains(&instance.target) {
            return true;
        }
        if existing.contains(&NodeId::new(&instance.target)) {
            found.dropped += 1;
            false
        } else {
            found.missing += 1;
            true
        }
    });
    for node in nodes.iter_mut() {
        if let Some(children) = node.children_mut() {
            retain_bound_instances(children, incoming, existing, found);
        }
    }
}

fn collect_ids(nodes: &[PenNode], out: &mut HashSet<String>) {
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

/// Variables the document does not have yet, plus the sorted names it
/// already defines (kept as they are).
fn new_variables(
    incoming: BTreeMap<String, VariableDefinition>,
    existing: &BTreeSet<String>,
) -> (BTreeMap<String, VariableDefinition>, Vec<String>) {
    let mut conflicts = Vec::new();
    let mut added = BTreeMap::new();
    for (name, definition) in incoming {
        if existing.contains(&name) {
            conflicts.push(name);
        } else {
            added.insert(name, definition);
        }
    }
    (added, conflicts)
}

/// Theme axes to write: for each incoming axis, the document's values with
/// the new ones appended. `SetThemes` merges per axis, so only changed axes
/// are sent.
fn merged_themes(
    incoming: BTreeMap<String, Vec<String>>,
    existing: &BTreeMap<String, Vec<String>>,
) -> BTreeMap<String, Vec<String>> {
    let mut out = BTreeMap::new();
    for (axis, values) in incoming {
        let mut merged = existing.get(&axis).cloned().unwrap_or_default();
        let before = merged.len();
        for value in values {
            if !merged.contains(&value) {
                merged.push(value);
            }
        }
        if merged.len() != before || !existing.contains_key(&axis) {
            out.insert(axis, merged);
        }
    }
    out
}
