//! Shared dispatch for the Interactions and Motion sections (#154).
//! Split out of `property_panel_dispatch.rs` (800-line cap); both hosts
//! reach it through `apply_property_action`.
//!
//! History: every write here is listed in `updates_document`, so the
//! host snapshots around it and drops the entry when nothing changed.
//! The writes themselves go through `EditorCommand::PatchNodeData`
//! (`de_editor_core::interaction_edit` / `motion_edit`).

use de_editor_core::interaction_edit::{InteractionActionEdit, InteractionMenuTarget};
use de_editor_core::EditorState;

use crate::widgets::PropertyPanelAction;

/// Whether `action` belongs to this module.
pub(crate) fn is_interaction_action(action: &PropertyPanelAction) -> bool {
    use PropertyPanelAction as A;
    matches!(
        action,
        A::ToggleInteractionMenu(_)
            | A::PickInteractionTrigger(_)
            | A::SetInteractionNavigate { .. }
            | A::SetInteractionPop(_)
            | A::SetInteractionTransition { .. }
            | A::RemoveInteraction(_)
            | A::AddAnimation
            | A::RemoveAnimation(_)
            | A::SetAnimationTrigger { .. }
            | A::SetAnimationProperty { .. }
    )
}

/// Whether `action` writes the document (the host's history choke point).
pub(crate) fn interaction_updates_document(action: &PropertyPanelAction) -> bool {
    is_interaction_action(action)
        && !matches!(
            action,
            PropertyPanelAction::ToggleInteractionMenu(_)
                | PropertyPanelAction::PickInteractionTrigger(_)
        )
}

/// Apply one of the actions above; any other action is ignored.
pub(crate) fn apply_interaction_action(state: &mut EditorState, action: &PropertyPanelAction) {
    use PropertyPanelAction as A;
    match action {
        A::ToggleInteractionMenu(target) => {
            let ui = &mut state.editor_ui;
            ui.toggle_interaction_menu(*target);
            ui.close_fill_type_picker();
            ui.image_fill_popover_open = false;
            ui.close_font_picker();
            ui.font_weight_picker_open = false;
            ui.export_scale_picker_open = false;
            ui.export_format_picker_open = false;
            ui.close_color_variable_picker();
            return;
        }
        A::PickInteractionTrigger(trigger) => {
            // Keep the popover open on the picked trigger's actions; it
            // anchors to the header "+" until the trigger has a row.
            state.editor_ui.interaction_menu_target = InteractionMenuTarget::Trigger(*trigger);
            state.editor_ui.interaction_menu_hover = None;
            return;
        }
        A::SetInteractionNavigate { trigger, path } => {
            let edit = InteractionActionEdit::Navigate { path: path.clone() };
            let _ = state.set_selected_interaction_action(*trigger, &edit);
        }
        A::SetInteractionPop(trigger) => {
            let _ = state.set_selected_interaction_action(*trigger, &InteractionActionEdit::Back);
        }
        A::SetInteractionTransition { trigger, verb } => {
            let _ = state.set_selected_interaction_verb(*trigger, *verb);
        }
        A::RemoveInteraction(trigger) => {
            let _ = state.remove_selected_interaction(*trigger);
        }
        A::AddAnimation => {
            let _ = state.add_selected_animation();
        }
        A::RemoveAnimation(index) => {
            let _ = state.remove_selected_animation(*index);
        }
        A::SetAnimationTrigger { index, trigger } => {
            let _ = state.set_selected_animation_trigger(*index, *trigger);
        }
        A::SetAnimationProperty { index, property } => {
            let _ = state.set_selected_animation_property(*index, *property);
        }
        _ => return,
    }
    state.editor_ui.close_interaction_menu();
}
