//! Document walks the set writes share: find a node on any page, take it
//! out, and visit every instance.

use jian_ops_schema::node::{PenNode, RefNode};
use jian_ops_schema::PenDocument;

use crate::pen_node_ext::PenNodeExt;
use crate::NodeId;

/// The node `id` on any page.
pub fn find<'a>(doc: &'a PenDocument, id: &str) -> Option<&'a PenNode> {
    crate::component_backing::find_node_in_document(doc, id)
}

/// The node `id` on any page, mutably.
pub(crate) fn find_mut<'a>(doc: &'a mut PenDocument, id: &str) -> Option<&'a mut PenNode> {
    let node_id = NodeId::new(id);
    if let Some(pages) = doc.pages.as_mut() {
        for page in pages {
            if crate::walkers::contains_node(&page.children, &node_id) {
                return crate::walkers::find_node_mut(&mut page.children, &node_id);
            }
        }
    }
    crate::walkers::find_node_mut(&mut doc.children, &node_id)
}

/// Where a node sits: its page (`None` = the legacy root list), its
/// parent (`None` = a page root) and its index among its siblings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Place {
    pub page: Option<usize>,
    pub parent: Option<String>,
    pub index: usize,
}

/// Where node `id` sits.
pub(crate) fn place_of(doc: &PenDocument, id: &str) -> Option<Place> {
    fn walk(nodes: &[PenNode], id: &str, parent: Option<&str>) -> Option<(Option<String>, usize)> {
        for (index, node) in nodes.iter().enumerate() {
            if node.id_str() == id {
                return Some((parent.map(str::to_string), index));
            }
            if let Some(children) = node.children() {
                if let Some(hit) = walk(children, id, Some(node.id_str())) {
                    return Some(hit);
                }
            }
        }
        None
    }
    for (page_index, page) in doc.pages.iter().flatten().enumerate() {
        if let Some((parent, index)) = walk(&page.children, id, None) {
            return Some(Place {
                page: Some(page_index),
                parent,
                index,
            });
        }
    }
    walk(&doc.children, id, None).map(|(parent, index)| Place {
        page: None,
        parent,
        index,
    })
}

/// The sibling list a place names.
pub(crate) fn siblings_mut<'a>(
    doc: &'a mut PenDocument,
    place: &Place,
) -> Option<&'a mut Vec<PenNode>> {
    let roots = match place.page {
        Some(page) => &mut doc.pages.as_mut()?.get_mut(page)?.children,
        None => &mut doc.children,
    };
    match &place.parent {
        None => Some(roots),
        Some(parent) => {
            crate::walkers::find_node_mut(roots, &NodeId::new(parent.as_str()))?.children_mut()
        }
    }
}

/// Take node `id` out of the document.
pub(crate) fn take(doc: &mut PenDocument, id: &str) -> Option<PenNode> {
    let place = place_of(doc, id)?;
    let siblings = siblings_mut(doc, &place)?;
    (place.index < siblings.len()).then(|| siblings.remove(place.index))
}

/// Visit every page-tree instance, on every page.
pub(crate) fn for_each_ref_mut(doc: &mut PenDocument, visit: &mut dyn FnMut(&mut RefNode)) {
    fn walk(nodes: &mut [PenNode], visit: &mut dyn FnMut(&mut RefNode)) {
        for node in nodes {
            if let PenNode::Ref(reference) = node {
                visit(reference);
            }
            if let Some(children) = node.children_mut() {
                walk(children, visit);
            }
        }
    }
    for page in doc.pages.iter_mut().flatten() {
        walk(&mut page.children, visit);
    }
    walk(&mut doc.children, visit);
}
