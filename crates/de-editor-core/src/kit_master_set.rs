//! Kit components that are variants of a kit set (#368): installing the
//! whole set on first use, and picking another variant through `K()`'s
//! `variant` override.

use std::collections::HashMap;

use jian_ops_schema::component_set::VariantValues;
use jian_ops_schema::node::PenNode;
use serde_json::{Map, Value};

use crate::id_allocator::{IdAllocError, IdAllocator};
use crate::kit_master::{kit_master_key, zip_ids, KitMaster, KitTemplate};
use crate::kit_sets::{build_kit_set, subtree_ids, KitSetSpec};
use crate::pen_node_ext::PenNodeExt;
use crate::{walkers, ComponentLibrary, EditorState, NodeId};

impl EditorState {
    /// Put the kit set holding `component_id` on the `Components` page
    /// and return that component's master. Ids stay verbatim unless one
    /// is taken; then the whole set gets fresh ids and every member's
    /// master is recorded in `kit_masters`.
    pub(crate) fn install_kit_set(
        &mut self,
        spec: &KitSetSpec,
        component_id: &str,
        kit: &KitTemplate,
        allocator: &mut dyn IdAllocator,
    ) -> Result<Option<KitMaster>, IdAllocError> {
        let Some(ui_kit) = self.ui_kits.iter().find(|k| k.id == spec.kit_id) else {
            return Ok(None);
        };
        let Some(mut set) = build_kit_set(spec, ui_kit) else {
            return Ok(None);
        };
        let templates: Vec<(String, PenNode)> = spec
            .members
            .iter()
            .filter_map(|(_, id)| {
                let component = ui_kit.components.iter().find(|c| c.id == *id)?;
                Some((id.to_string(), component.template.clone()))
            })
            .collect();
        // Members the document already holds as single masters (from before
        // the set) keep resolving to them; only the others join the set.
        let kept: Vec<String> = templates
            .iter()
            .filter(|(id, template)| {
                self.find_kit_master(&kit_master_key(spec.kit_id, id), template)
                    .is_some()
            })
            .map(|(id, _)| id.clone())
            .collect();
        let (x, y) = self.next_master_slot();
        set.base_mut().x = Some(x);
        set.base_mut().y = Some(y);
        let mut taken = self.collect_node_ids();
        let mut ids = Vec::new();
        subtree_ids(&set, &mut ids);
        let fresh_ids = ids
            .iter()
            .any(|id| taken.contains(&NodeId::new(id.as_str())));
        if fresh_ids {
            set = walkers::deep_clone_with_allocator(&set, allocator, &mut taken)?;
        }
        let had_pages = self
            .doc
            .pages
            .as_ref()
            .is_some_and(|pages| !pages.is_empty());
        if self.append_components_page_masters_with_allocator(vec![set.clone()], allocator)? == 0 {
            return Ok(None);
        }
        let masters: Vec<(NodeId, String)> = crate::component_set::variant_masters(&set)
            .iter()
            .map(|master| {
                let name = master.base().name.clone().unwrap_or_default();
                (NodeId::new(master.id_str()), name)
            })
            .collect();
        if had_pages {
            self.components
                .register_document_components(&self.doc, masters);
        } else {
            self.components = ComponentLibrary::from_document(&self.doc);
        }
        // Each member's `md` master stands for its kit component.
        let md = |template_id: &str| -> Option<PenNode> {
            let (_, values) = crate::kit_sets::kit_set_of(spec.kit_id, template_id)?;
            let pick = crate::component_set::pick_variant(&set, &values)?;
            Some(pick.master.clone())
        };
        let mut wanted = None;
        for (template_id, template) in &templates {
            self.install_kit_variables(template, kit.variables.as_ref());
            let Some(node) = md(template_id) else {
                continue;
            };
            if fresh_ids && !kept.contains(template_id) {
                self.editor_ui.kit_masters.insert(
                    kit_master_key(spec.kit_id, template_id),
                    node.id_str().to_string(),
                );
            }
            if template_id == component_id {
                let mut ids = HashMap::new();
                if fresh_ids {
                    zip_ids(template, &node, &mut ids);
                }
                wanted = Some(KitMaster {
                    node,
                    ids,
                    fresh_ids,
                });
            }
        }
        Ok(wanted)
    }

    /// Split a `variant` override off `overrides`: the master of the
    /// variant it asks for (when `master` is in a set and it differs),
    /// with the kit template's ids mapped onto it.
    pub(crate) fn kit_variant(
        &self,
        master: &KitMaster,
        template: &PenNode,
        mut overrides: Map<String, Value>,
    ) -> (Map<String, Value>, Option<KitMaster>) {
        let Some(requested) = overrides
            .remove("variant")
            .and_then(|raw| serde_json::from_value::<VariantValues>(raw).ok())
        else {
            return (overrides, None);
        };
        let target =
            crate::component_set::resolve_target(&self.doc, master.node.id_str(), Some(&requested))
                .ok()
                .flatten();
        let Some(target) = target.filter(|t| t.master_id != master.node.id_str()) else {
            return (overrides, None);
        };
        let Some(node) = crate::component_set::find_node(&self.doc, &target.master_id).cloned()
        else {
            return (overrides, None);
        };
        let mut ids = HashMap::new();
        zip_ids(template, &node, &mut ids);
        (
            overrides,
            Some(KitMaster {
                node,
                ids,
                fresh_ids: true,
            }),
        )
    }

    /// The full `variant` an instance of `master_id` records, when the
    /// master is a variant.
    pub(crate) fn kit_variant_values(&self, master_id: &NodeId) -> Option<VariantValues> {
        crate::component_set::resolve_target(
            &self.doc,
            master_id.as_str(),
            Some(&VariantValues::new()),
        )
        .ok()
        .flatten()
        .map(|target| target.values)
    }
}
