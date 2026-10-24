//! Runtime shaders with image children (#9) on [`NativeBackend`].
//!
//! A shader fill's `uniform shader <name>;` child arrives as a uniform of
//! the same name carrying the image's encoded paint id
//! (`de_editor_core::shader_images`). `RuntimeShaderBuilder` has no child
//! setter in skia-safe, so a program with children is built through
//! `RuntimeEffect::make_shader` with the uniform block written by reflection
//! (same tolerance as the builder: an unknown name or a wrong arity is
//! skipped). Each image child samples the decoded raster stretched over the
//! node box in `fragCoord` space; an image that is not resident yet binds
//! transparent so the program still runs.

use de_editor_core::shader_images::decode_image_id;
use de_editor_ui::Rect;
use skia_safe::runtime_effect::{uniform, ChildPtr, ChildType, RuntimeShaderBuilder};
use skia_safe::{Matrix, RuntimeEffect, Shader};

use super::NativeBackend;

impl NativeBackend {
    /// The runtime shader for `effect` with `uniforms` bound, or `None` when
    /// skia rejects the program (the caller paints the fallback).
    pub(super) fn make_runtime_shader(
        &mut self,
        effect: RuntimeEffect,
        uniforms: &[(&str, &[f32])],
        rect: Rect,
        local_matrix: &Matrix,
    ) -> Option<Shader> {
        if effect.children().is_empty() {
            // `RuntimeEffect` is an RCHandle — this is a refcount bump,
            // not a recompile.
            let mut builder = RuntimeShaderBuilder::new(effect);
            for (name, values) in uniforms {
                // Tolerate undeclared-name / arity mismatch so one bad
                // uniform doesn't sink the whole fill.
                let _ = builder.set_uniform_float(*name, values);
            }
            return builder.make_shader(local_matrix);
        }
        let box_size = fragment_box(rect, uniforms);
        let mut children = Vec::with_capacity(effect.children().len());
        for child in effect.children() {
            if child.ty() != ChildType::Shader {
                // Colour-filter / blender children have no binding here.
                return None;
            }
            let image = uniforms
                .iter()
                .find(|(name, _)| *name == child.name())
                .and_then(|(_, values)| decode_image_id(values))
                .and_then(|id| self.raster_image(id));
            let shader = image
                .and_then(|image| image_child(&image, box_size))
                .unwrap_or_else(|| skia_safe::shaders::color(skia_safe::Color::TRANSPARENT));
            children.push(ChildPtr::Shader(shader));
        }
        let data = skia_safe::Data::new_copy(&uniform_block(&effect, uniforms));
        effect.make_shader(data, &children, local_matrix)
    }
}

/// The node box in the program's `fragCoord` space: the document-space
/// `size` uniform when the loader bound one (the local matrix scales it to
/// the screen), else the painted rect.
fn fragment_box(rect: Rect, uniforms: &[(&str, &[f32])]) -> (f32, f32) {
    uniforms
        .iter()
        .find(|(name, values)| *name == "size" && values.len() == 2)
        .map(|(_, values)| (values[0], values[1]))
        .filter(|(w, h)| w.is_finite() && h.is_finite() && *w > 0.0 && *h > 0.0)
        .unwrap_or((rect.size.x, rect.size.y))
}

/// `image` stretched over `box_size`, clamped at the edges.
fn image_child(image: &skia_safe::Image, box_size: (f32, f32)) -> Option<Shader> {
    let (w, h) = (image.width() as f32, image.height() as f32);
    if w <= 0.0 || h <= 0.0 {
        return None;
    }
    let matrix = Matrix::scale((box_size.0 / w, box_size.1 / h));
    image.to_shader(
        (skia_safe::TileMode::Clamp, skia_safe::TileMode::Clamp),
        skia_safe::SamplingOptions::new(skia_safe::FilterMode::Linear, skia_safe::MipmapMode::None),
        &matrix,
    )
}

/// The effect's uniform block with every float uniform `uniforms` names at
/// its reflected offset. Integer uniforms and size mismatches stay zero.
fn uniform_block(effect: &RuntimeEffect, uniforms: &[(&str, &[f32])]) -> Vec<u8> {
    let mut block = vec![0u8; effect.uniform_size()];
    for (name, values) in uniforms {
        let Some(slot) = effect.find_uniform(name) else {
            continue;
        };
        let is_int = matches!(
            slot.ty(),
            uniform::Type::Int | uniform::Type::Int2 | uniform::Type::Int3 | uniform::Type::Int4
        );
        let bytes = std::mem::size_of_val(*values);
        if is_int || bytes != slot.size_in_bytes() || slot.offset() + bytes > block.len() {
            continue;
        }
        for (index, value) in values.iter().enumerate() {
            let at = slot.offset() + index * 4;
            block[at..at + 4].copy_from_slice(&value.to_ne_bytes());
        }
    }
    block
}

#[cfg(test)]
#[path = "shader_children_tests.rs"]
mod tests;
