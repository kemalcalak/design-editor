//! Which part of the active page a write (or a whole design-agent run)
//! created or changed.
//!
//! The post-write repairs and the loop finalizer tidy AI-generated designs.
//! Run over the whole page they also rewrite designs a person made or
//! imported (#107, #116), so callers capture a [`WriteBaseline`] before they
//! write and repair only the [`WriteScope`] the write produced.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::hash::Hasher;

use jian_ops_schema::node::PenNode;

use crate::{EditorState, PenNodeExt};

/// What the active page looked like before a write: which page it was and
/// the fingerprints of every node on it (its own fields, children excluded).
#[derive(Debug, Clone)]
pub struct WriteBaseline {
    page: Option<String>,
    /// Fingerprints per id; a list because a document may repeat an id.
    fingerprints: HashMap<String, Vec<u64>>,
}

/// The part of the active page one write created or changed.
#[derive(Debug, Default)]
pub struct WriteScope {
    /// Top-level roots the write created, in page order.
    pub new_roots: Vec<String>,
    /// Pre-existing roots the write inserted new nodes into, in page order.
    pub extended_roots: Vec<String>,
    /// Every node that is new or whose own fields changed.
    pub touched: HashSet<String>,
}

impl WriteScope {
    /// True for a root the write created.
    pub fn is_new_root(&self, id: &str) -> bool {
        self.new_roots.iter().any(|root| root == id)
    }

    /// True for a root the write created or inserted nodes into.
    pub fn is_new_or_extended_root(&self, id: &str) -> bool {
        self.is_new_root(id) || self.extended_roots.iter().any(|root| root == id)
    }
}

impl WriteBaseline {
    pub fn capture(state: &EditorState) -> Self {
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
    pub fn scope_after(&self, state: &EditorState) -> WriteScope {
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

    /// The top-level roots created since this baseline: the
    /// [`WriteScope::new_roots`] of [`Self::scope_after`], without
    /// fingerprinting the page again.
    pub fn new_roots_after(&self, state: &EditorState) -> BTreeSet<String> {
        if active_page_id(state) != self.page {
            return BTreeSet::new();
        }
        state
            .active_children()
            .iter()
            .map(PenNodeExt::id_str)
            .filter(|id| !self.fingerprints.contains_key(*id))
            .map(str::to_string)
            .collect()
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
fn node_fingerprints(root: &PenNode) -> Vec<(String, u64)> {
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

#[cfg(test)]
#[path = "write_scope_tests.rs"]
mod tests;
