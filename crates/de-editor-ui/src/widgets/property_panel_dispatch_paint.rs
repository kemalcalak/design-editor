//! Shared dispatch for the effect-add menu and the paint options #90
//! exposed (stroke alignment, flip, shader presets) and the stroke cap /
//! join / dash rows (#89). Split out of
//! `property_panel_dispatch.rs` (800-line cap); both hosts reach it
//! through `apply_property_action`.
//!
//! History: `AddEffect` snapshots inside `add_effect_to_selected`; the
//! other three are in `updates_document`, so the host snapshots around
//! them and drops the entry when nothing changed.

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
        _ => {}
    }
}
