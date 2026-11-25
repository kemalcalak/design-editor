//! Read-only questions about component instances (#365), over just the
//! document, the component registry and one page — so MCP tools can ask
//! them on their request snapshot without cloning the whole editor state.

use std::collections::{BTreeMap, HashSet};

use jian_ops_schema::node::PenNode;
use jian_ops_schema::PenDocument;

use crate::components::ComponentLibrary;
use crate::id_allocator::{IdAllocError, IdAllocator};
use crate::instance_error::InstanceError;
use crate::pen_node_ext::PenNodeExt;
use crate::ref_resolve::{find_component_node, materialize_instance};
use crate::{walkers, EditorState, NodeId};

/// One place a master is used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceUse {
    /// `None` for a document without pages.
    pub page_id: Option<String>,
    pub page_name: Option<String>,
    pub instance_id: String,
    /// Entries in the instance's `descendants`.
    pub override_count: usize,
}

/// A node of a master that an instance's `descendants` can name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverrideTarget {
    pub id: String,
    pub name: Option<String>,
    pub kind: &'static str,
}

/// The document and registry seen from one page (the "active" page of the
/// checks below).
#[derive(Debug, Clone, Copy)]
pub struct InstanceView<'a> {
    pub doc: &'a PenDocument,
    pub components: &'a ComponentLibrary,
    pub page_index: usize,
}

impl EditorState {
    /// The instance checks over this state's active page.
    pub fn instance_view(&self) -> InstanceView<'_> {
        InstanceView {
            doc: &self.doc,
            components: &self.components,
            page_index: self.ui.active_page_index,
        }
    }
}

impl<'a> InstanceView<'a> {
    /// The same document seen from another page.
    pub fn on_page(self, page_index: usize) -> Self {
        Self { page_index, ..self }
    }

    /// The page's root nodes, like `EditorState::active_children`.
    pub fn page_children(&self) -> &'a [PenNode] {
        match self.doc.pages.as_ref() {
            Some(pages) if !pages.is_empty() => {
                &pages[self.page_index.min(pages.len() - 1)].children
            }
            _ => &self.doc.children,
        }
    }

    /// Index of the page holding `node_id`, if any.
    pub fn find_node_page(&self, node_id: &NodeId) -> Option<usize> {
        match self.doc.pages.as_ref().filter(|pages| !pages.is_empty()) {
            Some(pages) => pages
                .iter()
                .position(|page| walkers::contains_node(&page.children, node_id)),
            None => walkers::contains_node(&self.doc.children, node_id).then_some(0),
        }
    }

    /// Whether `node` is a component master: registered, or a `reusable`
    /// frame (library and kit masters are both).
    pub fn is_component_master(&self, node: &PenNode) -> bool {
        matches!(node, PenNode::Frame(frame) if frame.reusable == Some(true))
            || self
                .components
                .find_by_id(&NodeId::new(node.id_str()))
                .is_some()
    }

    /// The master `component_id` names, from any page.
    pub fn component_master(&self, component_id: &str) -> Result<PenNode, InstanceError> {
        let master = find_component_node(self.doc, component_id)
            .ok_or_else(|| InstanceError::ComponentNotFound(component_id.to_string()))?;
        if self.is_component_master(&master) {
            Ok(master)
        } else {
            Err(InstanceError::NotAComponent(component_id.to_string()))
        }
    }

    /// Check that an instance of `component_id` may go under `parent` on
    /// the page (`NONE` = page root); returns the master.
    pub fn check_instance_insert(
        &self,
        component_id: &NodeId,
        parent: &NodeId,
    ) -> Result<PenNode, InstanceError> {
        let master = self.component_master(component_id.as_str())?;
        if !parent.is_real() {
            return Ok(master);
        }
        let not_found = || InstanceError::ParentNotFound(parent.as_str().to_string());
        let path = ancestor_path(self.page_children(), parent.as_str()).ok_or_else(not_found)?;
        let target = path.last().ok_or_else(not_found)?;
        if !target.is_container() {
            return Err(InstanceError::ParentNotContainer(
                parent.as_str().to_string(),
            ));
        }
        for ancestor in &path {
            // An instance stands for its master: going inside one means
            // going inside that master.
            let shown = match ancestor {
                PenNode::Ref(reference) => reference.target.as_str(),
                other if self.is_component_master(other) => other.id_str(),
                _ => continue,
            };
            if shown == component_id.as_str()
                || master_uses(self.doc, component_id.as_str(), shown, &mut HashSet::new())
            {
                return Err(InstanceError::Cycle {
                    component: component_id.as_str().to_string(),
                    parent: parent.as_str().to_string(),
                });
            }
        }
        Ok(master)
    }

    /// The instance `node_id` on the page.
    pub fn instance_node(&self, node_id: &NodeId) -> Result<&'a PenNode, InstanceError> {
        match walkers::find_node(self.page_children(), node_id) {
            None => Err(InstanceError::NodeNotFound(node_id.as_str().to_string())),
            Some(node @ PenNode::Ref(_)) => Ok(node),
            Some(_) => Err(InstanceError::NotAnInstance(node_id.as_str().to_string())),
        }
    }

    /// Check that the instance `node_id` on the page may show
    /// `component_id` instead. Swapping goes through the registry, like
    /// the property panel's picker.
    pub fn check_instance_swap(
        &self,
        node_id: &NodeId,
        component_id: &NodeId,
    ) -> Result<(), InstanceError> {
        if let PenNode::Ref(reference) = self.instance_node(node_id)? {
            if reference.target == component_id.as_str() {
                return Err(InstanceError::SameComponent(
                    component_id.as_str().to_string(),
                ));
            }
        }
        if self.components.find_by_id(component_id).is_none() {
            self.component_master(component_id.as_str())?;
            return Err(InstanceError::NotAComponent(
                component_id.as_str().to_string(),
            ));
        }
        let parent = walkers::find_parent_and_index(self.page_children(), node_id)
            .and_then(|(parent, _)| parent)
            .unwrap_or(NodeId::NONE);
        self.check_instance_insert(component_id, &parent).map(|_| ())
    }

    /// The independent subtree an instance detaches into: its master with
    /// the instance's overrides applied. The root keeps the instance's id;
    /// inner nodes get fresh ids from `allocator`. `Ok(None)` when the node
    /// is no instance or its master is gone.
    pub fn detached_instance(
        &self,
        instance: &PenNode,
        allocator: &mut dyn IdAllocator,
        taken: &mut HashSet<NodeId>,
    ) -> Result<Option<PenNode>, IdAllocError> {
        let PenNode::Ref(reference) = instance else {
            return Ok(None);
        };
        let Some(master) = find_component_node(self.doc, &reference.target) else {
            return Ok(None);
        };
        let Some(merged) = materialize_instance(instance, &master) else {
            return Ok(None);
        };
        let mut detached = walkers::deep_clone_with_allocator(&merged, allocator, taken)?;
        detached.base_mut().id = reference.base.id.clone();
        Ok(Some(detached))
    }

    /// Every instance of `component_id`, in page order.
    pub fn list_instances(&self, component_id: &str) -> Vec<InstanceUse> {
        let mut out = Vec::new();
        if let Some(pages) = self.doc.pages.as_ref() {
            for page in pages {
                collect_instances(
                    &page.children,
                    component_id,
                    (Some(&page.id), Some(&page.name)),
                    &mut out,
                );
            }
        }
        collect_instances(&self.doc.children, component_id, (None, None), &mut out);
        out
    }

    /// The master's nodes an instance's `descendants` can name, depth
    /// first. A nested instance is listed itself; its content is reached
    /// with `nested/leaf` keys.
    pub fn override_targets(&self, component_id: &str) -> Vec<OverrideTarget> {
        let mut out = Vec::new();
        if let Ok(master) = self.component_master(component_id) {
            for child in master.children().into_iter().flatten() {
                collect_targets(child, &mut out);
            }
        }
        out
    }
}

/// The nodes from a page root down to `target`, both included.
fn ancestor_path<'a>(nodes: &'a [PenNode], target: &str) -> Option<Vec<&'a PenNode>> {
    for node in nodes {
        if node.id_str() == target {
            return Some(vec![node]);
        }
        if let Some(mut path) = node
            .children()
            .and_then(|children| ancestor_path(children, target))
        {
            path.insert(0, node);
            return Some(path);
        }
    }
    None
}

/// Whether the master `from` shows `target`, directly or through the
/// masters its instances show.
fn master_uses(doc: &PenDocument, from: &str, target: &str, seen: &mut HashSet<String>) -> bool {
    fn walk(node: &PenNode, doc: &PenDocument, target: &str, seen: &mut HashSet<String>) -> bool {
        if let PenNode::Ref(reference) = node {
            if reference.target == target {
                return true;
            }
            if seen.insert(reference.target.clone())
                && master_uses(doc, &reference.target, target, seen)
            {
                return true;
            }
        }
        node.children()
            .into_iter()
            .flatten()
            .any(|child| walk(child, doc, target, seen))
    }
    find_component_node(doc, from).is_some_and(|master| walk(&master, doc, target, seen))
}

fn collect_instances(
    nodes: &[PenNode],
    component_id: &str,
    page: (Option<&String>, Option<&String>),
    out: &mut Vec<InstanceUse>,
) {
    for node in nodes {
        if let PenNode::Ref(reference) = node {
            if reference.target == component_id {
                out.push(InstanceUse {
                    page_id: page.0.cloned(),
                    page_name: page.1.cloned(),
                    instance_id: reference.base.id.clone(),
                    override_count: reference.descendants.as_ref().map_or(0, BTreeMap::len),
                });
            }
        }
        if let Some(children) = node.children() {
            collect_instances(children, component_id, page, out);
        }
    }
}

fn collect_targets(node: &PenNode, out: &mut Vec<OverrideTarget>) {
    out.push(OverrideTarget {
        id: node.id_str().to_string(),
        name: node.base().name.clone(),
        kind: node.type_tag(),
    });
    if matches!(node, PenNode::Ref(_)) {
        return;
    }
    for child in node.children().into_iter().flatten() {
        collect_targets(child, out);
    }
}
