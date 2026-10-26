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

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::Path;

use de_editor_core::{EditorState, NodeId, PenNodeExt};
use jian_ops_schema::node::PenNode;
use jian_ops_schema::variable::VariableDefinition;

use super::import_common::{count_subtree_nodes, offset_roots, parse_import_placement};
use super::import_ids::{collect_ids, rename_colliding_ids};
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
        offset_roots(&mut nodes, &placement);
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
