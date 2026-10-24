//! Shader textures (#165) on [`NativeBackend`]: the frame textures a
//! shader child binds by id (`de_editor_core::shader_textures`), their
//! signed distance fields, and the child shaders they become.
//!
//! The redirected drawing that fills them lives on `NativeFrameBackend`
//! (`frame_redirect.rs`); this side stores the result and binds it.

use de_editor_core::shader_textures::{
    downsample_alpha, signed_distance_field, texture_kind, TextureKind,
};
use skia_safe::{FilterMode, Image, Matrix, MipmapMode, SamplingOptions, Shader, TileMode};

use super::NativeBackend;

/// One frame texture: its raster and what it holds.
#[derive(Clone)]
pub(crate) struct FrameTexture {
    pub(crate) image: Image,
    pub(crate) kind: TextureKind,
}

/// Textures kept before the store starts over: SDFs stay across frames
/// (their id changes with the silhouette) and a paint that forgot to
/// release cannot grow it without bound.
const FRAME_TEXTURE_CAP: usize = 64;

impl NativeBackend {
    /// Store `image` as frame texture `id`.
    pub(crate) fn install_frame_texture(&mut self, id: u64, image: Image) {
        let Some(kind) = texture_kind(id) else {
            return;
        };
        if self.frame_textures.len() >= FRAME_TEXTURE_CAP && !self.frame_textures.contains_key(&id)
        {
            self.frame_textures.clear();
        }
        self.frame_textures.insert(id, FrameTexture { image, kind });
    }

    /// Forget frame texture `id`.
    pub(crate) fn release_frame_texture(&mut self, id: u64) {
        self.frame_textures.remove(&id);
    }

    /// The frame texture `id`, if one is stored.
    pub(crate) fn frame_texture(&self, id: u64) -> Option<&FrameTexture> {
        self.frame_textures.get(&id)
    }
}

/// The tile mode a texture of `kind` samples with past its edges: the
/// backdrop mirrors (pen.dev's `pencil:background`), the node is
/// transparent, a field clamps.
pub(crate) fn tile_mode(kind: Option<TextureKind>) -> TileMode {
    match kind {
        Some(TextureKind::Background) => TileMode::Mirror,
        Some(TextureKind::Object) => TileMode::Decal,
        Some(TextureKind::Sdf) | None => TileMode::Clamp,
    }
}

/// Linear sampling, mipmapped when the child asks for it.
pub(crate) fn sampling(mipmap: bool) -> SamplingOptions {
    SamplingOptions::new(
        FilterMode::Linear,
        if mipmap {
            MipmapMode::Linear
        } else {
            MipmapMode::None
        },
    )
}

/// `image` as a child shader whose texels map to the program's
/// coordinates through `matrix` (image space → program space).
pub(crate) fn texture_child(
    image: &Image,
    tile: TileMode,
    mipmap: bool,
    matrix: &Matrix,
) -> Option<Shader> {
    let image = if mipmap {
        image
            .with_default_mipmaps()
            .unwrap_or_else(|| image.clone())
    } else {
        image.clone()
    };
    image.to_shader((tile, tile), sampling(mipmap), matrix)
}

/// The signed distance field of `alpha` (`width × height` bytes) as an
/// RGBA float raster, distances divided by `texel_scale` (texels per
/// node pixel at full resolution).
pub(crate) fn sdf_image(
    alpha: &[u8],
    width: usize,
    height: usize,
    texel_scale: f32,
) -> Option<Image> {
    let (alpha, w, h, factor) = downsample_alpha(alpha, width, height);
    let mut field = signed_distance_field(&alpha, w, h);
    if field.is_empty() {
        return None;
    }
    let unit = factor / texel_scale.max(f32::EPSILON);
    for texel in field.chunks_exact_mut(4) {
        texel[0] *= unit;
    }
    let bytes: Vec<u8> = field.iter().flat_map(|v| v.to_ne_bytes()).collect();
    let info = skia_safe::ImageInfo::new(
        (w as i32, h as i32),
        skia_safe::ColorType::RGBAF32,
        skia_safe::AlphaType::Unpremul,
        None,
    );
    skia_safe::images::raster_from_data(&info, skia_safe::Data::new_copy(&bytes), w * 16)
}

impl NativeBackend {
    /// Draw one object shader-effect pass (#165) over `rect` on `canvas`:
    /// `object` samples `capture` (the pass's content, device pixels from
    /// `origin`), an `@sdf` child bound to `OBJECT_SDF_TEXTURE` the field
    /// of its alpha, the rest their frame textures or images. `false` when
    /// skia refuses the program.
    pub(crate) fn draw_effect_pass(
        &mut self,
        canvas: &skia_safe::Canvas,
        rect: de_editor_ui::Rect,
        sksl: &str,
        uniforms: &[(&str, &[f32])],
        capture: &mut skia_safe::Surface,
        origin: (i32, i32),
    ) -> bool {
        use de_editor_core::shader_effects::OBJECT_CHILD;
        use de_editor_core::shader_images::decode_image_id;
        use de_editor_core::shader_textures::{
            shader_child_specs, ChildSource, OBJECT_SDF_TEXTURE,
        };
        let Some(effect) = self.shader_cache.get_or_compile(sksl) else {
            return false;
        };
        let local = super::gradient::runtime_shader_local_matrix(rect, uniforms);
        let total = Matrix::concat(&canvas.local_to_device_as_3x3(), &local);
        let Some(inverse) = total.invert() else {
            return false;
        };
        // Capture texels → program coordinates.
        let to_program = Matrix::concat(
            &inverse,
            &Matrix::translate((origin.0 as f32, origin.1 as f32)),
        );
        let specs = shader_child_specs(sksl);
        let mipmap = |name: &str| specs.iter().any(|s| s.name == name && s.mipmap);
        let image = capture.image_snapshot();
        let mut extra: Vec<(&str, Shader)> = Vec::new();
        if let Some(shader) =
            texture_child(&image, TileMode::Decal, mipmap(OBJECT_CHILD), &to_program)
        {
            extra.push((OBJECT_CHILD, shader));
        }
        let sdf_children: Vec<&str> = specs
            .iter()
            .filter(|spec| spec.source == ChildSource::Sdf)
            .filter(|spec| {
                uniforms
                    .iter()
                    .find(|(name, _)| *name == spec.name)
                    .and_then(|(_, values)| decode_image_id(values))
                    == Some(OBJECT_SDF_TEXTURE)
            })
            .map(|spec| spec.name.as_str())
            .collect();
        if !sdf_children.is_empty() {
            // Texels per program unit: the pass's device scale.
            let det = total.scale_x() * total.scale_y() - total.skew_x() * total.skew_y();
            let scale = det.abs().sqrt();
            let field = read_alpha(capture)
                .and_then(|(alpha, w, h)| sdf_image(&alpha, w, h, scale).map(|f| (f, w)));
            if let Some((field, full_width)) = field {
                let shrink = full_width as f32 / field.width().max(1) as f32;
                let matrix = Matrix::concat(&to_program, &Matrix::scale((shrink, shrink)));
                for name in sdf_children {
                    if let Some(shader) =
                        texture_child(&field, TileMode::Clamp, mipmap(name), &matrix)
                    {
                        extra.push((name, shader));
                    }
                }
            }
        }
        let Some(shader) = self.make_runtime_shader(effect, uniforms, rect, &local, &extra) else {
            return false;
        };
        let mut paint = skia_safe::Paint::default();
        paint.set_shader(shader);
        canvas.draw_rect(super::to_sk_rect(rect), &paint);
        true
    }
}

/// The alpha bytes of `surface` (`width × height`), read back.
pub(crate) fn read_alpha(surface: &mut skia_safe::Surface) -> Option<(Vec<u8>, usize, usize)> {
    let (w, h) = (
        surface.width().max(0) as usize,
        surface.height().max(0) as usize,
    );
    if w == 0 || h == 0 {
        return None;
    }
    let info = skia_safe::ImageInfo::new_a8((w as i32, h as i32));
    let mut alpha = vec![0u8; w * h];
    surface
        .read_pixels(&info, &mut alpha, w, (0, 0))
        .then_some((alpha, w, h))
}
