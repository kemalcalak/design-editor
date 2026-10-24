//! Built-in shader-fill presets the property panel offers (#90).
//!
//! A preset is a whole `ShaderFillBody`: `Noise` is the loader-expanded
//! `turbulence` preset, `Fade` and `Glow` are the known-good SkSL
//! programs the `shader-fill` AI skill documents. Raw SkSL authoring
//! stays MCP / file only; the panel only swaps between these bodies.

use std::collections::BTreeMap;

use jian_ops_schema::node::PenNode;
use jian_ops_schema::style::{PenFill, ShaderFillBody, ShaderUniformValue};

use crate::fills::{node_fills, node_fills_mut};
use crate::state::EditorState;
use crate::walkers::find_node_mut;

/// The `turbulence` preset name the loader expands (`de-pen-loader`).
const TURBULENCE: &str = "turbulence";

/// Vertical fade from `accent` to `base`. `size` is bound to the node's
/// resolved size by the layout scene. The colour uniforms are named so
/// `accent` sorts first: the loader's fallback colour and the fill-row
/// swatch both take the first colour uniform (BTreeMap order).
const FADE_SKSL: &str = "uniform half4 accent; uniform half4 base; uniform float2 size; half4 main(float2 p){ float t = clamp(p.y/size.y, 0.0, 1.0); return mix(accent, base, t); }";

/// Radial glow from a bright `accent` centre to dark `base` edges.
const GLOW_SKSL: &str = "uniform half4 accent; uniform half4 base; uniform float2 size; half4 main(float2 p){ float2 c = p/size - 0.5; float d = clamp(length(c)*1.6, 0.0, 1.0); return mix(accent, base, d); }";

/// Dark end colour shared by Fade and Glow.
const DARK: &str = "#0b0614";

/// Accent for a shader body that carries no colour uniform (an MCP-made
/// program, say): the violet the shader-fill skill's glow example uses.
const FALLBACK_ACCENT: &str = "#7c3aed";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShaderPreset {
    /// FBM value noise / film grain (`preset: "turbulence"`).
    Noise,
    /// Top-to-bottom colour fade.
    Fade,
    /// Bright centre fading to dark edges.
    Glow,
}

impl ShaderPreset {
    pub const ALL: [Self; 3] = [Self::Noise, Self::Fade, Self::Glow];

    pub fn label_key(self) -> &'static str {
        match self {
            Self::Noise => "fill.presetNoise",
            Self::Fade => "fill.presetFade",
            Self::Glow => "fill.presetGlow",
        }
    }

    /// The preset's shader body, its first colour seeded with `hex` (the
    /// fill's current colour, so switching presets keeps the hue).
    pub fn body(self, hex: &str) -> ShaderFillBody {
        let mut uniforms = BTreeMap::new();
        let (preset, sksl) = match self {
            Self::Noise => {
                uniforms.insert(
                    "baseFrequency".to_string(),
                    ShaderUniformValue::Vec(vec![0.08, 0.08]),
                );
                uniforms.insert("seed".to_string(), ShaderUniformValue::Float(0.0));
                uniforms.insert("numOctaves".to_string(), ShaderUniformValue::Float(3.0));
                // The turbulence program ignores colour uniforms, but the
                // fallback colour and the fill-row swatch read the first
                // one, and it carries the hue to the next preset picked.
                uniforms.insert("accent".to_string(), color(hex));
                (Some(TURBULENCE.to_string()), None)
            }
            Self::Fade => {
                uniforms.insert("accent".to_string(), color(hex));
                uniforms.insert("base".to_string(), color(DARK));
                uniforms.insert("size".to_string(), size());
                (None, Some(FADE_SKSL.to_string()))
            }
            Self::Glow => {
                uniforms.insert("accent".to_string(), color(hex));
                uniforms.insert("base".to_string(), color(DARK));
                uniforms.insert("size".to_string(), size());
                (None, Some(GLOW_SKSL.to_string()))
            }
        };
        ShaderFillBody {
            preset,
            sksl,
            uniforms: Some(uniforms),
            explain: None,
            opacity: None,
            blend_mode: None,
        }
    }

    /// Which preset a body is, if any: the `turbulence` preset name, or
    /// one of the panel's SkSL programs verbatim.
    pub fn detect(body: &ShaderFillBody) -> Option<Self> {
        if body.preset.as_deref() == Some(TURBULENCE) {
            return Some(Self::Noise);
        }
        match body.sksl.as_deref() {
            Some(FADE_SKSL) => Some(Self::Fade),
            Some(GLOW_SKSL) => Some(Self::Glow),
            _ => None,
        }
    }
}

fn color(hex: &str) -> ShaderUniformValue {
    ShaderUniformValue::Color(hex.to_string())
}

/// Placeholder `size`; the layout scene overwrites it with the node size.
fn size() -> ShaderUniformValue {
    ShaderUniformValue::Vec(vec![0.0, 0.0])
}

/// The preset of the node's primary fill, when that fill is a shader
/// built from one.
pub fn primary_shader_preset(node: &PenNode) -> Option<ShaderPreset> {
    match node_fills(node)?.first()? {
        PenFill::Shader(body) => ShaderPreset::detect(body),
        _ => None,
    }
}

/// Replace the primary shader fill's body with `preset`, keeping its
/// opacity and blend mode. `false` when the primary fill is not a
/// shader or already is that preset.
pub fn set_primary_shader_preset(node: &mut PenNode, preset: ShaderPreset) -> bool {
    let Some(PenFill::Shader(body)) = node_fills_mut(node).and_then(|fills| fills.first_mut())
    else {
        return false;
    };
    if ShaderPreset::detect(body) == Some(preset) {
        return false;
    }
    let hex = first_color_uniform(body)
        .unwrap_or(FALLBACK_ACCENT)
        .to_string();
    let mut next = preset.body(&hex);
    next.opacity = body.opacity;
    next.blend_mode = body.blend_mode.clone();
    *body = next;
    true
}

/// The shader body's first colour uniform (BTreeMap order), skipping
/// image sources bound to a shader child: those are strings too but not
/// colours. This is the colour the fill row's swatch, a type flip and a
/// preset switch carry.
pub fn first_color_uniform(body: &ShaderFillBody) -> Option<&str> {
    let children = crate::shader_images::shader_children(body.sksl.as_deref().unwrap_or_default());
    body.uniforms
        .as_ref()?
        .iter()
        .find_map(|(name, value)| match value {
            ShaderUniformValue::Color(hex) if !children.contains(&name.as_str()) => {
                Some(hex.as_str())
            }
            _ => None,
        })
}

/// The primary fill's shader body when it is a custom program (no
/// preset reproduces it).
pub fn primary_custom_shader(node: &PenNode) -> Option<&ShaderFillBody> {
    match node_fills(node)?.first()? {
        PenFill::Shader(body) if ShaderPreset::detect(body).is_none() => Some(body),
        _ => None,
    }
}

/// Put `custom` back as the primary shader fill's body, keeping the
/// current opacity and blend mode (as a preset switch does). `false`
/// when the primary fill is not a shader or already is that program.
fn restore_primary_custom_shader(node: &mut PenNode, custom: &ShaderFillBody) -> bool {
    let Some(PenFill::Shader(body)) = node_fills_mut(node).and_then(|fills| fills.first_mut())
    else {
        return false;
    };
    let mut next = custom.clone();
    next.opacity = body.opacity;
    next.blend_mode = body.blend_mode.clone();
    if *body == next {
        return false;
    }
    *body = next;
    true
}

impl EditorState {
    /// Swap the anchor node's primary shader fill to `preset`.
    /// Editable-gated; marks the document changed on a real change. A
    /// custom program it replaces is kept in
    /// `editor_ui.custom_shader_bodies` for the "Custom" cell.
    pub fn set_selected_shader_preset(&mut self, preset: ShaderPreset) -> bool {
        let sel = self.selection.anchor.clone();
        if !sel.is_real() || !self.is_editable(&sel) {
            return false;
        }
        let Some(node) = find_node_mut(self.active_children_mut(), &sel) else {
            return false;
        };
        let custom = primary_custom_shader(node).cloned();
        if !set_primary_shader_preset(node, preset) {
            return false;
        }
        if let Some(custom) = custom {
            self.editor_ui.custom_shader_bodies.insert(sel, custom);
        }
        self.mark_document_changed();
        true
    }

    /// Whether the anchor's preset row offers "Custom": its primary
    /// shader is a custom program, or a preset pick stashed one.
    pub fn selected_custom_shader_available(&self) -> bool {
        let sel = &self.selection.anchor;
        crate::walkers::find_node(self.active_children(), sel).is_some_and(|node| {
            primary_custom_shader(node).is_some()
                || (matches!(
                    node_fills(node).and_then(|f| f.first()),
                    Some(PenFill::Shader(_))
                ) && self.editor_ui.custom_shader_bodies.contains_key(sel))
        })
    }

    /// The "Custom" cell: bring back the program a preset pick replaced.
    /// One document change (the host snapshots it, so it undoes); the
    /// stash stays, so undo / redo across it keep the cell working.
    pub fn restore_selected_custom_shader(&mut self) -> bool {
        let sel = self.selection.anchor.clone();
        if !sel.is_real() || !self.is_editable(&sel) {
            return false;
        }
        let Some(custom) = self.editor_ui.custom_shader_bodies.get(&sel).cloned() else {
            return false;
        };
        let changed = find_node_mut(self.active_children_mut(), &sel)
            .is_some_and(|node| restore_primary_custom_shader(node, &custom));
        if changed {
            self.mark_document_changed();
        }
        changed
    }
}

#[cfg(test)]
#[path = "shader_presets_tests.rs"]
mod tests;
