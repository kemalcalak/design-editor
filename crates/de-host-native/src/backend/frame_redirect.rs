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
}

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
    /// clipped to its surface; `None` when nothing is left.
    fn device_box(&self, rect: Rect) -> Option<skia_safe::IRect> {
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
        skia_safe::IRect::intersect(
            &device,
            &skia_safe::IRect::from_wh(bounds.width, bounds.height),
        )
    }

    /// Start drawing into a fresh surface covering `rect` (paint
    /// coordinates) on the current canvas's device. `false` (nothing
    /// changed) when the box is empty or no surface can be made.
    pub(super) fn begin_redirect(&mut self, rect: Rect, purpose: Purpose) -> bool {
        let Some(device) = self.device_box(rect) else {
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

    /// `capture_shader_backdrop`: the device pixels under `rect`.
    pub(super) fn capture_backdrop(&mut self, id: u64, rect: Rect) {
        // A surface snapshot sees what is painted so far; content still in
        // an open save-layer is not on it (the web bridge has the same gap).
        let Some(device) = self.device_box(rect) else {
            return;
        };
        // SAFETY: `surface()` is `unsafe` only because the returned handle
        // could outlive the canvas; it is dropped at the end of this call.
        if let Some(mut source) = unsafe { self.canvas.surface() } {
            if let Some(image) = source.image_snapshot_with_bounds(device) {
                self.inner.install_frame_texture(id, image);
            }
        }
    }

    /// `end_shader_texture`: store the top texture redirect.
    pub(super) fn end_texture(&mut self, sdf: bool, sdf_scale: f32) {
        if !matches!(
            self.redirects.last().map(|r| &r.purpose),
            Some(Purpose::Texture(_))
        ) {
            // An unbalanced end (the begin found no surface) still restores
            // the default's save.
            self.canvas.restore();
            return;
        }
        let Some(mut redirect) = self.end_redirect() else {
            return;
        };
        let Purpose::Texture(id) = redirect.purpose else {
            return;
        };
        let image = if sdf {
            let scale = redirect.scale * sdf_scale;
            crate::backend::skia::shader_textures::read_alpha(&mut redirect.surface).and_then(
                |(alpha, w, h)| {
                    crate::backend::skia::shader_textures::sdf_image(&alpha, w, h, scale)
                },
            )
        } else {
            Some(redirect.surface.image_snapshot())
        };
        if let Some(image) = image {
            self.inner.install_frame_texture(id, image);
        }
    }

    /// `begin_shader_texture`; a box with no surface falls back to a save
    /// so the matching end stays balanced.
    pub(super) fn begin_texture(&mut self, id: u64, rect: Rect) {
        if !self.begin_redirect(rect, Purpose::Texture(id)) {
            self.canvas.save();
        }
    }

    /// The restore closing an effect redirect, if the top one is waiting
    /// for exactly this restore. Returns whether it handled the restore.
    pub(super) fn finish_effect_redirect(&mut self) -> bool {
        let waiting = self.redirects.last().is_some_and(|redirect| {
            matches!(redirect.purpose, Purpose::Effect { .. })
                && self.canvas.save_count() == redirect.depth
        });
        if !waiting {
            return false;
        }
        let Some(mut redirect) = self.end_redirect() else {
            return false;
        };
        let Purpose::Effect {
            rect,
            sksl,
            uniforms,
        } = &redirect.purpose
        else {
            return false;
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
/// textures, expanded inside the impl in `frame_backend.rs` (one impl block
/// that sits at the 800-line cap).
macro_rules! shader_texture_items {
    () => {
        fn push_shader_effect_layer(
            &mut self,
            rect: Rect,
            sksl: &str,
            uniforms: &[(&str, &[f32])],
            backdrop: bool,
        ) {
            // A pass sampling more than the node draws offscreen (#165).
            let owned = || {
                uniforms
                    .iter()
                    .map(|(n, v)| (n.to_string(), v.to_vec()))
                    .collect()
            };
            let effect = || $crate::backend::frame_redirect::Purpose::Effect {
                rect,
                sksl: sksl.to_owned(),
                uniforms: owned(),
            };
            if backdrop
                || !$crate::backend::frame_redirect::effect_needs_redirect(sksl)
                || !self.begin_redirect(rect, effect())
            {
                self.inner
                    .push_shader_effect_layer(self.canvas, rect, sksl, uniforms, backdrop);
            }
        }

        fn restore(&mut self) {
            if !self.finish_effect_redirect() {
                self.inner.restore(self.canvas);
            }
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
