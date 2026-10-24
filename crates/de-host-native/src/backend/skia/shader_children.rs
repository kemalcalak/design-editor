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
    /// skia rejects the program (the caller paints the fallback). A child
    /// named in `extra` binds that shader (an effect pass's capture);
    /// another binds the frame texture or image its uniform names (#9,
    /// #165).
    pub(super) fn make_runtime_shader(
        &mut self,
        effect: RuntimeEffect,
        uniforms: &[(&str, &[f32])],
        rect: Rect,
        local_matrix: &Matrix,
        extra: &[(&str, Shader)],
    ) -> Option<Shader> {
        if effect.children().is_empty() {
            // `RuntimeEffect` is an RCHandle — this is a refcount bump,
            // not a recompile.
            let mut builder = RuntimeShaderBuilder::new(effect.clone());
            for (name, values) in uniforms {
                // Tolerate undeclared-name / arity mismatch so one bad
                // uniform doesn't sink the whole fill.
                set_uniform(&mut builder, &effect, name, values);
            }
            return builder.make_shader(local_matrix);
        }
        let box_size = fragment_box(rect, local_matrix);
        let specs = de_editor_core::shader_textures::shader_child_specs(effect.source());
        let mut children = Vec::with_capacity(effect.children().len());
        for child in effect.children() {
            if child.ty() != ChildType::Shader {
                // Colour-filter / blender children have no binding here.
                return None;
            }
            let mipmap = specs
                .iter()
                .any(|spec| spec.name == child.name() && spec.mipmap);
            let shader = match extra.iter().find(|(name, _)| *name == child.name()) {
                Some((_, shader)) => Some(shader.clone()),
                None => uniforms
                    .iter()
                    .find(|(name, _)| *name == child.name())
                    .and_then(|(_, values)| decode_image_id(values))
                    .and_then(|id| self.bound_child(id, box_size, mipmap)),
            };
            let shader =
                shader.unwrap_or_else(|| skia_safe::shaders::color(skia_safe::Color::TRANSPARENT));
            children.push(ChildPtr::Shader(shader));
        }
        let data = skia_safe::Data::new_copy(&uniform_block(&effect, uniforms));
        effect.make_shader(data, &children, local_matrix)
    }
}

impl NativeBackend {
    /// The child shader for id `id`: a frame texture first (#165), else a
    /// resident image, stretched over `box_size`.
    fn bound_child(&mut self, id: u64, box_size: (f32, f32), mipmap: bool) -> Option<Shader> {
        if let Some(texture) = self.frame_texture(id) {
            let image = texture.image.clone();
            let tile = super::shader_textures::tile_mode(Some(texture.kind));
            return stretched(&image, box_size)
                .and_then(|m| super::shader_textures::texture_child(&image, tile, mipmap, &m));
        }
        let image = self.raster_image(id)?;
        let matrix = stretched(&image, box_size)?;
        super::shader_textures::texture_child(&image, skia_safe::TileMode::Clamp, mipmap, &matrix)
    }
}

/// The matrix stretching `image` over `box_size`.
fn stretched(image: &skia_safe::Image, box_size: (f32, f32)) -> Option<Matrix> {
    let (w, h) = (image.width() as f32, image.height() as f32);
    (w > 0.0 && h > 0.0).then(|| Matrix::scale((box_size.0 / w, box_size.1 / h)))
}

/// The node box in the program's `fragCoord` space: the painted rect
/// mapped back through the local matrix, which scales a document-space
/// `size` uniform onto the screen (`runtime_shader_local_matrix`). The web
/// bridge derives its box from the same matrix.
fn fragment_box(rect: Rect, local_matrix: &Matrix) -> (f32, f32) {
    let (sx, sy) = (local_matrix.scale_x(), local_matrix.scale_y());
    if sx > 0.0 && sy > 0.0 {
        (rect.size.x / sx, rect.size.y / sy)
    } else {
        (rect.size.x, rect.size.y)
    }
}

/// Whether `ty` is an integer uniform type (`int` .. `int4`).
fn is_int(ty: uniform::Type) -> bool {
    matches!(
        ty,
        uniform::Type::Int | uniform::Type::Int2 | uniform::Type::Int3 | uniform::Type::Int4
    )
}

/// `value` as the whole number an integer uniform binds (#165): the
/// document stores numbers, rounded here.
fn whole(value: f32) -> i32 {
    value.round() as i32
}

/// Bind `values` to the uniform `name` of `effect` on `builder`: floats
/// for a float uniform, rounded integers for an `int*` one (skia's builder
/// would otherwise copy the float bits into an int slot). An undeclared
/// name or a wrong arity is skipped.
pub(super) fn set_uniform(
    builder: &mut RuntimeShaderBuilder,
    effect: &RuntimeEffect,
    name: &str,
    values: &[f32],
) {
    let Some(slot) = effect.find_uniform(name) else {
        return;
    };
    if std::mem::size_of_val(values) != slot.size_in_bytes() {
        return;
    }
    if is_int(slot.ty()) {
        let ints: Vec<i32> = values.iter().copied().map(whole).collect();
        let _ = builder.set_uniform_int(name, &ints);
    } else {
        let _ = builder.set_uniform_float(name, values);
    }
}

/// The effect's uniform block with every uniform `uniforms` names at its
/// reflected offset, integers as `i32`. Size mismatches stay zero.
fn uniform_block(effect: &RuntimeEffect, uniforms: &[(&str, &[f32])]) -> Vec<u8> {
    let mut block = vec![0u8; effect.uniform_size()];
    for (name, values) in uniforms {
        let Some(slot) = effect.find_uniform(name) else {
            continue;
        };
        let bytes = std::mem::size_of_val(*values);
        if bytes != slot.size_in_bytes() || slot.offset() + bytes > block.len() {
            continue;
        }
        let int = is_int(slot.ty());
        for (index, value) in values.iter().enumerate() {
            let at = slot.offset() + index * 4;
            let raw = if int {
                whole(*value).to_ne_bytes()
            } else {
                value.to_ne_bytes()
            };
            block[at..at + 4].copy_from_slice(&raw);
        }
    }
    block
}

#[cfg(test)]
#[path = "shader_children_tests.rs"]
mod tests;
