//! Component-instance commands (#365) and UIKit inserts behind
//! [`EditorState::apply`]. Each runs on the page it names (or, for detach
//! and swap, the page holding the node); the caller's active page and
//! selection come back afterwards, also in the undo step.

use super::*;
use crate::instance_command::InstanceCommand;

impl EditorState {
    pub(super) fn apply_instance_command(
        &mut self,
        command: InstanceCommand,
        allocator: &mut dyn IdAllocator,
    ) -> Result<bool, IdAllocError> {
        match command {
            InstanceCommand::Insert {
                component_id,
                parent,
                x,
                y,
                descendants_json,
                page_id,
            } => {
                let Some(page) = command_page_index(self, page_id.as_deref()) else {
                    return Ok(false);
                };
                self.on_page(page, |state| {
                    state
                        .insert_component_instance_with_allocator(
                            &component_id,
                            &parent,
                            (x, y),
                            descendants_json.as_deref(),
                            allocator,
                        )
                        .map(|id| id.is_some())
                })
            }
            InstanceCommand::Copy { component_id } => Ok(self
                .instantiate_component_with_allocator(&component_id, allocator)?
                .is_some()),
            InstanceCommand::Detach { node_id, page_id } => {
                let Some(page) = self.node_page_index(&node_id, page_id.as_deref()) else {
                    return Ok(false);
                };
                self.on_page(page, |state| {
                    if state.instance_view().instance_node(&node_id).is_err() {
                        return Ok(false);
                    }
                    Ok(state
                        .detach_component_with_allocator(&node_id, allocator)?
                        .is_some())
                })
            }
            InstanceCommand::Swap {
                node_id,
                component_id,
                page_id,
            } => {
                let Some(page) = self.node_page_index(&node_id, page_id.as_deref()) else {
                    return Ok(false);
                };
                self.on_page(page, |state| {
                    Ok(state
                        .instance_view()
                        .check_instance_swap(&node_id, &component_id)
                        .is_ok()
                        && state.set_instance_component(&node_id, &component_id))
                })
            }
        }
    }

    pub(super) fn apply_kit_command(
        &mut self,
        cmd: EditorCommand,
        allocator: &mut dyn IdAllocator,
    ) -> Result<bool, IdAllocError> {
        let EditorCommand::InstantiateKitComponent {
            kit_id,
            component_id,
            doc_x,
            doc_y,
            target_parent,
            page_id,
            overrides_json,
            detach,
        } = cmd
        else {
            return Ok(false);
        };
        let Some(page) = command_page_index(self, page_id.as_deref()) else {
            return Ok(false);
        };
        self.on_page(page, |state| {
            state
                .instantiate_kit_component_under_parent_with_allocator(
                    &kit_id,
                    &component_id,
                    &target_parent,
                    doc_x.unwrap_or(0.0),
                    doc_y.unwrap_or(0.0),
                    overrides_json.as_deref(),
                    detach,
                    allocator,
                )
                .map(|id| id.is_some())
        })
    }

    /// Run `apply` with page `page` active. On another page than the
    /// active one, the active page and selection are put back afterwards,
    /// and into the undo step `apply` pushed, so undo lands where the
    /// caller was.
    fn on_page(
        &mut self,
        page: usize,
        apply: impl FnOnce(&mut Self) -> Result<bool, IdAllocError>,
    ) -> Result<bool, IdAllocError> {
        let original_page = self.ui.active_page_index;
        if page == original_page {
            return apply(self);
        }
        let original_selection = self.selection.clone();
        let pushes_before = self.history_push_count;
        self.ui.active_page_index = page;
        let result = apply(self);
        self.ui.active_page_index = original_page;
        self.selection = original_selection.clone();
        if self.history_push_count != pushes_before {
            if let Some(snapshot) = self.history.past.back_mut() {
                snapshot.active_page_index = original_page;
                snapshot.selection = original_selection;
            }
        }
        result
    }
}
