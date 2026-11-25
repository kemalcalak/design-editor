//! Node ids used more than once in a document (#364).
//!
//! Ids must be unique across the whole document: every page, and the
//! content component instances own in their `descendants`
//! ([`crate::instance_ids`]). Documents written before #364 could break that
//! (the id allocator did not see instance content), so opening one reports
//! the duplicates and the user can repair them in one undo step
//! ([`EditorCommand::RepairDuplicateIds`](crate::EditorCommand)). The repair
//! itself lives in [`repair`].
//!
//! Occurrences are counted in document order: pages in order, then the
//! document's own root list; inside each, a node before its instance content
//! and that before its children. Page ids are not counted (they never take
//! part in node lookups), and neither is a replacement root that keeps the
//! id of the node it replaced (that id is the master node's).

use std::collections::HashMap;
use std::fmt;

use crate::id_allocator::IdAllocError;
use crate::instance_ids::{for_each_owned_node, json_node_id};
use crate::pen_node_ext::PenNodeExt;
use crate::state::EditorState;
use jian_ops_schema::node::PenNode;
use jian_ops_schema::PenDocument;
use serde_json::{Map, Value};

mod repair;

pub use repair::{repair_duplicate_ids, repaired_document, RepairedDocument};

/// Label of the document's own root list, which belongs to no page.
const ROOT_LIST_LABEL: &str = "Document";

/// How many node names a report line spells out before "…".
const NAMES_SHOWN: usize = 4;

/// One id that more than one node uses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateIdReport {
    pub id: String,
    /// How many nodes use it.
    pub count: usize,
    /// The pages they are on, in document order, each once.
    pub pages: Vec<String>,
    /// The name of each node using it, in document order.
    pub names: Vec<String>,
}

impl fmt::Display for DuplicateIdReport {
    /// `n12 ×3 (Home, Settings): Title, Card, Row`
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} ×{} ({}): ",
            self.id,
            self.count,
            self.pages.join(", ")
        )?;
        let shown: Vec<&str> = self
            .names
            .iter()
            .take(NAMES_SHOWN)
            .map(String::as_str)
            .collect();
        f.write_str(&shown.join(", "))?;
        if self.names.len() > NAMES_SHOWN {
            f.write_str(", …")?;
        }
        Ok(())
    }
}

/// One node the repair gave a fresh id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenamedId {
    pub old_id: String,
    pub new_id: String,
    /// The page the node is on.
    pub page: String,
    /// The node's name.
    pub name: String,
}

/// Why a duplicate-id repair did not run.
#[derive(Debug)]
pub enum DuplicateIdRepairError {
    /// The id counter ran out.
    IdAlloc(IdAllocError),
    /// The document did not survive its JSON round trip.
    Json { detail: String },
}

impl fmt::Display for DuplicateIdRepairError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IdAlloc(error) => error.fmt(f),
            Self::Json { detail } => write!(f, "duplicate id repair failed: {detail}"),
        }
    }
}

impl std::error::Error for DuplicateIdRepairError {}

impl From<IdAllocError> for DuplicateIdRepairError {
    fn from(error: IdAllocError) -> Self {
        Self::IdAlloc(error)
    }
}

impl From<serde_json::Error> for DuplicateIdRepairError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json {
            detail: error.to_string(),
        }
    }
}

/// The label a page shows in reports: its name, else its id.
pub(crate) fn page_label(name: &str, id: &str) -> String {
    if name.trim().is_empty() {
        id.to_string()
    } else {
        name.to_string()
    }
}

/// The name a node shows in reports: its name, else its type.
pub(crate) fn owned_node_label(node: &Map<String, Value>) -> String {
    node.get("name")
        .and_then(Value::as_str)
        .filter(|name| !name.trim().is_empty())
        .or_else(|| node.get("type").and_then(Value::as_str))
        .unwrap_or("node")
        .to_string()
}

fn node_label(node: &PenNode) -> String {
    node.base()
        .name
        .as_deref()
        .filter(|name| !name.trim().is_empty())
        .unwrap_or(node.type_tag())
        .to_string()
}

/// A node using an id: one in a page tree, or one of an instance's own.
enum Use<'a> {
    Tree(&'a PenNode),
    Owned(&'a Map<String, Value>),
}

impl Use<'_> {
    fn label(&self) -> String {
        match self {
            Use::Tree(node) => node_label(node),
            Use::Owned(node) => owned_node_label(node),
        }
    }
}

/// Visit every use of an id in counting order (see the module docs) with
/// the index of its page (the document's root list comes after the pages).
fn for_each_use<'a>(doc: &'a PenDocument, visit: &mut dyn FnMut(&'a str, usize, Use<'a>)) {
    fn walk<'a>(nodes: &'a [PenNode], page: usize, visit: &mut dyn FnMut(&'a str, usize, Use<'a>)) {
        for node in nodes {
            if !node.id_str().is_empty() {
                visit(node.id_str(), page, Use::Tree(node));
            }
            if let PenNode::Ref(reference) = node {
                if let Some(descendants) = reference.descendants.as_ref() {
                    for_each_owned_node(descendants, &mut |owned, keeps_slot_id| {
                        if let (false, Some(id)) = (keeps_slot_id, json_node_id(owned)) {
                            visit(id, page, Use::Owned(owned));
                        }
                    });
                }
            }
            if let Some(children) = node.children() {
                walk(children, page, visit);
            }
        }
    }
    let pages = doc.pages.as_deref().unwrap_or_default();
    for (index, page) in pages.iter().enumerate() {
        walk(&page.children, index, visit);
    }
    walk(&doc.children, pages.len(), visit);
}

/// Every id more than one node uses, in the order of its first use.
pub fn find_duplicate_ids(doc: &PenDocument) -> Vec<DuplicateIdReport> {
    // Count first, so a document without duplicates (the usual case on
    // open) allocates nothing per node.
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for_each_use(doc, &mut |id, _, _| *counts.entry(id).or_default() += 1);
    if counts.values().all(|count| *count < 2) {
        return Vec::new();
    }
    let pages = doc.pages.as_deref().unwrap_or_default();
    let page_name = |index: usize| {
        pages.get(index).map_or_else(
            || ROOT_LIST_LABEL.to_string(),
            |page| page_label(&page.name, &page.id),
        )
    };
    let mut order: Vec<&str> = Vec::new();
    let mut reports: HashMap<&str, DuplicateIdReport> = HashMap::new();
    for_each_use(doc, &mut |id, page, node| {
        let count = counts[id];
        if count < 2 {
            return;
        }
        let report = reports.entry(id).or_insert_with(|| {
            order.push(id);
            DuplicateIdReport {
                id: id.to_string(),
                count,
                pages: Vec::new(),
                names: Vec::new(),
            }
        });
        let page = page_name(page);
        if !report.pages.contains(&page) {
            report.pages.push(page);
        }
        report.names.push(node.label());
    });
    order
        .into_iter()
        .filter_map(|id| reports.remove(id))
        .collect()
}

impl EditorState {
    /// Every id more than one node of the document uses (#364).
    pub fn duplicate_ids(&self) -> Vec<DuplicateIdReport> {
        find_duplicate_ids(&self.doc)
    }

    /// `EditorCommand::RepairDuplicateIds`: give every later copy of a
    /// duplicated id a fresh one, as one undo step. `Ok(false)` when there is
    /// nothing to repair.
    pub(crate) fn cmd_repair_duplicate_ids(
        &mut self,
        allocator: &mut dyn crate::id_allocator::IdAllocator,
    ) -> Result<bool, IdAllocError> {
        let repaired = match repaired_document(&self.doc, allocator) {
            Ok(Some(repaired)) => repaired,
            Ok(None) => return Ok(false),
            Err(DuplicateIdRepairError::IdAlloc(error)) => return Err(error),
            Err(error @ DuplicateIdRepairError::Json { .. }) => {
                eprintln!("[repair ids] {error}");
                return Ok(false);
            }
        };
        let snap = self.snapshot_for_history();
        self.doc = repaired.doc;
        self.components = crate::components::ComponentLibrary::from_document(&self.doc);
        self.clear_selection();
        self.history_push_past(snap);
        Ok(true)
    }
}

#[cfg(test)]
#[path = "duplicate_ids_tests.rs"]
mod tests;
