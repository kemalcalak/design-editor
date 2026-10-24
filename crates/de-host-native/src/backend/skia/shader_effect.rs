//! SkSL shader-effect layers (#6) on [`NativeBackend`].
//!
//! An object pass is a save-layer whose image filter is the program with
//! its `object` child bound to the layer content
//! (`SkImageFilters::RuntimeShader`); a backdrop pass puts the same filter,
//! with the `background` child, in the layer's backdrop slot. Skia accepts
//! only a translate between a runtime-shader filter and the device, so the
//! layer opens with the matrix reset to device pixels (keeping the node's
//! content at full resolution) and the current matrix is set again inside
//! it; the program is wrapped so it still runs in node-local document
//! pixels (`shader_effect_source.rs`). The extra save that reset needs is
//! popped by [`NativeBackend::restore`] together with the layer. The
//! output is cropped to the node box.

use std::hash::{Hash, Hasher};

use de_editor_core::shader_effects::{BACKGROUND_CHILD, OBJECT_CHILD};
use de_editor_ui::Rect;
use skia_safe::runtime_effect::RuntimeShaderBuilder;

use super::gradient::runtime_shader_local_matrix;
use super::shader_effect_source::{float3x3, wrap_effect_source, DEVICE_TO_DOC, DOC_TO_DEVICE};
use super::{to_sk_rect, NativeBackend};

/// Wrapped programs kept before the cache starts over.
const WRAPPED_SOURCE_CAP: usize = 256;

/// One open shader-effect layer: on `canvas`, at save depth `layer`, whose
/// restore must also unwind to `base`.
#[derive(Debug, Clone, Copy)]
pub(super) struct EffectLayerMark {
    canvas: usize,
    layer: usize,
    base: usize,
}

impl NativeBackend {
    /// Begin a shader-effect layer over `rect`; see
    /// `Painter::push_shader_effect_layer`. Anything skia rejects (compile
    /// error, missing child, singular matrix) degrades to a plain save.
    pub fn push_shader_effect_layer(
        &mut self,
        canvas: &skia_safe::Canvas,
        rect: Rect,
        sksl: &str,
        uniforms: &[(&str, &[f32])],
        backdrop: bool,
    ) {
        let Some((filter, device_box)) =
            self.shader_effect_filter(canvas, rect, sksl, uniforms, backdrop)
        else {
            canvas.save();
            return;
        };
        let matrix = canvas.local_to_device();
        let base = canvas.save();
        canvas.reset_matrix();
        let mut paint = skia_safe::Paint::default();
        let rec = skia_safe::canvas::SaveLayerRec::default().bounds(&device_box);
        if backdrop {
            canvas.save_layer(
                &rec.backdrop(&filter)
                    .backdrop_tile_mode(skia_safe::TileMode::Clamp),
            );
        } else {
            paint.set_image_filter(filter);
            canvas.save_layer(&rec.paint(&paint));
        }
        canvas.set_matrix(&matrix);
        self.mark_effect_layer(canvas, base);
    }

    /// The cropped device-space runtime-shader filter for one pass plus the
    /// node box in device pixels.
    fn shader_effect_filter(
        &mut self,
        canvas: &skia_safe::Canvas,
        rect: Rect,
        sksl: &str,
        uniforms: &[(&str, &[f32])],
        backdrop: bool,
    ) -> Option<(skia_safe::ImageFilter, skia_safe::Rect)> {
        if !(rect.size.x > 0.0 && rect.size.y > 0.0) {
            return None;
        }
        let device = canvas.local_to_device_as_3x3();
        let doc_to_device =
            skia_safe::Matrix::concat(&device, &runtime_shader_local_matrix(rect, uniforms));
        let device_to_doc = doc_to_device.invert()?;
        let child = if backdrop {
            BACKGROUND_CHILD
        } else {
            OBJECT_CHILD
        };
        let wrapped = wrapped_effect_source(&mut self.effect_sources, sksl, child);
        // A program that doesn't declare `child` fails to compile here,
        // since the wrapper samples it (cached as a failure like any other).
        let effect = self.shader_cache.get_or_compile(wrapped)?;
        let mut builder = RuntimeShaderBuilder::new(effect);
        for (name, values) in uniforms {
            // Same tolerance as shader fills: a bad uniform is skipped.
            let _ = builder.set_uniform_float(*name, values);
        }
        builder
            .set_uniform_float(DOC_TO_DEVICE, &float3x3(&doc_to_device))
            .ok()?;
        builder
            .set_uniform_float(DEVICE_TO_DOC, &float3x3(&device_to_doc))
            .ok()?;
        let shader = skia_safe::image_filters::runtime_shader(&builder, child, None)?;
        let (device_box, _) = device.map_rect(to_sk_rect(rect));
        let filter =
            skia_safe::image_filters::crop(device_box, skia_safe::TileMode::Decal, shader)?;
        Some((filter, device_box))
    }

    /// Record that the save at `base` belongs to the effect layer now on
    /// top of `canvas`, so the restore that pops the layer also pops it.
    fn mark_effect_layer(&self, canvas: &skia_safe::Canvas, base: usize) {
        let key = canvas as *const skia_safe::Canvas as usize;
        let mut marks = self.effect_layer_marks.borrow_mut();
        marks.retain(|mark| mark.canvas != key || mark.layer <= base);
        marks.push(EffectLayerMark {
            canvas: key,
            layer: canvas.save_count(),
            base,
        });
    }

    /// Pop the most recent save; when it was an effect layer, also pop the
    /// device-matrix save beneath it.
    pub fn restore(&self, canvas: &skia_safe::Canvas) {
        let mut marks = self.effect_layer_marks.borrow_mut();
        if marks.is_empty() {
            // The common case: no effect layer is open anywhere.
            canvas.restore();
            return;
        }
        let key = canvas as *const skia_safe::Canvas as usize;
        let before = canvas.save_count();
        canvas.restore();
        // Forget marks a `restore_to` (or a previous frame) left behind.
        marks.retain(|mark| mark.canvas != key || mark.layer <= before);
        if let Some(at) = marks
            .iter()
            .rposition(|mark| mark.canvas == key && mark.layer == before)
        {
            let mark = marks.remove(at);
            canvas.restore_to_count(mark.base);
        }
    }
}

/// The device-space wrapper of `sksl` for `child` from `cache`, built
/// once per program (paint asks for it every frame).
fn wrapped_effect_source<'a>(
    cache: &'a mut std::collections::HashMap<u64, String>,
    sksl: &str,
    child: &str,
) -> &'a str {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    (sksl, child).hash(&mut hasher);
    let key = hasher.finish();
    if cache.len() >= WRAPPED_SOURCE_CAP && !cache.contains_key(&key) {
        cache.clear();
    }
    cache
        .entry(key)
        .or_insert_with(|| wrap_effect_source(sksl, child))
}

#[cfg(test)]
#[path = "shader_effect_tests.rs"]
mod tests;
