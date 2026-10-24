//! SkSL shader effects (#6).
//!
//! A `PenEffect::Shader` post-processes a node: its program declares the
//! texture it samples as an SkSL child, `uniform shader object;` for the
//! node's own rendered output or `uniform shader background;` for what is
//! already painted behind it. The entrypoint is `half4 main(float2 p)` with
//! `p` in node-local document pixels, the same space a shader fill runs in,
//! so `time` and `mouse` bind as they do for fills; `size` is always the
//! node size (an authored value is ignored).
//!
//! v1 samples one texture per effect: a program that declares `background`
//! is a backdrop pass (its `object` child, if any, reads transparent), every
//! other program is an object pass. Image children (#9) are not bound in
//! effects yet and read transparent.

use jian_ops_schema::style::{PenEffect, ShaderEffectBody, ShaderUniformValue};

use crate::shader_images::shader_children;

/// Child name that samples the node's own rendered output.
pub const OBJECT_CHILD: &str = "object";

/// Child name that samples the backdrop behind the node.
pub const BACKGROUND_CHILD: &str = "background";

/// The program the Effects add menu and `AddNodeEffect("shader")` insert:
/// a pixelate pass over the node, visible on any content and tunable
/// through one `cell` uniform.
pub const DEFAULT_SHADER_EFFECT_SKSL: &str = "// Pixelate: sample the node once per cell.
uniform shader object;
uniform float cell;

half4 main(float2 p) {
    float2 c = (floor(p / cell) + 0.5) * cell;
    return object.eval(c);
}
";

/// `cell` of [`DEFAULT_SHADER_EFFECT_SKSL`], in document px.
pub const DEFAULT_SHADER_EFFECT_CELL: f32 = 8.0;

/// A fresh default shader effect (see [`DEFAULT_SHADER_EFFECT_SKSL`]).
pub fn default_shader_effect() -> PenEffect {
    let mut uniforms = std::collections::BTreeMap::new();
    uniforms.insert(
        "cell".to_string(),
        ShaderUniformValue::Float(DEFAULT_SHADER_EFFECT_CELL),
    );
    PenEffect::Shader(ShaderEffectBody {
        sksl: DEFAULT_SHADER_EFFECT_SKSL.to_string(),
        uniforms: Some(uniforms),
        visible: None,
    })
}

/// Whether the effect program samples the backdrop (declares
/// `uniform shader background;`). Every other program is an object pass.
pub fn samples_background(sksl: &str) -> bool {
    shader_children(sksl).contains(&BACKGROUND_CHILD)
}

#[cfg(test)]
#[path = "shader_effects_tests.rs"]
mod tests;
