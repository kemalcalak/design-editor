//! Keep MCP-inserted page roots out of occupied canvas space (#37).
//!
//! The host calls [`place_new_roots`] with each write command before it
//! applies it. For a tool that creates new content, the command is tried on
//! a copy of the state, the new roots are measured with the same layout
//! `snapshot_layout` reports, and `de_editor_core::root_placement` decides
//! where they go. The answer comes back as one `Batch`: the original
//! command, an `UpdateNode` per moved root, and a `MoveNode` per new root
//! that brings it to the front (a lower index paints in front). Every host
//! applies that same command, so the canvas, the connection thread's copy,
//! and undo all agree.

use std::collections::{BTreeMap, HashSet};

use de_editor_core::geometry::DocRect;
use de_editor_core::root_placement::{plan_root_placement, IncomingRoot, ROOT_GAP};
use de_editor_core::{EditorCommand, EditorState, NodeId, PenNodeExt};
use jian_ops_schema::node::PenNode;
use jian_scene::layout_scene::SceneNode;
use serde_json::Value;

/// Whether `tool` creates new content whose page roots get placed.
/// Duplicate / paste keep their deliberate offset, `use_scene_template`
/// places its own boards, and page tools start fresh pages.
pub fn places_new_roots(tool: &str) -> bool {
    // Every `insert_*` tool (`insert_node` and the kit components) inserts.
    tool.starts_with("insert_")
        || matches!(
            tool,
            "batch_design"
                | "design_skeleton"
                | "design_content"
                | "import_svg"
                | "import_html"
                | "import_pen_dev"
                | "import_pptx"
                | "import_html_url"
                | "import_web_snapshot"
                | "browser"
                | "upsert_screen"
                | "upsert_component"
                | "instantiate_component"
                | "copy_node"
                | "execute"
        )
}

/// Whether the call set `allowOverlap: true`: explicit root positions stay
/// even where they overlap (#37) and `batch_design` skips its sibling
/// overlap warnings (#265).
pub(crate) fn allow_overlap(args: &BTreeMap<String, String>) -> bool {
    ["allowOverlap", "allow_overlap"]
        .iter()
        .any(|key| args.get(*key).is_some_and(|v| v == "true"))
}

/// The command to apply instead of `cmd`, or `None` to apply it unchanged:
/// the tool is out of scope, the command creates no page root, it rejects,
/// or nothing needs moving. `&mut` only so the trial copy can be a
/// `narrowed_snapshot` (no chat / codegen / presets); `state` is left as
/// it was.
pub fn place_new_roots(
    state: &mut EditorState,
    tool: &str,
    args: &BTreeMap<String, String>,
    cmd: &EditorCommand,
) -> Option<EditorCommand> {
    if !places_new_roots(tool) || !may_create_page_root(cmd) {
        return None;
    }
    let ids_before = state.collect_node_ids();
    let roots_before: Vec<Vec<String>> = page_roots(state)
        .into_iter()
        .map(|(_, roots)| roots.iter().map(|n| n.id_str().to_string()).collect())
        .collect();
    let mut trial = de_editor_core::request_snapshot::narrowed_snapshot(state);
    if !trial.apply(cmd.clone()) {
        return None;
    }
    let mut replaced = HashSet::new();
    collect_replaced_ids(cmd, &mut replaced);

    // Which roots are new, per page, before paying for a layout pass.
    let pages: Vec<(usize, Vec<usize>)> = page_roots(&trial)
        .into_iter()
        .enumerate()
        .filter_map(|(idx, (_, roots))| {
            let before = roots_before.get(idx).map(Vec::as_slice).unwrap_or_default();
            let new = new_root_indices(roots, before, &ids_before, &replaced);
            (!new.is_empty()).then_some((idx, new))
        })
        .collect();
    if pages.is_empty() {
        return None;
    }

    let scene = de_pen_loader::editor_state_to_layout_scene(&trial);
    let has_pages = trial.doc.pages.as_ref().is_some_and(|p| !p.is_empty());
    let allow_overlap = allow_overlap(args);
    let origin_from_args =
        defaults_origin_when_unset(tool).then(|| args.contains_key("x") || args.contains_key("y"));
    let trial_pages = page_roots(&trial);
    let mut extra = Vec::new();
    for (idx, new) in pages {
        let (page_id, roots) = trial_pages[idx];
        let scene_roots = scene
            .pages
            .get(idx)
            .map(|page| page.children.as_slice())
            .unwrap_or_default();
        let page = PagePlacement {
            roots,
            new: &new,
            scene_roots,
            page_id: has_pages.then(|| page_id.to_string()),
            origin_from_args,
            allow_overlap,
        };
        page.push_moves(&mut extra);
    }
    if extra.is_empty() {
        return None;
    }
    let mut commands = match cmd {
        EditorCommand::Batch { commands } => commands.clone(),
        other => vec![other.clone()],
    };
    commands.extend(extra);
    Some(EditorCommand::Batch { commands })
}

/// Tools whose command writes an origin even when the call gave none
/// (kit components and `import_svg` default to 0,0), so whether the agent
/// asked for a position has to come from the call's own `x` / `y`.
fn defaults_origin_when_unset(tool: &str) -> bool {
    (tool.starts_with("insert_") && tool != "insert_node") || tool == "import_svg"
}

/// Cheap pre-check: can `cmd` add a page root at all? Parented inserts and
/// edits skip the trial apply and layout entirely.
fn may_create_page_root(cmd: &EditorCommand) -> bool {
    use EditorCommand as C;
    match cmd {
        C::Batch { commands } => commands.iter().any(may_create_page_root),
        C::InsertNode { target_parent, .. }
        | C::CopyNode { target_parent, .. }
        | C::InstantiateKitComponent { target_parent, .. }
        | C::ImportSvg { target_parent, .. } => !target_parent.is_real(),
        C::InsertSubtree { parent_id, .. }
        | C::InsertAuthoredSubtree { parent_id, .. }
        | C::InsertAuthoredSubtreePreservingRoots { parent_id, .. } => !parent_id.is_real(),
        C::BatchInsert { .. }
        | C::InstantiateComponent { .. }
        | C::UpsertScreen { .. }
        | C::UpsertComponent { .. } => true,
        _ => false,
    }
}

/// Ids of the nodes `cmd` replaces (`R()` in a `batch_design` program).
fn collect_replaced_ids(cmd: &EditorCommand, out: &mut HashSet<String>) {
    match cmd {
        EditorCommand::Batch { commands } => {
            for sub in commands {
                collect_replaced_ids(sub, out);
            }
        }
        EditorCommand::ReplaceNode { node_id, .. }
        | EditorCommand::ReplaceSubtree { node_id, .. } => {
            out.insert(node_id.as_str().to_string());
        }
        _ => {}
    }
}

/// Indices into `roots` of the page roots `cmd` created. An id that existed
/// anywhere before is a moved node (`M()`), not new. A replacement (`R()`)
/// mints fresh ids but keeps the replaced root's slot, while every insert
/// appends after the existing roots: so a fresh root that still has an old
/// root after it is a replacement, and a replaced root that was last hands
/// its slot to the first fresh root of the tail.
fn new_root_indices(
    roots: &[PenNode],
    before: &[String],
    ids_before: &HashSet<NodeId>,
    replaced: &HashSet<String>,
) -> Vec<usize> {
    let fresh = |node: &PenNode| !ids_before.contains(&NodeId::new(node.id_str().to_string()));
    let last_old = roots.iter().rposition(|node| !fresh(node));
    let last_old_before = last_old
        .and_then(|i| before.iter().position(|id| id == roots[i].id_str()))
        .map_or(0, |i| i + 1);
    let replaced_at_tail = before[last_old_before.min(before.len())..]
        .iter()
        .filter(|id| replaced.contains(*id))
        .count();
    let tail_start = last_old.map_or(0, |i| i + 1);
    (tail_start..roots.len())
        .filter(|&i| fresh(&roots[i]))
        .skip(replaced_at_tail)
        .collect()
}

/// One page's new roots and what is needed to place them.
struct PagePlacement<'a> {
    roots: &'a [PenNode],
    new: &'a [usize],
    scene_roots: &'a [SceneNode],
    page_id: Option<String>,
    /// `Some(asked)` for tools whose command always writes an origin.
    origin_from_args: Option<bool>,
    allow_overlap: bool,
}

impl PagePlacement<'_> {
    /// Push an `UpdateNode` per root that moves and a `MoveNode` per new
    /// root that is not already in front.
    fn push_moves(&self, out: &mut Vec<EditorCommand>) {
        let existing: Vec<DocRect> = (0..self.roots.len())
            .filter(|i| !self.new.contains(i))
            .filter_map(|i| self.bounds_of(&self.roots[i]))
            .collect();
        // A root the layout did not resolve (nothing to measure) still
        // comes to the front; it just is not moved.
        let measured: Vec<(&PenNode, DocRect)> = self
            .new
            .iter()
            .filter_map(|&i| self.bounds_of(&self.roots[i]).map(|b| (&self.roots[i], b)))
            .collect();
        let incoming: Vec<IncomingRoot> = measured
            .iter()
            .map(|(node, bounds)| IncomingRoot {
                bounds: *bounds,
                positioned: self
                    .origin_from_args
                    .unwrap_or(node.base().x.is_some() || node.base().y.is_some()),
            })
            .collect();
        let plan = plan_root_placement(&existing, &incoming, self.allow_overlap);
        for ((node, bounds), target) in measured.iter().zip(plan) {
            let Some((x, y)) = target else { continue };
            // Bounds may start off the node's own origin (strokes, overflow),
            // so move the origin by the same delta the bounds move.
            let base = node.base();
            out.push(EditorCommand::UpdateNode {
                node_id: NodeId::new(node.id_str().to_string()),
                x: Some((base.x.unwrap_or(0.0) + x - bounds.x).round() as i32),
                y: Some((base.y.unwrap_or(0.0) + y - bounds.y).round() as i32),
                width: None,
                height: None,
                name: None,
                fill_hex: None,
                page_id: self.page_id.clone(),
            });
        }
        // Already in front means the new roots hold the first slots in order.
        if self.new.iter().enumerate().all(|(slot, &i)| slot == i) {
            return;
        }
        for (index, &i) in self.new.iter().enumerate() {
            out.push(EditorCommand::MoveNode {
                node_id: NodeId::new(self.roots[i].id_str().to_string()),
                target_parent: NodeId::NONE,
                page_id: self.page_id.clone(),
                index: Some(index),
            });
        }
    }

    fn bounds_of(&self, node: &PenNode) -> Option<DocRect> {
        let scene = self.scene_roots.iter().find(|n| n.id == node.id_str())?;
        let b = scene.aggregate_bounds();
        Some(DocRect {
            x: f64::from(b.origin.x),
            y: f64::from(b.origin.y),
            w: f64::from(b.size.x),
            h: f64::from(b.size.y),
        })
    }
}

/// `(page id, roots)` for every page, in document order; a document
/// without pages is one page with id `"0"` (as in `read_tools`).
fn page_roots(state: &EditorState) -> Vec<(&str, &[PenNode])> {
    match state.doc.pages.as_ref() {
        Some(pages) if !pages.is_empty() => pages
            .iter()
            .map(|page| (page.id.as_str(), page.children.as_slice()))
            .collect(),
        _ => vec![("0", state.doc.children.as_slice())],
    }
}

/// MCP-facing wording for the placement above: `schema` (one tool
/// descriptor JSON) gains an `allowOverlap` property and a sentence on where
/// new roots land for the tools that place them, and `find_empty_space`
/// says it is optional for new roots. Applied by the MCP host only — the
/// shared schema catalog also feeds the in-app design agent, whose loop
/// does not run this placement.
pub fn advertise_root_placement(schema: &str) -> String {
    let Ok(mut value) = serde_json::from_str::<Value>(schema) else {
        return schema.to_string();
    };
    let Some(name) = value.get("name").and_then(Value::as_str) else {
        return schema.to_string();
    };
    let gap = ROOT_GAP as i32;
    let note = if name == "find_empty_space" {
        " Optional for new page roots: inserts already land right of existing content when they have no x/y or would overlap.".to_string()
    } else if places_new_roots(name) {
        if let Some(props) = value
            .pointer_mut("/inputSchema/properties")
            .and_then(Value::as_object_mut)
        {
            props.insert(
                "allowOverlap".into(),
                serde_json::json!({
                    "type": "boolean",
                    "description": "Keep an explicit page-root x/y even where it overlaps existing roots, and skip warnings about overlapping siblings the write creates (default false)"
                }),
            );
        }
        format!(
            " New page roots land in free canvas space, in front: without x/y right of existing content ({gap}px gap, top-aligned); an explicit x/y that overlaps an existing root moves there too unless allowOverlap is true. Read final positions with snapshot_layout."
        )
    } else {
        return schema.to_string();
    };
    if let Some(Value::String(description)) = value.get_mut("description") {
        description.push_str(&note);
    }
    value.to_string()
}
