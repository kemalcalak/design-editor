//! When a script node's children are regenerated, and how a result lands
//! (#19).
//!
//! Core owns the decision, the host owns the engine. A script's output is
//! a pure function of its source, its resolved inputs and its size, so a
//! fingerprint of those three ([`script_fingerprint`]) is compared with the
//! `scriptHash` its saved children were generated from. The host asks for
//! work with [`EditorState::take_script_jobs`], which walks the document
//! only when something a run depends on may have moved (the document
//! revision, the active theme, the active page), never per paint, and
//! never hands out a second job for a node whose job is still running.
//! Results come back through [`EditorState::land_script_output`], which
//! drops a result whose fingerprint no longer matches the node (the node
//! was edited while the job ran) and writes a current one without an undo
//! step: generated children are derived data, and every history snapshot
//! already carries the children and `scriptHash` that belong to it.

use std::collections::{BTreeMap, HashMap, HashSet};

use jian_ops_schema::node::{PenNode, ScriptNode};
use jian_ops_schema::sizing::{SizingBehavior, SizingKeyword};
use jian_ops_schema::PenDocument;
use serde_json::Value;

use crate::node_id::NodeId;
use crate::pen_node_ext::PenNodeExt;
use crate::script_inputs::{resolved_script_inputs, script_input_specs};
use crate::state::EditorState;
use crate::variables_resolve::effective_theme;
use crate::walkers::find_node_mut;
use crate::web_sync::{fnv1a64, fnv1a64_extend};

type Theme = BTreeMap<String, String>;

/// The laid-out size of a node, when the host has a layout. A script sized
/// `fill_container` (or by an expression) runs at this size.
pub type LaidOutSize<'a> = &'a dyn Fn(&NodeId) -> Option<(f64, f64)>;

/// For hosts without a layout: only fixed sizes are known.
pub fn no_layout(_: &NodeId) -> Option<(f64, f64)> {
    None
}

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
    /// Why the run failed, as the panel shows it. Display text only: core
    /// never branches on the reason, and the engine's typed error
    /// (`de_mcp::script_node::ScriptNodeError`) lives in a crate core
    /// cannot depend on, so it crosses as its message.
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

/// What a scan depends on besides the document: the theme the inputs
/// resolve under and the page whose layout gives fill sizes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ScanKey {
    generation: u64,
    revision: u64,
    theme: u64,
    active_page: usize,
}

/// Runtime bookkeeping. Never saved, never in history.
#[derive(Debug, Clone, Default)]
pub struct ScriptRegenState {
    /// What the last scan saw; `None` forces the next scan.
    scanned: Option<ScanKey>,
    /// Node → fingerprint of its running job.
    in_flight: HashMap<NodeId, String>,
    status: HashMap<NodeId, ScriptRunStatus>,
    /// Node → its source's hash and declared inputs, so a scan (every frame
    /// of a drag) compares an unchanged source instead of hashing and
    /// parsing it again.
    sources: HashMap<NodeId, SourceFacts>,
    /// The document generation `sources` belongs to.
    sources_generation: u64,
}

/// What a scan derives from a script's source alone.
#[derive(Debug, Clone)]
struct SourceFacts {
    source: String,
    hash: u64,
    specs: Vec<crate::script_inputs::ScriptInputSpec>,
}

/// `source`'s facts, from `cache` while the source is unchanged.
fn source_facts<'c>(
    cache: &'c mut HashMap<NodeId, SourceFacts>,
    id: &NodeId,
    source: &str,
) -> &'c SourceFacts {
    let stale = cache.get(id).is_none_or(|facts| facts.source != source);
    if stale {
        let facts = SourceFacts {
            source: source.to_string(),
            hash: fnv1a64(source.as_bytes()),
            specs: script_input_specs(source),
        };
        cache.insert(id.clone(), facts);
    }
    &cache[id]
}

impl ScriptRegenState {
    /// Status of `id`'s script, if it has run in this session.
    pub fn status(&self, id: &NodeId) -> Option<&ScriptRunStatus> {
        self.status.get(id)
    }

    /// Whether any job is still running.
    pub fn has_running_jobs(&self) -> bool {
        !self.in_flight.is_empty()
    }
}

/// The size a script runs at, per axis: a fixed size as authored; a fill
/// (or expression) size as laid out; a hug or unset size is 0, because a
/// hug box would follow the children the run itself produces. `None` when
/// the size depends on a layout the host does not have yet.
fn script_size(script: &ScriptNode, layout: LaidOutSize<'_>) -> Option<(f64, f64)> {
    let id = NodeId::new(script.frame.base.id.as_str());
    let mut laid_out: Option<Option<(f64, f64)>> = None;
    let mut axis = |sizing: Option<&SizingBehavior>, pick: fn((f64, f64)) -> f64| match sizing {
        Some(SizingBehavior::Number(n)) => Some(*n),
        Some(SizingBehavior::Keyword(SizingKeyword::FitContent)) | None => Some(0.0),
        Some(_) => laid_out.get_or_insert_with(|| layout(&id)).map(pick),
    };
    let width = axis(script.frame.container.width.as_ref(), |s| s.0)?;
    let height = axis(script.frame.container.height.as_ref(), |s| s.1)?;
    Some((width, height))
}

/// Fingerprint of everything a run depends on, as 16 hex digits. FNV-1a is
/// stable across runs and platforms, unlike `std`'s randomly keyed hasher,
/// so a saved `scriptHash` stays comparable.
pub fn script_fingerprint(
    source: &str,
    width: f64,
    height: f64,
    inputs: &BTreeMap<String, Value>,
) -> String {
    fingerprint_from_source_hash(fnv1a64(source.as_bytes()), width, height, inputs)
}

fn fingerprint_from_source_hash(
    source_hash: u64,
    width: f64,
    height: f64,
    inputs: &BTreeMap<String, Value>,
) -> String {
    let mut hash = fnv1a64_extend(source_hash, &width.to_bits().to_le_bytes());
    hash = fnv1a64_extend(hash, &height.to_bits().to_le_bytes());
    // A `BTreeMap` serializes in key order, so equal maps hash equally.
    let inputs = serde_json::to_string(inputs).unwrap_or_default();
    format!("{:016x}", fnv1a64_extend(hash, inputs.as_bytes()))
}

/// A script as it would run now, without its source copied.
struct Run<'a> {
    source: &'a str,
    width: f64,
    height: f64,
    inputs: BTreeMap<String, Value>,
    hash: String,
}

impl Run<'_> {
    fn into_job(self, node_id: NodeId) -> ScriptJob {
        ScriptJob {
            node_id,
            source: self.source.to_string(),
            width: self.width,
            height: self.height,
            inputs: self.inputs,
            hash: self.hash,
        }
    }
}

/// How `script` would run under `theme` (its own and its ancestors' theme
/// scopes already merged). `None` without a source or a known size.
fn current_run<'a>(
    script: &'a ScriptNode,
    doc: &PenDocument,
    theme: &Theme,
    layout: LaidOutSize<'_>,
    sources: &mut HashMap<NodeId, SourceFacts>,
) -> Option<Run<'a>> {
    let source = script.script.as_deref().filter(|s| !s.trim().is_empty())?;
    let (width, height) = script_size(script, layout)?;
    let id = NodeId::new(script.frame.base.id.as_str());
    let facts = source_facts(sources, &id, source);
    let inputs = resolved_script_inputs(
        &facts.specs,
        script.inputs.as_ref(),
        doc.variables.as_ref(),
        theme,
    );
    Some(Run {
        source,
        width,
        height,
        hash: fingerprint_from_source_hash(facts.hash, width, height, &inputs),
        inputs,
    })
}

/// Every page's root list (the single-page fallback when there are no
/// pages; a document with pages keeps no live roots outside them).
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

/// `theme` with `node`'s own `theme` axes laid over it, as the canvas
/// resolves a subtree (`variables_resolve::resolve_node_for_canvas`).
fn scoped(theme: &Theme, node: &PenNode) -> Option<Theme> {
    let own = node.base().theme.as_ref().filter(|own| !own.is_empty())?;
    let mut merged = theme.clone();
    merged.extend(
        own.iter()
            .map(|(axis, value)| (axis.clone(), value.clone())),
    );
    Some(merged)
}

/// Every script node with the theme its inputs resolve under. A script's
/// own children are generated, so the walk does not enter them.
fn visit_scripts<'a>(nodes: &'a [PenNode], theme: &Theme, out: &mut Vec<(&'a ScriptNode, Theme)>) {
    for node in nodes {
        let own = scoped(theme, node);
        let theme = own.as_ref().unwrap_or(theme);
        if let PenNode::Script(script) = node {
            out.push((script, theme.clone()));
        } else if let Some(children) = node.children() {
            visit_scripts(children, theme, out);
        }
    }
}

/// The script node `id` with the theme its inputs resolve under.
fn find_script<'a>(
    nodes: &'a [PenNode],
    id: &str,
    theme: &Theme,
) -> Option<(&'a ScriptNode, Theme)> {
    nodes.iter().find_map(|node| {
        let own = scoped(theme, node);
        let theme = own.as_ref().unwrap_or(theme);
        match node {
            PenNode::Script(script) if script.frame.base.id == id => Some((script, theme.clone())),
            PenNode::Script(_) => None,
            _ => find_script(node.children()?, id, theme),
        }
    })
}

/// Ids starting with `<script id>-` anywhere outside that script's own
/// subtree: the only ids its generated `<script id>-<n>` ids can collide
/// with.
fn ids_sharing_prefix(nodes: &[PenNode], script_id: &str, prefix: &str, out: &mut HashSet<String>) {
    for node in nodes {
        let id = node.id_str();
        if id == script_id {
            continue;
        }
        if id.starts_with(prefix) {
            out.insert(id.to_string());
        }
        if let Some(children) = node.children() {
            ids_sharing_prefix(children, script_id, prefix, out);
        }
    }
}

/// Generated nodes get `<script id>-<n>` ids in depth-first order, so a
/// regeneration keeps the ids of nodes that stay put (selection and
/// hover survive). An id some other node already uses gets a suffix.
fn assign_generated_ids(script_id: &str, nodes: &mut [PenNode], taken: &mut HashSet<String>) {
    fn walk(script_id: &str, nodes: &mut [PenNode], next: &mut usize, taken: &mut HashSet<String>) {
        for node in nodes {
            let mut id = format!("{script_id}-{next}");
            let mut bump = 1;
            while taken.contains(&id) {
                id = format!("{script_id}-{next}-{bump}");
                bump += 1;
            }
            *next += 1;
            taken.insert(id.clone());
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

/// Whether any script node's size comes from layout (fill or expression).
fn any_layout_sized(nodes: &[PenNode]) -> bool {
    nodes.iter().any(|node| match node {
        PenNode::Script(script) => [
            &script.frame.container.width,
            &script.frame.container.height,
        ]
        .into_iter()
        .any(|sizing| {
            matches!(
                sizing,
                Some(SizingBehavior::Keyword(SizingKeyword::FillContainer))
                    | Some(SizingBehavior::Expression(_))
            )
        }),
        _ => node
            .children()
            .is_some_and(|children| any_layout_sized(children)),
    })
}

impl EditorState {
    /// Whether a scan needs a layout: hosts that compute one on demand
    /// (the file-backed MCP servers) skip it when no script is fill-sized.
    pub fn scripts_need_layout(&self) -> bool {
        root_lists(&self.doc).into_iter().any(any_layout_sized)
    }

    fn script_scan_key(&self) -> ScanKey {
        let theme = serde_json::to_string(&self.ui.variables.active_theme).unwrap_or_default();
        ScanKey {
            generation: self.document_generation(),
            revision: self.revision,
            theme: fnv1a64(theme.as_bytes()),
            active_page: self.ui.active_page_index,
        }
    }

    /// Scripts whose saved children are stale, each marked running. Empty
    /// without a scan when nothing a run depends on moved since the last
    /// call, and never a second job for a node still running. `layout`
    /// gives fill-sized scripts their size; one with no layout yet waits.
    pub fn take_script_jobs(&mut self, layout: LaidOutSize<'_>) -> Vec<ScriptJob> {
        let key = self.script_scan_key();
        if self.script_regen.scanned == Some(key) {
            return Vec::new();
        }
        self.script_regen.scanned = Some(key);
        let theme = effective_theme(&self.doc, &self.ui.variables.active_theme);
        let mut scripts = Vec::new();
        for roots in root_lists(&self.doc) {
            visit_scripts(roots, &theme, &mut scripts);
        }
        let regen = &mut self.script_regen;
        if regen.sources_generation != key.generation {
            // Another document: cached sources name nodes it may not have.
            regen.sources.clear();
            regen.sources_generation = key.generation;
        }
        let mut jobs = Vec::new();
        for (script, theme) in scripts {
            let id = NodeId::new(script.frame.base.id.as_str());
            if regen.in_flight.contains_key(&id) {
                continue;
            }
            let Some(run) = current_run(script, &self.doc, &theme, layout, &mut regen.sources)
            else {
                continue;
            };
            let current = script.script_hash.as_deref() == Some(run.hash.as_str());
            let failed_before = matches!(regen.status.get(&id),
                Some(ScriptRunStatus::Failed { hash, .. }) if *hash == run.hash);
            if !current && !failed_before {
                jobs.push(run.into_job(id));
            }
        }
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

    /// Land a finished run. Returns true when the document changed.
    /// `layout` must answer as it did for [`Self::take_script_jobs`].
    pub fn land_script_output(
        &mut self,
        node_id: &NodeId,
        hash: &str,
        outcome: ScriptOutcome,
        elapsed_ms: u64,
        layout: LaidOutSize<'_>,
    ) -> bool {
        self.script_regen.in_flight.remove(node_id);
        // Whatever happens, look again: an edit made while this ran may
        // need a fresh job.
        self.script_regen.scanned = None;
        let theme = effective_theme(&self.doc, &self.ui.variables.active_theme);
        let found = root_lists(&self.doc)
            .into_iter()
            .find_map(|roots| find_script(roots, node_id.as_str(), &theme));
        let current = found
            .as_ref()
            .and_then(|(script, theme)| {
                current_run(
                    script,
                    &self.doc,
                    theme,
                    layout,
                    &mut self.script_regen.sources,
                )
            })
            .map(|run| run.hash);
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
        let prefix = format!("{}-", node_id.as_str());
        let mut taken = HashSet::new();
        for roots in root_lists(&self.doc) {
            ids_sharing_prefix(roots, node_id.as_str(), &prefix, &mut taken);
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
