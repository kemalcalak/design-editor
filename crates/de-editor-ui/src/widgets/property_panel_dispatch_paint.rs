//! Shared dispatch for the effect-add menu and the paint options #90
//! exposed (stroke alignment, flip, shader presets), the stroke cap /
//! join / dash rows (#89), the mesh lattice steppers (#142) and the
//! shader uniform sliders and Code button (#164). Split out of
//! `property_panel_dispatch.rs` (800-line cap); both hosts reach it
//! through `apply_property_action`.
//!
//! History: `AddEffect` snapshots inside `add_effect_to_selected`; a
//! shader slider snapshots on the first step of a drag (like the effect
//! amount slider) and writes in place after; opening the source editor
//! writes nothing; the others are in `updates_document`, so the host
//! snapshots around them and drops the entry when nothing changed.

use de_editor_core::EditorState;

use crate::widgets::property_panel_snapshot::EffectKind;
use crate::widgets::PropertyPanelAction;

/// Apply one of the actions above; any other action is ignored.
pub(crate) fn apply_paint_option_action(state: &mut EditorState, action: &PropertyPanelAction) {
    use PropertyPanelAction as A;
    match action {
        A::AddEffect(kind) => {
            match kind {
                EffectKind::Shadow => state.add_drop_shadow_to_selected(),
                EffectKind::InnerShadow => state.add_inner_shadow_to_selected(),
                EffectKind::LayerBlur => state.add_layer_blur_to_selected(),
                EffectKind::BackgroundBlur => state.add_background_blur_to_selected(),
                EffectKind::Shader => state.add_shader_effect_to_selected(),
            };
            state.editor_ui.close_effect_add_picker();
        }
        A::SetStrokeAlign(align) => {
            let _ = state.set_selected_stroke_align(align.clone());
        }
        A::SetStrokeStyle(edit) => {
            let _ = state.set_selected_stroke_style(edit);
        }
        A::FlipSelection(axis) => {
            let _ = state.flip_selected(*axis);
        }
        A::SetShaderPreset(preset) => {
            let _ = state.set_selected_shader_preset(*preset);
        }
        A::RestoreCustomShader => {
            let _ = state.restore_selected_custom_shader();
        }
        A::EditMeshLattice(edit) => {
            let _ = state.set_selected_mesh_lattice(*edit);
        }
        A::DragShaderUniform {
            target,
            index,
            value,
        } => {
            let drag = Some((*target, *index));
            if state.editor_ui.shader_editing.uniform_drag != drag {
                state.commit_history();
                state.editor_ui.shader_editing.uniform_drag = drag;
            }
            let _ = state.set_selected_shader_uniform_number(*target, *index, 0, *value);
        }
        A::ToggleShaderUniform { target, index, on } => {
            let value = if *on { 1.0 } else { 0.0 };
            // History only for a write that can land (an editable row).
            let editable = state.is_editable(&state.selection.anchor.clone());
            if editable && state.selected_shader_uniform(*target, *index).is_some() {
                state.commit_history();
                let _ = state.set_selected_shader_uniform_number(*target, *index, 0, value);
            }
        }
        A::OpenShaderSource(target) => {
            let _ = state.open_shader_source_editor(*target);
        }
        _ => {}
    }
}
