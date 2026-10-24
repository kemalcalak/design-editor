//! Image textures for shader fills (#9) on the paint side.
//!
//! The loader turns a string uniform bound to a `uniform shader` child into
//! the image's encoded paint id (see `de_editor_core::shader_images`). Before
//! the shader paints, [`prepare_shader_images`] runs each such image through
//! the same seam an image fill uses: bytes from the shared cache (or the
//! inline `data:` decode, or the host's remote fetch), then the backend's
//! decode check, queueing a decode when the raster is missing or too coarse.
//! The backend then finds the raster by id when it binds the child. Until it
//! lands the child samples transparent, and the decode pump's repaint brings
//! the texture in, exactly like an image fill's placeholder → bitmap swap.
//! Exports run the same paint over their synchronous decode pump, so they
//! capture the texture.

use super::canvas_viewport_image::{
    cached_bytes_for, image_source_bytes, note_pending_decode, required_raster_edge,
};
use super::PaintCx;
use crate::Rect;
use de_editor_core::shader_images::{decode_image_id, image_source, shader_children};

/// The image ids `sksl`'s shader children are bound to in `uniforms`, in
/// declaration order. A child without a decodable binding is skipped.
pub(crate) fn shader_image_ids(sksl: &str, uniforms: &[(&str, &[f32])]) -> Vec<u64> {
    shader_children(sksl)
        .into_iter()
        .filter_map(|child| {
            uniforms
                .iter()
                .find(|(name, _)| *name == child)
                .and_then(|(_, values)| decode_image_id(values))
        })
        .collect()
}

/// Make every image the shader's children sample resident on the backend
/// (or queued for decode) at the resolution `world_rect` needs on screen.
pub(crate) fn prepare_shader_images(
    cx: &mut PaintCx<'_>,
    sksl: &str,
    uniforms: &[(&str, &[f32])],
    world_rect: Rect,
) {
    let ids = shader_image_ids(sksl, uniforms);
    if ids.is_empty() {
        return;
    }
    let max_edge_px = required_raster_edge(world_rect, cx.backend.dpi_scale());
    for id in ids {
        let bytes = match image_source(id) {
            Some(src) => image_source_bytes(&src, id),
            None => cached_bytes_for(id),
        };
        let Some(bytes) = bytes else {
            continue;
        };
        if !cx.backend.image_decoded(id, &bytes, max_edge_px) {
            note_pending_decode(id, max_edge_px);
        }
    }
}

#[cfg(test)]
#[path = "canvas_shader_images_tests.rs"]
mod tests;
