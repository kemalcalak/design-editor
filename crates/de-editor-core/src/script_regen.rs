//! When a script node's children are regenerated, and how a result lands
//! (#19).
//!
//! Core owns the decision, the host owns the engine. A script's output is
//! a pure function of its source, its resolved inputs and its size, so a
//! fingerprint of those three ([`script_fingerprint`]) is compared with the
//! `scriptHash` its saved children were generated from. The host asks for
//! work with [`EditorState::take_script_jobs`], which walks the document
//! only when its revision moved, never per paint, and never hands out a
//! second job for a node whose job is still running. Results come back
//! through [`EditorState::land_script_output`], which drops a result whose
//! fingerprint no longer matches the node (the node was edited while the
//! job ran) and writes a current one without an undo step: generated
//! children are derived data, and every history snapshot already carries
//! the children and `scriptHash` that belong to it.

use std::collections::{BTreeMap, HashMap, HashSet};

use jian_ops_schema::node::{PenNode, ScriptNode};
use jian_ops_schema::PenDocument;
use serde_json::Value;

use crate::node_id::NodeId;
use crate::pen_node_ext::PenNodeExt;
use crate::script_inputs::{resolved_script_inputs, script_input_specs};
use crate::state::EditorState;
use crate::variables_resolve::effective_theme;
use crate::walkers::{collect_ids, find_node_mut};

/// One script run the host should perform.
#[derive(Debug, Clone, PartialEq)]
pub struct ScriptJob {
    pub node_id: NodeId,
    pub source: String,
    /// The node's width and height (`pencil.width` / `pencil.height`).
    pub width: f64,
    pub height: f64,
    /// `pencil.input`, defaults filled and bindings resolved.
    pub inputs: BTreeMap<String, Value>,
    /// Fingerprint of the three above; hand it back with the result.
    pub hash: String,
}

/// What a run produced.
#[derive(Debug, Clone, PartialEq)]
pub enum ScriptOutcome {
    /// The nodes the program returned, ids not yet assigned.
    Generated(Vec<PenNode>),
    /// Why the run failed, as shown in the panel.
    Failed(String),
}

/// The last thing known about a node's script, for the panel.
#[derive(Debug, Clone, PartialEq)]
pub enum ScriptRunStatus {
    Running,
    Done {
        nodes: usize,
        elapsed_ms: u64,
    },
    /// The run for `hash` failed; the node keeps its last good children
    /// and is not retried until its fingerprint changes.
    Failed {
        hash: String,
        message: String,
    },
}

/// Runtime bookkeeping. Never saved, never in history.
#[derive(Debug, Clone, Default)]
pub struct ScriptRegenState {
    /// `(document generation, revision)` the last scan saw; `None` forces
    /// the next scan.
    scanned: Option<(u64, u64)>,
    /// Node → fingerprint of its running job.
    in_flight: HashMap<NodeId, String>,
    status: HashMap<NodeId, ScriptRunStatus>,
}

impl ScriptRegenState {
    /// Status of `id`'s script, if it has run in this session.
    pub fn status(&self, id: &NodeId) -> Option<&ScriptRunStatus> {
        self.status.get(id)
    }

    /// Whether any job is still running (hosts keep waking while so).
    pub fn has_running_jobs(&self) -> bool {
        !self.in_flight.is_empty()
    }
}

/// Width / height a script runs at: the authored fixed size, else 0.
fn script_size(node: &PenNode) -> (f64, f64) {
    (
        node.width_px().unwrap_or(0.0),
        node.height_px().unwrap_or(0.0),
    )
}

/// FNV-1a, 64 bit: stable across runs and platforms, unlike `std`'s
/// randomly keyed hasher, so the saved `scriptHash` stays comparable.
fn fnv1a(bytes: &[u8], mut hash: u64) -> u64 {
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// The job a script node runs as now, with its fingerprint. `None` for a
/// node with no source.
pub fn script_job(
    node: &PenNode,
    script: &ScriptNode,
    doc: &PenDocument,
    theme: &BTreeMap<String, String>,
) -> Option<ScriptJob> {
    let source = script.script.as_deref().filter(|s| !s.trim().is_empty())?;
    let specs = script_input_specs(source);
    let inputs = resolved_script_inputs(
        &specs,
        script.inputs.as_ref(),
        doc.variables.as_ref(),
        theme,
    );
    let (width, height) = script_size(node);
    Some(ScriptJob {
        node_id: NodeId::new(script.frame.base.id.as_str()),
        source: source.to_string(),
        width,
        height,
        hash: script_fingerprint(source, width, height, &inputs),
        inputs,
    })
}

/// Fingerprint of everything a run depends on, as 16 hex digits.
pub fn script_fingerprint(
    source: &str,
    width: f64,
    height: f64,
    inputs: &BTreeMap<String, Value>,
) -> String {
    let mut hash = fnv1a(source.as_bytes(), 0xcbf2_9ce4_8422_2325);
    hash = fnv1a(&width.to_bits().to_le_bytes(), hash);
    hash = fnv1a(&height.to_bits().to_le_bytes(), hash);
    // A `BTreeMap` serializes in key order, so equal maps hash equally.
    let inputs = serde_json::to_string(inputs).unwrap_or_default();
    hash = fnv1a(inputs.as_bytes(), hash);
    format!("{hash:016x}")
}

/// Every page's root list (the single-page fallback when there are no
/// pages).
fn root_lists(doc: &PenDocument) -> Vec<&[PenNode]> {
    match doc.pages.as_ref() {
        Some(pages) if !pages.is_empty() => pages.iter().map(|p| p.children.as_slice()).collect(),
        _ => vec![doc.children.as_slice()],
    }
}

fn root_lists_mut(doc: &mut PenDocument) -> Vec<&mut Vec<PenNode>> {
    match doc.pages.as_mut() {
        Some(pages) if !pages.is_empty() => pages.iter_mut().map(|p| &mut p.children).collect(),
        _ => vec![&mut doc.children],
    }
}

/// Visit every script node; a script's own children are generated, so
/// the walk does not enter them.
fn visit_scripts<'a>(nodes: &'a [PenNode], out: &mut Vec<(&'a PenNode, &'a ScriptNode)>) {
    for node in nodes {
        if let PenNode::Script(script) = node {
            out.push((node, script));
        } else if let Some(children) = node.children() {
            visit_scripts(children, out);
        }
    }
}

/// Generated nodes get `<script id>-<n>` ids in depth-first order, so a
/// regeneration keeps the ids of nodes that stay put (selection and
/// hover survive). An id some other node already uses gets a suffix.
fn assign_generated_ids(script_id: &str, nodes: &mut [PenNode], taken: &mut HashSet<NodeId>) {
    fn walk(script_id: &str, nodes: &mut [PenNode], next: &mut usize, taken: &mut HashSet<NodeId>) {
        for node in nodes {
            let mut id = format!("{script_id}-{next}");
            let mut bump = 1;
            while taken.contains(&NodeId::new(id.as_str())) {
                id = format!("{script_id}-{next}-{bump}");
                bump += 1;
            }
            *next += 1;
            taken.insert(NodeId::new(id.as_str()));
            node.base_mut().id = id;
            if let Some(children) = node.children_mut() {
                walk(script_id, children, next, taken);
            }
        }
    }
    let mut next = 0;
    walk(script_id, nodes, &mut next, taken);
}

/// Count of nodes in a forest, descendants included.
pub fn forest_size(nodes: &[PenNode]) -> usize {
    nodes
        .iter()
        .map(|node| 1 + node.children().map_or(0, |c| forest_size(c)))
        .sum()
}

impl EditorState {
    /// Scripts whose saved children are stale, each marked running. Empty
    /// without a scan when the document has not changed since the last
    /// call, and never a second job for a node still running.
    pub fn take_script_jobs(&mut self) -> Vec<ScriptJob> {
        let key = (self.document_generation(), self.revision);
        if self.script_regen.scanned == Some(key) {
            return Vec::new();
        }
        self.script_regen.scanned = Some(key);
        let theme = effective_theme(&self.doc, &self.ui.variables.active_theme);
        let mut scripts = Vec::new();
        for roots in root_lists(&self.doc) {
            visit_scripts(roots, &mut scripts);
        }
        let regen = &self.script_regen;
        let jobs: Vec<ScriptJob> = scripts
            .into_iter()
            .filter_map(|(node, script)| {
                let job = script_job(node, script, &self.doc, &theme)?;
                let failed_before = matches!(regen.status.get(&job.node_id),
                    Some(ScriptRunStatus::Failed { hash, .. }) if *hash == job.hash);
                let current = script.script_hash.as_deref() == Some(job.hash.as_str());
                (!current && !failed_before && !regen.in_flight.contains_key(&job.node_id))
                    .then_some(job)
            })
            .collect();
        for job in &jobs {
            self.script_regen
                .in_flight
                .insert(job.node_id.clone(), job.hash.clone());
            self.script_regen
                .status
                .insert(job.node_id.clone(), ScriptRunStatus::Running);
        }
        jobs
    }

    fn find_script(&self, id: &NodeId) -> Option<&ScriptNode> {
        root_lists(&self.doc).into_iter().find_map(|roots| {
            match crate::walkers::find_node(roots, id) {
                Some(PenNode::Script(script)) => Some(script),
                _ => None,
            }
        })
    }

    /// Land a finished run. Returns true when the document changed.
    pub fn land_script_output(
        &mut self,
        node_id: &NodeId,
        hash: &str,
        outcome: ScriptOutcome,
        elapsed_ms: u64,
    ) -> bool {
        self.script_regen.in_flight.remove(node_id);
        // Whatever happens, look again: an edit made while this ran may
        // need a fresh job.
        self.script_regen.scanned = None;
        let theme = effective_theme(&self.doc, &self.ui.variables.active_theme);
        let current = root_lists(&self.doc).into_iter().find_map(|roots| {
            match crate::walkers::find_node(roots, node_id) {
                Some(node @ PenNode::Script(script)) => {
                    script_job(node, script, &self.doc, &theme).map(|job| job.hash)
                }
                _ => None,
            }
        });
        if current.as_deref() != Some(hash) {
            self.script_regen.status.remove(node_id);
            return false;
        }
        let mut nodes = match outcome {
            ScriptOutcome::Failed(message) => {
                let status = ScriptRunStatus::Failed {
                    hash: hash.to_string(),
                    message,
                };
                self.script_regen.status.insert(node_id.clone(), status);
                return false;
            }
            ScriptOutcome::Generated(nodes) => nodes,
        };
        // Ids other nodes use, outside this script's own (replaced) subtree.
        let mut taken = HashSet::new();
        for roots in root_lists(&self.doc) {
            collect_ids(roots, &mut taken);
        }
        if let Some(script) = self.find_script(node_id) {
            let mut own = HashSet::new();
            collect_ids(script.frame.children.as_deref().unwrap_or(&[]), &mut own);
            taken.retain(|id| !own.contains(id));
        }
        assign_generated_ids(node_id.as_str(), &mut nodes, &mut taken);
        let count = forest_size(&nodes);
        let mut landed = false;
        for roots in root_lists_mut(&mut self.doc) {
            if let Some(PenNode::Script(script)) = find_node_mut(roots, node_id) {
                script.frame.children = Some(nodes);
                script.script_hash = Some(hash.to_string());
                landed = true;
                break;
            }
        }
        if landed {
            self.mark_document_changed();
            self.script_regen.status.insert(
                node_id.clone(),
                ScriptRunStatus::Done {
                    nodes: count,
                    elapsed_ms,
                },
            );
        }
        landed
    }
}

#[cfg(test)]
#[path = "script_regen_tests.rs"]
mod tests;
