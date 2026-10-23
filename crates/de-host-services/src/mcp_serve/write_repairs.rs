//! Post-write repairs for bare-MCP writes, scoped to what the write touched.
//!
//! The repairs tidy AI-generated designs (status bars on phone screens, icon
//! paths, empty image slots, a bottom nav appended after a fixed-height
//! wrapper). They used to walk the whole active page, so an unrelated write
//! rewrote any design a person made or imported (#107). Each write now
//! captures a [`WriteBaseline`] before it applies and repairs only the
//! [`WriteScope`] it produced:
//!
//! - status bar / viewport height: roots the write created;
//! - trailing-nav reflow: roots the write created or added nodes into;
//! - icon paths / image slots: nodes the write created or changed.

use std::collections::{HashMap, HashSet};
use std::hash::Hasher;

use de_editor_core::{EditorState, PenNodeExt};

/// Tools whose writes must land exactly as authored, so no repair runs after
/// them:
/// - `import_pen_dev` creates new roots, which [`WriteScope`] would take for
///   AI-generated screens; repairing them rewrites someone else's work.
/// - `undo` / `redo` restore an earlier document; a root they bring back
///   would look new and be repaired, so the step would not round-trip.
pub(crate) fn writes_verbatim(tool_name: &str) -> bool {
    matches!(tool_name, "import_pen_dev" | "undo" | "redo")
}

/// What the active page looked like before a write: which page it was and
/// the fingerprints of every node on it (its own fields, children excluded).
pub(crate) struct WriteBaseline {
    page: Option<String>,
    /// Fingerprints per id; a list because a document may repeat an id.
    fingerprints: HashMap<String, Vec<u64>>,
}

/// The part of the active page one write created or changed.
#[derive(Debug, Default)]
pub(crate) struct WriteScope {
    /// Top-level roots the write created, in page order.
    pub(crate) new_roots: Vec<String>,
    /// Pre-existing roots the write inserted new nodes into, in page order.
    pub(crate) extended_roots: Vec<String>,
    /// Every node that is new or whose own fields changed.
    pub(crate) touched: HashSet<String>,
}

impl WriteBaseline {
    /// The baseline every MCP applier captures right before applying a write
    /// of `tool_name`; `None` for a tool whose writes stay verbatim.
    pub(crate) fn for_tool(tool_name: &str, state: &EditorState) -> Option<Self> {
        (!writes_verbatim(tool_name)).then(|| Self::capture(state))
    }

    pub(crate) fn capture(state: &EditorState) -> Self {
        let mut fingerprints: HashMap<String, Vec<u64>> = HashMap::new();
        for root in state.active_children() {
            for (id, fingerprint) in node_fingerprints(root) {
                fingerprints.entry(id).or_default().push(fingerprint);
            }
        }
        Self {
            page: active_page_id(state),
            fingerprints,
        }
    }

    /// Diff the active page after the write against this baseline. A write
    /// that switched the active page (`set_active_page`, `add_page`, …) did
    /// not create the roots it now shows, so its scope is empty.
    pub(crate) fn scope_after(&self, state: &EditorState) -> WriteScope {
        let mut scope = WriteScope::default();
        if active_page_id(state) != self.page {
            return scope;
        }
        for root in state.active_children() {
            let root_id = root.id_str();
            // Absent from the whole page before, not just from its top level:
            // a frame ungrouped or moved up to the root was not created.
            let root_is_new = !self.fingerprints.contains_key(root_id);
            let mut gained_nodes = false;
            for (id, fingerprint) in node_fingerprints(root) {
                match self.fingerprints.get(&id) {
                    None => {
                        gained_nodes = true;
                        scope.touched.insert(id);
                    }
                    Some(before) if !before.contains(&fingerprint) => {
                        scope.touched.insert(id);
                    }
                    Some(_) => {}
                }
            }
            if root_is_new {
                scope.new_roots.push(root_id.to_string());
            } else if gained_nodes {
                scope.extended_roots.push(root_id.to_string());
            }
        }
        scope
    }
}

/// Id of the page `active_children` reads, or `None` for a page-less
/// document (its single implicit page cannot change).
fn active_page_id(state: &EditorState) -> Option<String> {
    let pages = state.doc.pages.as_ref().filter(|pages| !pages.is_empty())?;
    let index = state.ui.active_page_index.min(pages.len() - 1);
    Some(pages[index].id.clone())
}

/// `(id, fingerprint)` for `root` and every descendant. The fingerprint
/// hashes the node's serialized fields minus `children`, so a node counts as
/// changed only when its own fields change, not when a descendant does.
fn node_fingerprints(root: &jian_ops_schema::node::PenNode) -> Vec<(String, u64)> {
    /// Feeds serialized JSON straight into the hasher, so no field is
    /// rendered to an intermediate string.
    struct HashWriter<'a>(&'a mut std::collections::hash_map::DefaultHasher);

    impl std::io::Write for HashWriter<'_> {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.write(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn walk(value: &serde_json::Value, out: &mut Vec<(String, u64)>) {
        let Some(object) = value.as_object() else {
            return;
        };
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        for (key, field) in object {
            if key != "children" {
                hasher.write(key.as_bytes());
                let _ = serde_json::to_writer(HashWriter(&mut hasher), field);
            }
        }
        if let Some(id) = object.get("id").and_then(serde_json::Value::as_str) {
            out.push((id.to_string(), hasher.finish()));
        }
        if let Some(children) = object.get("children").and_then(serde_json::Value::as_array) {
            for child in children {
                walk(child, out);
            }
        }
    }
    let mut out = Vec::new();
    if let Ok(value) = serde_json::to_value(root) {
        walk(&value, &mut out);
    }
    out
}

pub(crate) fn normalize_mobile_screens_after_apply(
    state: &mut EditorState,
    in_scope: impl Fn(&str) -> bool,
) -> de_chat_agent::MobileNormalizeReport {
    let report = de_chat_agent::normalize_mobile_screens_for_roots(state, in_scope);
    let mut changes = Vec::new();
    if report.status_bars_inserted > 0 {
        changes.push(format!(
            "status_bars_inserted={}",
            report.status_bars_inserted
        ));
    }
    if report.status_bars_replaced > 0 {
        changes.push(format!(
            "status_bars_replaced={}",
            report.status_bars_replaced
        ));
    }
    if report.duplicate_status_bars_removed > 0 {
        changes.push(format!(
            "duplicate_status_bars_removed={}",
            report.duplicate_status_bars_removed
        ));
    }
    if report.viewport_heights_fixed > 0 {
        changes.push(format!(
            "viewport_heights_fixed={}",
            report.viewport_heights_fixed
        ));
    }
    if !changes.is_empty() {
        eprintln!(
            "design-editor mcp: mobile normalization: {}",
            changes.join(", ")
        );
    }
    report
}

pub(crate) fn normalize_icon_paths_after_apply(
    state: &mut EditorState,
    in_scope: impl Fn(&str) -> bool,
) -> de_editor_core::icon_path_normalize::IconPathNormalizeReport {
    let report = de_editor_core::icon_path_normalize::normalize_icon_paths_for_ids(
        state,
        de_editor_ui::widgets::icons::lucide_name_for_path_d,
        in_scope,
    );
    if report != Default::default() {
        eprintln!(
            "design-editor mcp: icon path normalization: converted_to_icon_font={}, refit_uniform={}",
            report.converted_to_icon_font, report.refit_uniform
        );
    }
    report
}

/// Run the cheap, idempotent repair passes shared by every bare-MCP document
/// write over the part of the page `scope` names, after the command itself
/// has applied and before it saves. Returns whether the trailing-nav reflow
/// or the image-slot pass changed the document: those edit roots that were
/// already laid out, while status bars and heights land only on roots this
/// write created, which the caller already treats as a layout change.
pub(crate) fn run_write_repairs_after_apply(state: &mut EditorState, scope: &WriteScope) -> bool {
    if scope.touched.is_empty() {
        return false;
    }
    let is_new_root = |id: &str| scope.new_roots.iter().any(|root| root == id);
    let is_touched = |id: &str| scope.touched.contains(id);
    normalize_mobile_screens_after_apply(state, is_new_root);
    normalize_icon_paths_after_apply(state, is_touched);
    let mobile_nav_reflowed =
        de_orchestrator::repair_mobile_trailing_nav_reflow_for_roots(state, |id| {
            is_new_root(id) || scope.extended_roots.iter().any(|root| root == id)
        });
    let image_slots_materialized =
        de_orchestrator::cleanup_image_slots::materialize_empty_image_fill_slots_for_ids(
            state, is_touched,
        );
    mobile_nav_reflowed || image_slots_materialized
}

#[cfg(test)]
#[path = "write_repairs_tests.rs"]
mod tests;
