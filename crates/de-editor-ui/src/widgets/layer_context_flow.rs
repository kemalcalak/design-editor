//! Layer / page context-menu row dispatch, split out of `press_flow.rs`
//! to honor the 800-line cap. Re-exported from `press_flow` so both
//! hosts keep their existing `press_flow::…` import paths.

use de_editor_core::copy_as::CopyAsFormat;
use de_editor_core::host_press_transitions as core_press;
use de_editor_core::path_ops::PathOperation;
use de_editor_core::ui_draft::LayerContextTarget;
use de_editor_core::{BooleanOp, EditorState};

use crate::widgets::layer_context_menu::LayerContextAction;

/// Residual host work after [`apply_layer_context_action`]. Both hosts
/// mark dirty afterwards regardless of the variant.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LayerContextStep {
    /// Fully applied by the shared dispatch.
    Done,
    /// Host runs its `apply_group()`.
    Group,
    /// Host runs its `apply_path_operation(op)` (booleans, Flatten,
    /// Outline stroke; #388).
    Path(PathOperation),
    /// Host re-fits the viewport on the active page.
    Refit,
}

/// Layer / page context-menu row dispatch.
pub fn apply_layer_context_action(
    state: &mut EditorState,
    next_node_id: &mut u64,
    action: LayerContextAction,
    target: LayerContextTarget,
    now_ms: u64,
) -> LayerContextStep {
    use LayerContextAction as A;
    use LayerContextTarget as T;
    match (action, target) {
        (A::Duplicate, T::Layer(id)) => {
            // Act on the whole multi-selection when the right-clicked
            // row is part of it; otherwise retarget to just this row.
            if !state.is_selected(&id) {
                state.set_single_selection(id);
            }
            state.commit_history();
            let _ = state.duplicate_selected(next_node_id, 10.0);
            LayerContextStep::Done
        }
        (A::CopyAsPng | A::CopyAsSvg, T::Layer(id)) => {
            // Copy the multi-selection when the row is part of it, like
            // Duplicate; the desktop renders and writes the clipboard.
            if !state.is_selected(&id) {
                state.set_single_selection(id);
            }
            state.editor_ui.canvas_view.pending_copy_as = Some(match action {
                A::CopyAsSvg => CopyAsFormat::Svg,
                _ => CopyAsFormat::Png,
            });
            LayerContextStep::Done
        }
        (A::Delete, T::Layer(id)) => {
            // Keep the multi-selection so Delete removes every selected
            // layer, not just the right-clicked one.
            if !state.is_selected(&id) {
                state.set_single_selection(id);
            }
            state.commit_history();
            let _ = state.delete_selected();
            LayerContextStep::Done
        }
        (A::GroupSelection, T::Layer(_)) => LayerContextStep::Group,
        // TS boolean rows act on the current selection and push history
        // explicitly (`layer-panel.tsx:389-407`); the host's
        // `apply_boolean_op` does both.
        (
            A::BooleanUnion | A::BooleanSubtract | A::BooleanIntersect | A::BooleanExclude,
            T::Layer(_),
        ) => {
            let op = match action {
                A::BooleanSubtract => BooleanOp::Subtract,
                A::BooleanIntersect => BooleanOp::Intersect,
                A::BooleanExclude => BooleanOp::Exclude,
                _ => BooleanOp::Union,
            };
            LayerContextStep::Path(PathOperation::Boolean(op))
        }
        (A::Flatten, T::Layer(_)) => LayerContextStep::Path(PathOperation::Flatten),
        (A::OutlineStroke, T::Layer(_)) => LayerContextStep::Path(PathOperation::OutlineStroke),
        (A::ToggleLock, T::Layer(id)) => {
            // TS toggleLock runs through mutateWithHistory
            // (document-store-node-actions.ts:176-188).
            core_press::with_doc_history(state, |s| s.toggle_node_locked(&id));
            LayerContextStep::Done
        }
        (A::ToggleVisibility, T::Layer(id)) => {
            // TS toggleVisibility runs through mutateWithHistory
            // (document-store-node-actions.ts:162-174).
            core_press::with_doc_history(state, |s| s.toggle_node_hidden(&id));
            LayerContextStep::Done
        }
        (A::CreateComponent, T::Layer(id)) => {
            let _ = state.create_component_from_node_name(&id);
            LayerContextStep::Done
        }
        (A::CreateComponentFromSimilar, T::Layer(id)) => {
            convert_similar_layers(state, id, now_ms);
            LayerContextStep::Done
        }
        (A::DetachComponent | A::DetachInstance, T::Layer(id)) => {
            // Reusable component sheds its flag; a Ref instance
            // materializes into an independent subtree (#22).
            let _ = state.detach_component(&id);
            LayerContextStep::Done
        }
        // Page CRUD pushes history in TS (document-store-pages.ts:19-121)
        // — snapshot-before-mutate, skipped when the guard rejects the op.
        (A::DuplicatePage, T::Page(idx)) => {
            if core_press::with_doc_history(state, |s| s.duplicate_page(idx).is_some()) {
                LayerContextStep::Refit
            } else {
                LayerContextStep::Done
            }
        }
        (A::MovePageUp, T::Page(idx)) => {
            core_press::with_doc_history(state, |s| s.move_page_up(idx));
            LayerContextStep::Done
        }
        (A::MovePageDown, T::Page(idx)) => {
            core_press::with_doc_history(state, |s| s.move_page_down(idx));
            LayerContextStep::Done
        }
        (A::DeletePage, T::Page(idx)) => {
            let deleting_active = idx == state.ui.active_page_index;
            if core_press::with_doc_history(state, |s| s.remove_page(idx)) && deleting_active {
                LayerContextStep::Refit
            } else {
                LayerContextStep::Done
            }
        }
        (A::RenamePage, T::Page(idx)) => {
            if state.start_rename_page(idx) {
                if let Some(rename) = state.ui.layer_rename.as_mut() {
                    rename.input.touch(now_ms);
                }
            }
            LayerContextStep::Done
        }
        (A::RenameLayersAuto, T::Layer(id)) => {
            // Act on the whole multi-selection when the right-clicked row
            // is part of it; otherwise retarget to just this row.
            if !state.is_selected(&id) {
                state.set_single_selection(id);
            }
            state.open_rename_layers();
            LayerContextStep::Done
        }
        (A::RenameLayer, T::Layer(id)) => {
            if state.start_rename_layer(id) {
                if let Some(rename) = state.ui.layer_rename.as_mut() {
                    rename.input.touch(now_ms);
                }
            }
            LayerContextStep::Done
        }
        // Mismatched action/target — no-op.
        _ => LayerContextStep::Done,
    }
}

/// "Component from copies" (#369): the right-clicked frame is the master
/// source; copies whose structure differs stay as they are. The outcome is
/// said in a toast, since nothing else on screen tells the user which
/// copies were left out or that none could be converted.
fn convert_similar_layers(state: &mut EditorState, id: de_editor_core::NodeId, now_ms: u64) {
    use de_editor_core::editor_toast::EditorToastLevel;
    use de_editor_core::repeated_structures::ConvertRequest;
    let node_ids = state.similar_layer_ids(&id);
    if node_ids.len() < 2 {
        return;
    }
    let request = ConvertRequest {
        master_from: Some(id),
        node_ids,
        ..Default::default()
    };
    let (key, args, level) = match state.convert_to_instances(&request) {
        Some(plan) if plan.skipped.is_empty() => (
            "convertInstances.done",
            vec![("converted".to_string(), plan.converted.len().to_string())],
            EditorToastLevel::Info,
        ),
        Some(plan) => (
            "convertInstances.partial",
            vec![
                ("converted".to_string(), plan.converted.len().to_string()),
                ("skipped".to_string(), plan.skipped.len().to_string()),
            ],
            EditorToastLevel::Warn,
        ),
        None => (
            "convertInstances.failed",
            Vec::new(),
            EditorToastLevel::Warn,
        ),
    };
    state.editor_ui.show_toast(key, args, level, now_ms);
}

#[cfg(test)]
#[path = "layer_context_copy_tests.rs"]
mod copy_tests;
