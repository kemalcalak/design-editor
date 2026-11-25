//! Registering `reusable: true` frames that land in the document after it
//! was loaded (#366): an agent's `batch_design` master is a component at
//! once, as it would be after a reload, so `list_components` /
//! `get_component` see it. Split from `components.rs` (800-line cap).

use std::collections::HashMap;

use jian_ops_schema::node::PenNode;
use jian_ops_schema::PenDocument;

use crate::component_backing::{shallow_root, DocumentNodeLocation};
use crate::components::{Component, ComponentLibrary};
use crate::node_id::NodeId;
use crate::pen_node_ext::PenNodeExt;

/// The registry name of a node that defines a component: a `reusable: true`
/// frame, named by its layer name, else its id. `None` for anything else.
/// The one rule [`ComponentLibrary::from_document`] and inserts share.
pub(crate) fn reusable_component_name(node: &PenNode) -> Option<String> {
    let PenNode::Frame(frame) = node else {
        return None;
    };
    (frame.reusable == Some(true)).then(|| {
        frame
            .base
            .name
            .clone()
            .unwrap_or_else(|| frame.base.id.clone())
    })
}

/// `(id, name)` of every component-defining node in `nodes`, depth-first.
pub(crate) fn collect_reusable_frames(nodes: &[PenNode], out: &mut Vec<(NodeId, String)>) {
    for node in nodes {
        if let Some(name) = reusable_component_name(node) {
            out.push((NodeId::new(node.id_str()), name));
        }
        if let Some(children) = node.children() {
            collect_reusable_frames(children, out);
        }
    }
}

impl ComponentLibrary {
    /// Register `masters` (ids already in `doc`) with one document walk that
    /// stops once every id is found. Ids `doc` does not hold are skipped.
    pub(crate) fn register_document_components(
        &mut self,
        doc: &PenDocument,
        masters: Vec<(NodeId, String)>,
    ) {
        let mut pending: HashMap<String, String> = masters
            .into_iter()
            .map(|(id, name)| (id.as_str().to_string(), name))
            .collect();
        let mut found = Vec::new();
        let mut path = Vec::new();
        let pages = doc.pages.iter().flatten().enumerate();
        for (page_index, page) in pages {
            locate(
                &page.children,
                Some(page_index),
                &mut path,
                &mut pending,
                &mut found,
            );
        }
        locate(&doc.children, None, &mut path, &mut pending, &mut found);
        for (node, name, location) in found {
            self.insert_document_backed(
                Component {
                    id: NodeId::new(node.id_str()),
                    name,
                    root: shallow_root(node),
                },
                location,
            );
        }
    }
}

fn locate<'a>(
    nodes: &'a [PenNode],
    page_index: Option<usize>,
    path: &mut Vec<usize>,
    pending: &mut HashMap<String, String>,
    found: &mut Vec<(&'a PenNode, String, DocumentNodeLocation)>,
) {
    for (index, node) in nodes.iter().enumerate() {
        if pending.is_empty() {
            return;
        }
        path.push(index);
        if let Some(name) = pending.remove(node.id_str()) {
            found.push((
                node,
                name,
                DocumentNodeLocation {
                    page_index,
                    child_path: path.clone().into_boxed_slice(),
                },
            ));
        }
        if let Some(children) = node.children() {
            locate(children, page_index, path, pending, found);
        }
        path.pop();
    }
}
