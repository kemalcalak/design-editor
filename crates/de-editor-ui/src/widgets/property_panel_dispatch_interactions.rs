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
            | A::SetInteractionScrollTo { .. }
            | A::SetInteractionOpenUrl(_)
            | A::MoveInteraction { .. }
            | A::SetInteractionTransition { .. }
            | A::RemoveInteraction(_)
            | A::GoToInteractionTarget(_)
            | A::PickOverlayTarget { .. }
            | A::SetInteractionOverlay { .. }
            | A::SetInteractionCloseOverlay(_)
            | A::SetOverlayOption { .. }
            | A::EditScreenMarker(_)
            | A::AddAnimation
            | A::RemoveAnimation(_)
            | A::SetAnimationTrigger { .. }
            | A::SetAnimationProperty { .. }
            | A::AddTransition
            | A::RemoveTransition
            | A::SetTransitionEasing(_)
    )
}

/// Whether `action` writes the document (the host's history choke point).
pub(crate) fn interaction_updates_document(action: &PropertyPanelAction) -> bool {
    is_interaction_action(action)
        && !matches!(
            action,
            PropertyPanelAction::ToggleInteractionMenu(_)
                | PropertyPanelAction::PickInteractionTrigger(_)
                | PropertyPanelAction::GoToInteractionTarget(_)
                | PropertyPanelAction::PickOverlayTarget { .. }
        )
}

/// The node the selection's `trigger` leads to: a navigate's screen on
/// this page, or a `scroll_to` target (#371).
fn interaction_target(
    state: &EditorState,
    trigger: de_editor_core::interaction_edit::InteractionTrigger,
) -> Option<de_editor_core::NodeId> {
    use de_editor_core::PenNodeExt;
    let node = state.selected_node()?;
    let [action] = trigger.actions(node.events()?)?.as_slice() else {
        return None;
    };
    let (key, body) = action.single()?;
    let id = if de_editor_core::interaction_edit::NavigateVerb::from_key(key).is_some() {
        let route = de_editor_core::interaction_edit::unquote_literal(body.as_str()?);
        de_editor_core::interaction_graph::resolve_screen_target(state.active_children(), &route)
            .filter(|target| !target.needs_marker)?
            .node_id
    } else if key == "scroll_to" {
        body.get("target")?.as_str()?.to_owned()
    } else if let Some(target) =
        de_editor_core::overlay_action::OverlayAction::parse(action)
            .and_then(|overlay| overlay.target().map(str::to_owned))
    {
        // An overlay frame on this page (#372).
        let id = de_editor_core::NodeId::new_opt(&target)?;
        de_editor_core::walkers::find_node(state.active_children(), &id)?;
        target
    } else {
        return None;
    };
    de_editor_core::NodeId::new_opt(id)
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
        A::SetInteractionScrollTo { trigger, target } => {
            let edit = InteractionActionEdit::ScrollTo {
                target: target.clone(),
            };
            let _ = state.set_selected_interaction_action(*trigger, &edit);
        }
        A::SetInteractionOpenUrl(trigger) => {
            // The address is typed into the URL row that appears below.
            let edit = InteractionActionEdit::OpenUrl { url: String::new() };
            let _ = state.set_selected_interaction_action(*trigger, &edit);
        }
        A::MoveInteraction { from, to } => {
            let _ = state.move_selected_interaction(*from, *to);
        }
        A::SetInteractionTransition { trigger, verb } => {
            let _ = state.set_selected_interaction_verb(*trigger, *verb);
        }
        A::RemoveInteraction(trigger) => {
            let _ = state.remove_selected_interaction(*trigger);
        }
        A::PickOverlayTarget { trigger, swap } => {
            state.editor_ui.interaction_menu_target = InteractionMenuTarget::OverlayTarget {
                trigger: *trigger,
                swap: *swap,
            };
            state.editor_ui.interaction_menu_hover = None;
            return;
        }
        A::SetInteractionOverlay {
            trigger,
            target,
            swap,
        } => {
            let edit = if *swap {
                InteractionActionEdit::SwapOverlay {
                    target: target.clone(),
                }
            } else {
                InteractionActionEdit::OpenOverlay {
                    target: target.clone(),
                }
            };
            let _ = state.set_selected_interaction_action(*trigger, &edit);
        }
        A::SetInteractionCloseOverlay(trigger) => {
            let _ = state
                .set_selected_interaction_action(*trigger, &InteractionActionEdit::CloseOverlay);
        }
        A::SetOverlayOption { trigger, edit } => {
            let _ = state.set_selected_overlay_option(*trigger, edit);
            return;
        }
        A::GoToInteractionTarget(trigger) => {
            if let Some(target) = interaction_target(state, *trigger) {
                state.set_single_selection(target.clone());
                // The host frames it once the action's scope has closed.
                state.editor_ui.prototype.pending_frame = Some(target);
            }
        }
        A::EditScreenMarker(edit) => {
            let _ = state.edit_selected_screen_marker(*edit);
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
        A::AddTransition => {
            let _ = state.add_selected_transition();
        }
        A::RemoveTransition => {
            let _ = state.remove_selected_transition();
        }
        A::SetTransitionEasing(easing) => {
            let _ = state.set_selected_transition_easing(*easing);
        }
        _ => return,
    }
    state.editor_ui.close_interaction_menu();
}
