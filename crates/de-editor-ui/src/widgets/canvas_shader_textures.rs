//! Shader textures (#165) on the paint side: the fills under a shader fill,
//! the backdrop and the node's SDF, made through the painter's shader
//! texture calls and bound to the program's children by id
//! (`de_editor_core::shader_textures`).
//!
//! A fill's `object` is the fill layers below it, painted again into a
//! texture over transparent (each with the node's shape, without their
//! blend modes). `background` is what was painted before the node's fill
//! stack (one capture per node, taken when its stack starts). An `@sdf`
//! child is the field of the node's silhouette, the shape painted opaque.
//! Effects bind `background` to a capture taken when the pass opens and an
//! `@sdf` child to the pass's own capture (`OBJECT_SDF_TEXTURE`).

use de_editor_core::shader_images::encode_image_id;
use de_editor_core::shader_textures::{
    shader_child_specs, texture_id, ChildSource, TextureKind, OBJECT_SDF_TEXTURE,
};

use crate::layout_scene::{SceneFillLayer, SceneNode, SceneShaderUniform};
use crate::widgets::PaintCx;
use crate::{Color, ImageBlendMode, Rect};

/// The node's paint scale (paint px per node px).
fn zoom_of(node: &SceneNode, world_rect: Rect) -> f32 {
    let width = node.bounds.size.x;
    if width > 0.0 && world_rect.size.x > 0.0 {
        world_rect.size.x / width
    } else {
        1.0
    }
}

/// What the node's silhouette depends on, as a texture key: its id, paint
/// size and shape parameters.
fn silhouette_key(node: &SceneNode, world_rect: Rect) -> String {
    format!(
        "{}|{:.1}x{:.1}|{}|{:?}|{:?}|{}|{}|{:?}",
        node.id,
        world_rect.size.x,
        world_rect.size.y,
        node.corner_radius,
        node.corner_radii,
        node.svg_path,
        node.polygon_sides,
        node.points.len(),
        node.text,
    )
}

fn binding(name: &str, id: u64) -> SceneShaderUniform {
    SceneShaderUniform {
        name: name.to_owned(),
        values: encode_image_id(id).to_vec(),
    }
}

/// Whether some shader fill of `node` samples the backdrop; when it does,
/// capture it (before the stack paints) and return its id.
pub(crate) fn capture_fill_backdrop(
    cx: &mut PaintCx<'_>,
    node: &SceneNode,
    world_rect: Rect,
) -> Option<u64> {
    if !cx.backend.supports_shader_textures() {
        return None;
    }
    let wants = node.fill_layers.iter().any(|layer| match layer {
        SceneFillLayer::Shader { shader, .. } => shader_child_specs(&shader.sksl)
            .iter()
            .any(|child| child.source == ChildSource::Background),
        _ => false,
    });
    if !wants {
        return None;
    }
    let id = texture_id(&node.id, usize::MAX, TextureKind::Background);
    cx.backend.capture_shader_backdrop(id, world_rect);
    Some(id)
}

/// `layer` (fill `slot` of `node`) with its node / backdrop / SDF children
/// bound, after making the textures they read, plus the ids to release
/// once it painted. `None` when the layer needs no texture. `paint_layer`
/// paints one layer with the node's shape.
pub(crate) fn bind_fill_textures(
    cx: &mut PaintCx<'_>,
    node: &SceneNode,
    world_rect: Rect,
    slot: usize,
    backdrop: Option<u64>,
    paint_layer: &mut dyn FnMut(&mut PaintCx<'_>, &SceneFillLayer),
) -> Option<(SceneFillLayer, Vec<u64>)> {
    let layer = node.fill_layers.get(slot)?;
    let SceneFillLayer::Shader { shader, blend_mode } = layer else {
        return None;
    };
    if !cx.backend.supports_shader_textures() {
        return None;
    }
    let children = shader_child_specs(&shader.sksl);
    if children
        .iter()
        .all(|child| child.source == ChildSource::Image)
    {
        return None;
    }
    let mut shader = shader.clone();
    let mut made = Vec::new();
    for child in &children {
        let id = match child.source {
            ChildSource::Image => continue,
            ChildSource::Background => match backdrop {
                Some(id) => id,
                None => continue,
            },
            ChildSource::Object => {
                let id = texture_id(&node.id, slot, TextureKind::Object);
                cx.backend.begin_shader_texture(id, world_rect);
                // Canonical fills run front to back: the ones under this
                // layer follow it in the list.
                for lower in node.fill_layers[slot + 1..].iter().rev() {
                    paint_layer(cx, lower);
                }
                cx.backend.end_shader_texture(false, 1.0);
                made.push(id);
                id
            }
            ChildSource::Sdf => {
                // Kept across frames while the silhouette is unchanged: the
                // id hashes the geometry, so a change makes a new one.
                let id = texture_id(&silhouette_key(node, world_rect), slot, TextureKind::Sdf);
                if !cx.backend.shader_texture_ready(id) {
                    cx.backend.begin_shader_texture(id, world_rect);
                    paint_layer(
                        cx,
                        &SceneFillLayer::Solid {
                            color: Color::WHITE,
                            blend_mode: ImageBlendMode::Normal,
                        },
                    );
                    cx.backend
                        .end_shader_texture(true, zoom_of(node, world_rect));
                }
                id
            }
        };
        shader.uniforms.retain(|uniform| uniform.name != child.name);
        shader.uniforms.push(binding(&child.name, id));
    }
    Some((
        SceneFillLayer::Shader {
            shader,
            blend_mode: *blend_mode,
        },
        made,
    ))
}

/// `uniforms` of an object shader effect with its `background` and `@sdf`
/// children bound: the backdrop captured now (its id is returned for
/// release) and the pass's own capture.
pub(crate) fn bind_effect_textures(
    cx: &mut PaintCx<'_>,
    node: &SceneNode,
    pass_index: usize,
    sksl: &str,
    world_rect: Rect,
    uniforms: &mut Vec<SceneShaderUniform>,
) -> Option<u64> {
    if !cx.backend.supports_shader_textures() {
        return None;
    }
    let mut captured = None;
    for child in shader_child_specs(sksl) {
        let id = match child.source {
            ChildSource::Background => {
                let id = texture_id(&node.id, pass_index, TextureKind::Background);
                cx.backend.capture_shader_backdrop(id, world_rect);
                captured = Some(id);
                id
            }
            ChildSource::Sdf => OBJECT_SDF_TEXTURE,
            ChildSource::Object | ChildSource::Image => continue,
        };
        uniforms.retain(|uniform| uniform.name != child.name);
        uniforms.push(binding(&child.name, id));
    }
    captured
}

#[cfg(test)]
#[path = "canvas_shader_textures_tests.rs"]
mod tests;
