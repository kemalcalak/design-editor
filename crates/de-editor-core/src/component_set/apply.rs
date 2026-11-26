//! Applying set writes to the editor: one undo step each, with the
//! component registry following moved, new, renamed and removed masters.

use jian_ops_schema::node::PenNode;

use super::edit::apply_to_document;
use super::pick::variant_masters;
use super::{ComponentSetCommand, VariantError, VariantReport};
use crate::id_allocator::{IdAllocError, IdAllocator, SequentialIdAllocator};
use crate::pen_node_ext::PenNodeExt;
use crate::{EditorState, NodeId};

/// The toast a variant write raises when it could not keep everything.
pub const VARIANT_TOAST_KEY: &str = "variants.reportToast";

impl EditorState {
    /// Apply a component-set write as one undo step. The outer `Err` is
    /// id exhaustion; the inner one a refused write (nothing changed).
    pub fn apply_component_set_with_allocator(
        &mut self,
        command: &ComponentSetCommand,
        allocator: &mut dyn IdAllocator,
    ) -> Result<Result<VariantReport, VariantError>, IdAllocError> {
        if let Err(error) = command.check(&self.doc) {
            return Ok(Err(error));
        }
        let snapshot = self.snapshot_for_history();
        let before: Vec<NodeId> = command
            .set_id()
            .and_then(|id| super::tree::find(&self.doc, id))
            .map(|set| {
                variant_masters(set)
                    .iter()
                    .map(|master| NodeId::new(master.id_str()))
                    .collect()
            })
            .unwrap_or_default();
        let report = match apply_to_document(&mut self.doc, command, allocator) {
            Ok(Ok(report)) => report,
            Ok(Err(error)) => return Ok(Err(error)),
            Err(error) => {
                self.restore(snapshot);
                return Err(error);
            }
        };
        self.refresh_set_registry(report.set_id.as_deref(), &before);
        match command {
            ComponentSetCommand::Create(_) => {
                if let Some(set_id) = &report.set_id {
                    self.set_single_selection(NodeId::new(set_id.as_str()));
                }
            }
            ComponentSetCommand::AddVariant { .. } => {
                if let Some(id) = report.created.first() {
                    self.set_single_selection(NodeId::new(id.as_str()));
                }
            }
            _ => {}
        }
        self.history_push_past(snapshot);
        Ok(Ok(report))
    }

    /// [`Self::apply_component_set_with_allocator`] with a fresh
    /// sequential allocator, for the panel.
    pub fn apply_component_set(
        &mut self,
        command: &ComponentSetCommand,
    ) -> Result<VariantReport, VariantError> {
        let Ok(mut allocator) = SequentialIdAllocator::for_document(&self.doc, 1) else {
            return Err(VariantError::IdsExhausted);
        };
        let result = match self.apply_component_set_with_allocator(command, &mut allocator) {
            Ok(result) => result,
            Err(_) => Err(VariantError::IdsExhausted),
        };
        if let Ok(report) = &result {
            if !report.is_quiet() {
                self.editor_ui.pending_variant_report = Some(report.clone());
            }
        }
        result
    }

    /// Register the set's masters where they now are, under their current
    /// names; drop the ones that left.
    fn refresh_set_registry(&mut self, set_id: Option<&str>, before: &[NodeId]) {
        let Some(set_id) = set_id else {
            return;
        };
        let masters: Vec<(NodeId, String)> = super::tree::find(&self.doc, set_id)
            .map(|set| {
                variant_masters(set)
                    .iter()
                    .filter_map(|master| {
                        crate::component_registration::reusable_component_name(master)
                            .map(|name| (NodeId::new(master.id_str()), name))
                    })
                    .collect()
            })
            .unwrap_or_default();
        for id in before {
            if !masters.iter().any(|(master, _)| master == id) {
                self.components.remove(id);
            }
        }
        self.components
            .register_document_components(&self.doc, masters);
    }

    /// Raise the last panel variant write's report as a Warn toast (the
    /// host's clock), then forget it.
    pub fn raise_pending_variant_toast(&mut self, now_ms: u64) -> bool {
        let Some(report) = self.editor_ui.pending_variant_report.take() else {
            return false;
        };
        let args = vec![
            (
                "dropped".to_string(),
                report.dropped_overrides.len().to_string(),
            ),
            ("nearest".to_string(), report.fallbacks.len().to_string()),
            (
                "moved".to_string(),
                report.moved_to_default.len().to_string(),
            ),
        ];
        self.editor_ui.show_toast(
            VARIANT_TOAST_KEY,
            args,
            crate::editor_toast::EditorToastLevel::Warn,
            now_ms,
        );
        true
    }

    /// The set holding master `master_id`: the nearest set among its
    /// ancestors on the registry's stored path, so the panel never walks
    /// the document; a walk only for a master the registry does not know
    /// or whose path went stale.
    pub(crate) fn set_holding(&self, master_id: &str) -> Option<&PenNode> {
        let id = NodeId::new(master_id);
        let backing = self.components.document_backing();
        if let Some(location) = backing.get(&id) {
            let roots = match location.page_index {
                Some(page) => self
                    .doc
                    .pages
                    .as_ref()
                    .and_then(|p| p.get(page))
                    .map(|p| &p.children),
                None => Some(&self.doc.children),
            };
            let mut nodes = roots.map(Vec::as_slice).unwrap_or_default();
            let mut nearest = None;
            let mut reached = None;
            for index in location.child_path.iter() {
                let Some(node) = nodes.get(*index) else {
                    break;
                };
                reached = Some(node);
                if node.id_str() != master_id && super::component_set_of(node).is_some() {
                    nearest = Some(node);
                }
                nodes = node.children().map(Vec::as_slice).unwrap_or_default();
            }
            if reached.is_some_and(|node| node.id_str() == master_id) {
                return nearest;
            }
        }
        super::set_containing(&self.doc, master_id)
    }

    /// The set and variant values of the selected instance's component,
    /// when it is a variant.
    pub fn selected_variant_instance(&self) -> Option<(String, super::InstanceTarget)> {
        if self.selection_count() != 1 {
            return None;
        }
        let PenNode::Ref(reference) = self.selected_node()? else {
            return None;
        };
        let target = super::resolve::resolve_target_in(
            &self.doc,
            &reference.target,
            reference.variant.as_ref(),
            &|id| self.set_holding(id),
        )
        .ok()
        .flatten()?;
        Some((reference.base.id.clone(), target))
    }
}
