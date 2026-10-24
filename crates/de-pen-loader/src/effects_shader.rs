//! SkSL shader effects (#6) across the loader: canonical
//! `PenEffect::Shader` → [`ShaderEffectPayload`] → scene
//! [`Effect::Shader`]. Uniforms resolve through the shader-fill path
//! ([`crate::style_payload::shader_payload`]) so a colour, float or vector
//! binds the same way in an effect as in a fill.

use jian_ops_schema::node::PenNode;
use jian_ops_schema::style::{PenEffect, PenFill, ShaderFillBody};
use jian_scene::layout_scene::{Effect, SceneShaderUniform, ShaderEffect, ShaderEffectInput};
use serde::{Deserialize, Serialize};

use crate::payload::ShaderUniformPayload;

/// One visible shader effect in the desktop payload, in list order.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShaderEffectPayload {
    pub sksl: String,
    #[serde(default)]
    pub uniforms: Vec<ShaderUniformPayload>,
    /// `true` when the program samples the backdrop (`background`).
    #[serde(default)]
    pub background: bool,
}

/// The node's visible shader effects with a non-empty program.
pub fn shader_effects_from_canonical(node: &PenNode) -> Vec<ShaderEffectPayload> {
    let Some(effects) = crate::effects::canonical_node_effects(node) else {
        return Vec::new();
    };
    effects
        .iter()
        .filter_map(|effect| match effect {
            PenEffect::Shader(body) if body.visible != Some(false) => {
                let fill = PenFill::Shader(ShaderFillBody {
                    preset: None,
                    sksl: Some(body.sksl.clone()),
                    uniforms: body.uniforms.clone(),
                    explain: None,
                    opacity: None,
                    blend_mode: None,
                });
                let resolved = crate::style_payload::shader_payload(&fill)?;
                Some(ShaderEffectPayload {
                    background: de_editor_core::shader_effects::samples_background(&resolved.sksl),
                    sksl: resolved.sksl,
                    uniforms: resolved.uniforms,
                })
            }
            _ => None,
        })
        .collect()
}

/// The scene pass for `payload` on a node of document `size`. `size` is
/// always bound (overriding an authored value), since an effect's
/// coordinate space is the node box.
pub fn shader_effect_to_scene(payload: &ShaderEffectPayload, size: [f32; 2]) -> Effect {
    let mut uniforms: Vec<SceneShaderUniform> = payload
        .uniforms
        .iter()
        .filter(|uniform| uniform.name != "size")
        .map(|uniform| SceneShaderUniform {
            name: uniform.name.clone(),
            values: uniform.values.clone(),
        })
        .collect();
    uniforms.push(SceneShaderUniform {
        name: "size".to_string(),
        values: size.to_vec(),
    });
    Effect::Shader(ShaderEffect {
        sksl: payload.sksl.clone(),
        uniforms,
        input: if payload.background {
            ShaderEffectInput::Background
        } else {
            ShaderEffectInput::Object
        },
    })
}

#[cfg(test)]
#[path = "effects_shader_tests.rs"]
mod tests;
