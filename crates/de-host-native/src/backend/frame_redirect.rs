//! `NativeFrameBackend` and its redirected drawing (#165).
//!
//! Shader textures and the object shader-effect passes that sample more
//! than the node itself (its backdrop, images, an `@sdf` child) draw the
//! node into an offscreen surface first. Skia's runtime-shader image filter
//! binds one child only, so those passes cannot be save-layer filters like
//! the plain object pass (`skia/shader_effect.rs`): the frame backend
//! points its canvas at the offscreen surface until the pass closes, then
//! draws the program on the parent with every child bound. The CanvasKit
//! bridge does the same (`op_ck_bridge.js`).

use crate::backend::NativeBackend;
use de_editor_core::shader_textures::{shader_child_specs, ChildSource};
use de_editor_ui::Rect;

pub struct NativeFrameBackend<'a> {
    // `pub(super)` so the offscreen-layer helpers in the sibling
    // `frame_offscreen.rs` can reach the canvas + backend they wrap.
    pub(super) inner: &'a mut NativeBackend,
    /// Where draws go: the frame's canvas, or the top redirect's surface.
    pub(super) canvas: &'a skia_safe::Canvas,
    pub(super) redirects: Vec<Redirect<'a>>,
    /// Open `begin_shader_texture`s: `true` when it redirected, `false`
    /// when it fell back to a save, so the matching end does the same.
    pub(super) textures: Vec<bool>,
}

/// Longest edge of a texture surface; a larger box is clipped to the
/// canvas instead.
const MAX_TEXTURE_EDGE: i32 = 4096;

/// What a redirect captures for.
pub(super) enum Purpose {
    /// Shader texture `id`, closed by `end_shader_texture`.
    Texture(u64),
    /// An object shader-effect pass, closed by the restore that balances
    /// it.
    Effect {
        rect: Rect,
        sksl: String,
        uniforms: Vec<(String, Vec<f32>)>,
    },
}

pub(super) struct Redirect<'a> {
    surface: skia_safe::Surface,
    parent: &'a skia_safe::Canvas,
    /// The offscreen canvas's save count when the redirect began.
    depth: usize,
    /// Top-left of the surface in the parent's device pixels.
    origin: (i32, i32),
    /// Device pixels per paint unit when it began (an SDF's unit).
    scale: f32,
    purpose: Purpose,
}

/// Whether an object pass needs the redirected path: it samples more than
/// the node itself.
pub(super) fn effect_needs_redirect(sksl: &str) -> bool {
    shader_child_specs(sksl)
        .iter()
        .any(|child| child.source != ChildSource::Object)
}

impl<'a> NativeFrameBackend<'a> {
    /// `rect` (paint coordinates) in the current canvas's device pixels,
    /// clipped to its surface unless `whole` (a texture: the part off the
    /// window is sampled too) and not too large; `None` when nothing is
    /// left.
    fn device_box(&self, rect: Rect, whole: bool) -> Option<skia_safe::IRect> {
        let matrix = self.canvas.local_to_device_as_3x3();
        let (device, _) = matrix.map_rect(skia_safe::Rect::from_xywh(
            rect.origin.x,
            rect.origin.y,
            rect.size.x,
            rect.size.y,
        ));
        let bounds = self.canvas.base_layer_size();
        let device = skia_safe::IRect::from_ltrb(
            device.left.floor() as i32,
            device.top.floor() as i32,
            device.right.ceil() as i32,
            device.bottom.ceil() as i32,
        );
        if whole
            && !device.is_empty()
            && device.width() <= MAX_TEXTURE_EDGE
            && device.height() <= MAX_TEXTURE_EDGE
        {
            return Some(device);
        }
        skia_safe::IRect::intersect(
            &device,
            &skia_safe::IRect::from_wh(bounds.width, bounds.height),
        )
    }

    /// Start drawing into a fresh surface covering `rect` (paint
    /// coordinates) on the current canvas's device. `false` (nothing
    /// changed) when the box is empty or no surface can be made.
    pub(super) fn begin_redirect(&mut self, rect: Rect, purpose: Purpose) -> bool {
        let whole = matches!(purpose, Purpose::Texture(_));
        let Some(device) = self.device_box(rect, whole) else {
            return false;
        };
        let info = skia_safe::ImageInfo::new_n32_premul((device.width(), device.height()), None);
        let Some(mut surface) = self.canvas.new_surface(&info, None) else {
            return false;
        };
        let offscreen = surface.canvas();
        offscreen.clear(skia_safe::Color::TRANSPARENT);
        offscreen.translate((-device.left as f32, -device.top as f32));
        offscreen.concat_44(&self.canvas.local_to_device());
        let depth = offscreen.save_count();
        let m = self.canvas.local_to_device_as_3x3();
        let scale = (m.scale_x() * m.scale_y() - m.skew_x() * m.skew_y())
            .abs()
            .sqrt();
        // SAFETY: the canvas belongs to the `SkSurface` the handle in
        // `redirects` keeps alive, and moving the handle does not move it.
        // `self.canvas` points back at `parent` before that entry drops
        // (`end_redirect`, and `Drop` below for an unbalanced paint).
        let canvas: &'a skia_safe::Canvas = unsafe { &*(offscreen as *const skia_safe::Canvas) };
        self.redirects.push(Redirect {
            surface,
            parent: self.canvas,
            depth,
            origin: (device.left, device.top),
            scale,
            purpose,
        });
        self.canvas = canvas;
        true
    }

    /// Pop the top redirect and point drawing back at its parent.
    fn end_redirect(&mut self) -> Option<Redirect<'a>> {
        let redirect = self.redirects.pop()?;
        self.canvas = redirect.parent;
        Some(redirect)
    }

    /// `capture_shader_backdrop`: the device pixels under `rect`, read
    /// from the frame's own surface (an open redirect's offscreen holds
    /// only the node being captured, not what is behind it).
    pub(super) fn capture_backdrop(&mut self, id: u64, rect: Rect) {
        // A surface snapshot sees what is painted so far; content still in
        // an open save-layer is not on it (the web bridge has the same gap).
        let Some(device) = self.device_box(rect, true) else {
            return;
        };
        let root = self.redirects.first().map_or(self.canvas, |r| r.parent);
        // The current device's origin in the frame surface's pixels.
        let (dx, dy) = self
            .redirects
            .iter()
            .fold((0, 0), |(x, y), r| (x + r.origin.0, y + r.origin.1));
        let bounds = root.base_layer_size();
        let Some(source_box) = skia_safe::IRect::intersect(
            &device.with_offset((dx, dy)),
            &skia_safe::IRect::from_wh(bounds.width, bounds.height),
        ) else {
            return;
        };
        // SAFETY: `surface()` is `unsafe` only because the returned handle
        // could outlive the canvas; it is dropped at the end of this call.
        if let Some(mut source) = unsafe { root.surface() } {
            if let Some(image) = source.image_snapshot_with_bounds(source_box) {
                let origin = (source_box.left - dx, source_box.top - dy);
                self.inner.install_frame_texture(id, image, origin, 1.0);
            }
        }
    }

    /// `end_shader_texture`: store the texture the matching begin drew.
    pub(super) fn end_texture(&mut self, sdf: bool, sdf_scale: f32) {
        if !self.textures.pop().unwrap_or(false) {
            // The begin found no surface and saved instead.
            self.canvas.restore();
            return;
        }
        let Some(mut redirect) = self.end_redirect() else {
            return;
        };
        let Purpose::Texture(id) = redirect.purpose else {
            return;
        };
        let origin = redirect.origin;
        let stored = if sdf {
            let scale = redirect.scale * sdf_scale;
            crate::backend::skia::shader_textures::read_alpha(&mut redirect.surface).and_then(
                |(alpha, w, h)| {
                    crate::backend::skia::shader_textures::sdf_image(&alpha, w, h, scale)
                },
            )
        } else {
            Some((redirect.surface.image_snapshot(), 1.0))
        };
        if let Some((image, texel)) = stored {
            self.inner.install_frame_texture(id, image, origin, texel);
        }
    }

    /// `begin_shader_texture`; a box with no surface falls back to a save
    /// so the matching end stays balanced.
    pub(super) fn begin_texture(&mut self, id: u64, rect: Rect) {
        let redirected = self.begin_redirect(rect, Purpose::Texture(id));
        if !redirected {
            self.canvas.save();
        }
        self.textures.push(redirected);
    }

    /// `push_shader_effect_layer`: a pass sampling more than the node
    /// draws offscreen (#165), the rest are save-layer filters.
    pub(super) fn push_effect_layer(
        &mut self,
        rect: Rect,
        sksl: &str,
        uniforms: &[(&str, &[f32])],
        backdrop: bool,
    ) {
        let redirected = !backdrop
            && effect_needs_redirect(sksl)
            && self.begin_redirect(
                rect,
                Purpose::Effect {
                    rect,
                    sksl: sksl.to_owned(),
                    uniforms: uniforms
                        .iter()
                        .map(|(name, values)| (name.to_string(), values.to_vec()))
                        .collect(),
                },
            );
        if !redirected {
            self.inner
                .push_shader_effect_layer(self.canvas, rect, sksl, uniforms, backdrop);
        }
    }

    /// `restore`: closes an offscreen effect pass waiting for it, else a
    /// plain restore.
    pub(super) fn restore_layer(&mut self) {
        if !self.finish_effect_redirect() {
            self.inner.restore(self.canvas);
        }
    }

    /// The restore closing an effect redirect, if the top one is waiting
    /// for exactly this restore. Returns whether it handled the restore.
    pub(super) fn finish_effect_redirect(&mut self) -> bool {
        let waiting = self.redirects.last().is_some_and(|redirect| {
            matches!(redirect.purpose, Purpose::Effect { .. })
                && self.canvas.save_count() == redirect.depth
        });
        let Some(mut redirect) = waiting.then(|| self.end_redirect()).flatten() else {
            return false;
        };
        let Purpose::Effect {
            rect,
            sksl,
            uniforms,
        } = &redirect.purpose
        else {
            unreachable!("checked above");
        };
        let bound: Vec<(&str, &[f32])> = uniforms
            .iter()
            .map(|(name, values)| (name.as_str(), values.as_slice()))
            .collect();
        let origin = redirect.origin;
        let drawn = self.inner.draw_effect_pass(
            self.canvas,
            *rect,
            sksl,
            &bound,
            &mut redirect.surface,
            origin,
        );
        if !drawn {
            // Keep the node visible when the program cannot be built.
            let image = redirect.surface.image_snapshot();
            self.canvas.save();
            self.canvas.reset_matrix();
            self.canvas
                .draw_image(&image, (origin.0 as f32, origin.1 as f32), None);
            self.canvas.restore();
        }
        true
    }
}

impl NativeFrameBackend<'_> {
    /// The current canvas's save count (tests check balance).
    #[cfg(test)]
    pub(super) fn save_count_for_tests(&self) -> usize {
        self.canvas.save_count()
    }
}

impl Drop for NativeFrameBackend<'_> {
    fn drop(&mut self) {
        // An unbalanced paint left redirects open: drop them innermost
        // first, pointing back at each parent.
        while self.end_redirect().is_some() {}
    }
}

/// The `RenderBackend` items for offscreen effect passes and shader
/// textures, expanded inside the impl in `frame_backend.rs`, whose one impl
/// block sits at the 800-line cap. Every item forwards to an inherent
/// method above.
macro_rules! shader_texture_items {
    () => {
        fn push_shader_effect_layer(
            &mut self,
            rect: Rect,
            sksl: &str,
            uniforms: &[(&str, &[f32])],
            backdrop: bool,
        ) {
            self.push_effect_layer(rect, sksl, uniforms, backdrop);
        }
        fn restore(&mut self) {
            self.restore_layer();
        }
        fn supports_shader_textures(&self) -> bool {
            true
        }
        fn capture_shader_backdrop(&mut self, id: u64, rect: Rect) {
            self.capture_backdrop(id, rect);
        }
        fn begin_shader_texture(&mut self, id: u64, rect: Rect) {
            self.begin_texture(id, rect);
        }
        fn end_shader_texture(&mut self, sdf: bool, sdf_scale: f32) {
            self.end_texture(sdf, sdf_scale);
        }
        fn release_shader_texture(&mut self, id: u64) {
            self.inner.release_frame_texture(id);
        }
        fn shader_texture_ready(&self, id: u64) -> bool {
            self.inner.frame_texture(id).is_some()
        }
    };
}
pub(super) use shader_texture_items;

#[cfg(test)]
#[path = "frame_redirect_tests.rs"]
mod tests;
