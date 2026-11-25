//! UIKit instantiation through a document-wide id allocator.
//!
//! By default a kit component lands as a linked instance of a master the
//! first use installs on the `Components` page (`kit_master.rs`, #365);
//! `detach` keeps the old independent copy.

use crate::id_allocator::{IdAllocError, IdAllocator, SequentialIdAllocator};
use crate::pen_node_ext::PenNodeExt;
use crate::uikit::apply_kit_overrides;
use crate::{walkers, EditorState, NodeId};
use jian_ops_schema::node::PenNode;

impl EditorState {
    /// Instantiate a UIKit component on the active page as a linked
    /// instance.
    pub fn instantiate_kit_component(
        &mut self,
        kit_id: &str,
        component_id: &str,
        doc_x: f64,
        doc_y: f64,
    ) -> Option<NodeId> {
        self.instantiate_kit_component_under_parent(
            kit_id,
            component_id,
            &NodeId::NONE,
            doc_x,
            doc_y,
            None,
            false, // detach
        )
    }

    /// Drop an independent copy of a UIKit component on the active page.
    pub fn copy_kit_component(
        &mut self,
        kit_id: &str,
        component_id: &str,
        doc_x: f64,
        doc_y: f64,
    ) -> Option<NodeId> {
        self.instantiate_kit_component_under_parent(
            kit_id,
            component_id,
            &NodeId::NONE,
            doc_x,
            doc_y,
            None,
            true, // detach
        )
    }

    /// Parent-aware UIKit instantiation using standalone ids.
    #[allow(clippy::too_many_arguments)]
    pub fn instantiate_kit_component_under_parent(
        &mut self,
        kit_id: &str,
        component_id: &str,
        parent_id: &NodeId,
        doc_x: f64,
        doc_y: f64,
        overrides_json: Option<&str>,
        detach: bool,
    ) -> Option<NodeId> {
        let mut allocator = SequentialIdAllocator::for_document(&self.doc, 1).ok()?;
        self.instantiate_kit_component_under_parent_with_allocator(
            kit_id,
            component_id,
            parent_id,
            doc_x,
            doc_y,
            overrides_json,
            detach,
            &mut allocator,
        )
        .ok()
        .flatten()
    }

    /// Parent-aware UIKit instantiation using the caller's id policy: a
    /// linked instance, or an independent copy when `detach`.
    #[allow(clippy::too_many_arguments)]
    pub fn instantiate_kit_component_under_parent_with_allocator(
        &mut self,
        kit_id: &str,
        component_id: &str,
        parent_id: &NodeId,
        doc_x: f64,
        doc_y: f64,
        overrides_json: Option<&str>,
        detach: bool,
        allocator: &mut dyn IdAllocator,
    ) -> Result<Option<NodeId>, IdAllocError> {
        let placement = crate::kit_master::KitPlacement {
            parent: parent_id,
            doc_x,
            doc_y,
            overrides_json,
        };
        if !detach && self.kit_template_is_frame(kit_id, component_id) {
            return self.insert_linked_kit_component_with_allocator(
                kit_id,
                component_id,
                placement,
                allocator,
            );
        }
        self.copy_kit_component_with_allocator(kit_id, component_id, placement, allocator)
    }

    /// The independent-copy path: the template deep-cloned with fresh ids.
    fn copy_kit_component_with_allocator(
        &mut self,
        kit_id: &str,
        component_id: &str,
        placement: crate::kit_master::KitPlacement<'_>,
        allocator: &mut dyn IdAllocator,
    ) -> Result<Option<NodeId>, IdAllocError> {
        let crate::kit_master::KitPlacement {
            parent: parent_id,
            doc_x,
            doc_y,
            overrides_json,
        } = placement;
        if parent_id.is_real() {
            match walkers::find_node(self.active_children(), parent_id) {
                Some(parent) if parent.is_container() => {}
                _ => return Ok(None),
            }
        }
        let (template, label, kit_vars) = {
            let Some(kit) = self.ui_kits.iter().find(|kit| kit.id == kit_id) else {
                return Ok(None);
            };
            let Some(component) = kit
                .components
                .iter()
                .find(|component| component.id == component_id)
            else {
                return Ok(None);
            };
            (
                component.template.clone(),
                component.name.clone(),
                kit.variables.clone(),
            )
        };
        let mut authored = template.clone();
        authored.base_mut().name = Some(label);
        let dx = doc_x - authored.base().x.unwrap_or(0.0);
        let dy = doc_y - authored.base().y.unwrap_or(0.0);
        walkers::translate_subtree(&mut authored, dx, dy);
        if !apply_kit_overrides(&mut authored, overrides_json) {
            return Ok(None);
        }

        // Allocate the complete subtree before copying variables or mutating
        // the live tree, so exhaustion is document-atomic.
        let mut taken = self.collect_node_ids();
        let mut clone = walkers::deep_clone_with_allocator(&authored, allocator, &mut taken)?;
        if let PenNode::Frame(frame) = &mut clone {
            frame.reusable = None;
        }
        let new_id = NodeId::new(clone.base().id.clone());
        let snap = self.snapshot_for_history();

        self.install_kit_variables(&template, kit_vars.as_ref());
        if parent_id.is_real() {
            let Some(parent) = walkers::find_node_mut(self.active_children_mut(), parent_id) else {
                return Ok(None);
            };
            let Some(children) = parent.children_mut_or_insert() else {
                return Ok(None);
            };
            children.push(clone);
        } else {
            self.active_children_mut().push(clone);
        }
        self.set_single_selection(new_id.clone());
        self.history_push_past(snap);
        Ok(Some(new_id))
    }

    /// Copy the kit variables `template` references into the document;
    /// names the document already defines keep their definition.
    pub(crate) fn install_kit_variables(
        &mut self,
        template: &PenNode,
        vars: Option<
            &std::collections::BTreeMap<String, jian_ops_schema::variable::VariableDefinition>,
        >,
    ) {
        let Some(vars) = vars else {
            return;
        };
        let mut refs = std::collections::BTreeSet::new();
        crate::uikit_io::collect_template_variable_refs(template, &mut refs);
        for reference in refs {
            let name = reference.strip_prefix('$').unwrap_or(&reference);
            if let Some(definition) = vars.get(name) {
                let document_vars = self.doc.variables.get_or_insert_with(Default::default);
                if !document_vars.contains_key(name) {
                    document_vars.insert(name.to_string(), definition.clone());
                }
            }
        }
    }
}
