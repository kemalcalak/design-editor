//! Apply a conversion to the editor state (one undo step), and the queries
//! the layer context menu asks.

use std::cell::RefCell;

use jian_ops_schema::node::PenNode;

use super::{plan_conversion, ConversionPlan, ConvertError, ConvertRequest, MasterPlacement};
use crate::id_allocator::{IdAllocError, IdAllocator, SequentialIdAllocator};
use crate::pen_node_ext::PenNodeExt;
use crate::{walkers, ComponentLibrary, EditorState, NodeId};

/// Swap the node `id` for `replacement` at its slot, anywhere under
/// `nodes`. True when it was found.
fn replace_in(nodes: &mut [PenNode], id: &str, replacement: &mut Option<PenNode>) -> bool {
    for node in nodes.iter_mut() {
        if node.id_str() == id {
            if let Some(replacement) = replacement.take() {
                *node = replacement;
                return true;
            }
            return false;
        }
        // Instances hold no page-tree nodes.
        if matches!(node, PenNode::Ref(_)) {
            continue;
        }
        if let Some(children) = node.children_mut() {
            if replace_in(children, id, replacement) {
                return true;
            }
        }
    }
    false
}

fn find_mut<'a>(state: &'a mut EditorState, id: &NodeId) -> Option<&'a mut PenNode> {
    let doc = &mut state.doc;
    if let Some(pages) = doc.pages.as_mut() {
        for page in pages.iter_mut() {
            if let Some(node) = walkers::find_node_mut(&mut page.children, id) {
                return Some(node);
            }
        }
    }
    walkers::find_node_mut(&mut doc.children, id)
}

impl EditorState {
    /// Plan `request` against the current document, read only.
    pub fn plan_instance_conversion(
        &self,
        request: &ConvertRequest,
    ) -> Result<ConversionPlan, ConvertError> {
        let mut allocator = SequentialIdAllocator::for_document(&self.doc, 1)?;
        plan_conversion(&self.doc, &self.components, request, &mut allocator)
    }

    /// Convert with a fresh allocator; `None` when the plan is refused.
    pub fn convert_to_instances(&mut self, request: &ConvertRequest) -> Option<ConversionPlan> {
        let mut allocator = SequentialIdAllocator::for_document(&self.doc, 1).ok()?;
        self.convert_to_instances_with_allocator(request, &mut allocator)
            .ok()
            .flatten()
    }

    /// Plan, then replace every converted member with its instance and
    /// install the master, as one undo step. `Ok(None)` when the plan is
    /// refused; nothing changes then.
    pub fn convert_to_instances_with_allocator(
        &mut self,
        request: &ConvertRequest,
        allocator: &mut dyn IdAllocator,
    ) -> Result<Option<ConversionPlan>, IdAllocError> {
        let plan = match plan_conversion(&self.doc, &self.components, request, allocator) {
            Ok(plan) => plan,
            Err(ConvertError::IdAllocation(error)) => return Err(error),
            Err(_) => return Ok(None),
        };
        let snap = self.snapshot_for_history();
        for member in &plan.converted {
            let mut replacement = Some(member.instance.clone());
            let replaced =
                match self.doc.pages.as_mut() {
                    Some(pages) => pages.iter_mut().any(|page| {
                        replace_in(&mut page.children, member.id.as_str(), &mut replacement)
                    }),
                    None => false,
                } || replace_in(&mut self.doc.children, member.id.as_str(), &mut replacement);
            if !replaced {
                self.restore(snap);
                return Ok(None);
            }
        }
        let installed = match plan.placement {
            MasterPlacement::InPlace => match find_mut(self, &plan.master_id) {
                Some(PenNode::Frame(frame)) => {
                    frame.reusable = Some(true);
                    self.components.register_document_component(
                        &self.doc,
                        plan.master_id.clone(),
                        plan.master_name.clone(),
                    )
                }
                _ => false,
            },
            MasterPlacement::ComponentsPage => {
                let had_pages = self
                    .doc
                    .pages
                    .as_ref()
                    .is_some_and(|pages| !pages.is_empty());
                match self.append_components_page_masters_with_allocator(
                    vec![plan.master.clone()],
                    allocator,
                ) {
                    Ok(1) if had_pages => self.components.register_document_component(
                        &self.doc,
                        plan.master_id.clone(),
                        plan.master_name.clone(),
                    ),
                    Ok(1) => {
                        // The design moved into page 0: every stored
                        // location moved with it.
                        self.components = ComponentLibrary::from_document(&self.doc);
                        self.components.find_by_id(&plan.master_id).is_some()
                    }
                    Ok(_) => false,
                    Err(error) => {
                        self.restore(snap);
                        return Err(error);
                    }
                }
            }
        };
        if !installed {
            self.restore(snap);
            return Ok(None);
        }
        // Select the instances on the active page.
        let active = self.active_children().to_vec();
        let mut selection = crate::selection::SelectionState::empty();
        for member in &plan.converted {
            if walkers::find_node(&active, &member.id).is_some() {
                selection.set.push(member.id.clone());
                selection.anchor = member.id.clone();
            }
        }
        if !selection.is_empty() {
            self.selection = selection;
        }
        self.history_push_past(snap);
        Ok(Some(plan))
    }

    /// The frames "Create component from similar layers" converts when the
    /// layer `id` is right-clicked: the selection when `id` is part of a
    /// multi-selection that shares one structure, else every frame on the
    /// active page with the structure of `id`; `id` first (it becomes the
    /// master). Empty when there is nothing to convert.
    ///
    /// The context menu is rebuilt on every paint and hover while it is
    /// open, and the answer needs a scan of the page, so the last answer is
    /// kept per thread and reused while the key matches: the document (its
    /// generation, which a load or replace moves, and its revision, which
    /// every edit moves), the active page, the clicked layer and the
    /// selection.
    pub fn similar_layer_ids(&self, id: &NodeId) -> Vec<NodeId> {
        type Key = (u64, u64, usize, String, Vec<NodeId>, usize);
        thread_local! {
            static CACHE: RefCell<Option<(Key, Vec<NodeId>)>> = const { RefCell::new(None) };
        }
        let page = self
            .doc
            .pages
            .as_ref()
            .filter(|pages| !pages.is_empty())
            .map(|pages| self.ui.active_page_index.min(pages.len() - 1));
        let key: Key = (
            self.document_generation,
            self.revision,
            page.unwrap_or(usize::MAX),
            id.as_str().to_string(),
            self.selection.set.clone(),
            self.active_children().len(),
        );
        if let Some(hit) = CACHE.with(|cache| {
            cache
                .borrow()
                .as_ref()
                .filter(|(cached, _)| *cached == key)
                .map(|(_, ids)| ids.clone())
        }) {
            return hit;
        }
        let selection = &self.selection.set;
        let mut ids = if selection.len() >= 2 && selection.contains(id) {
            if super::scan::share_structure(&self.doc, &self.components, page, selection) {
                selection.clone()
            } else {
                Vec::new()
            }
        } else {
            super::similar_layers(&self.doc, &self.components, page, id)
        };
        if let Some(at) = ids.iter().position(|candidate| candidate == id) {
            let first = ids.remove(at);
            ids.insert(0, first);
        }
        CACHE.with(|cache| *cache.borrow_mut() = Some((key, ids.clone())));
        ids
    }
}
