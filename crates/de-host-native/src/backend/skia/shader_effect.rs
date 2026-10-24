//! SkSL shader-effect layers (#6) on [`NativeBackend`].
//!
//! An object pass is a save-layer whose image filter is the program with
//! its `object` child bound to the layer content
//! (`SkImageFilters::RuntimeShader`); a backdrop pass puts the same filter,
//! with the `background` child, in the layer's backdrop slot. Image filters
//! run in the layer's parameter space, the local coordinates at
//! `save_layer` time, so the layer opens under a node-local transform
//! (document px, scaled by the `size` uniform like a shader fill) and the
//! inverse is applied inside it, leaving the node's own draws where they
//! were. That costs one extra save, which [`NativeBackend::restore`] pops
//! together with the layer. The output is cropped to the node box.

use de_editor_core::shader_effects::{BACKGROUND_CHILD, OBJECT_CHILD};
use de_editor_ui::Rect;
use skia_safe::runtime_effect::RuntimeShaderBuilder;

use super::NativeBackend;

impl NativeBackend {
    /// Begin a shader-effect layer over `rect`; see
    /// `Painter::push_shader_effect_layer`. Anything skia rejects (compile
    /// error, missing child) degrades to a plain save.
    pub fn push_shader_effect_layer(
        &mut self,
        canvas: &skia_safe::Canvas,
        rect: Rect,
        sksl: &str,
        uniforms: &[(&str, &[f32])],
        backdrop: bool,
    ) {
        let Some((filter, scale)) = self.shader_effect_filter(rect, sksl, uniforms, backdrop)
        else {
            canvas.save();
            return;
        };
        let base = canvas.save();
        canvas.translate((rect.origin.x, rect.origin.y));
        canvas.scale(scale);
        let bounds = skia_safe::Rect::from_wh(rect.size.x / scale.0, rect.size.y / scale.1);
        let mut paint = skia_safe::Paint::default();
        let rec = skia_safe::canvas::SaveLayerRec::default().bounds(&bounds);
        if backdrop {
            canvas.save_layer(
                &rec.backdrop(&filter)
                    .backdrop_tile_mode(skia_safe::TileMode::Clamp),
            );
        } else {
            paint.set_image_filter(filter);
            canvas.save_layer(&rec.paint(&paint));
        }
        canvas.scale((1.0 / scale.0, 1.0 / scale.1));
        canvas.translate((-rect.origin.x, -rect.origin.y));
        self.mark_effect_layer(canvas, base);
    }

    /// The cropped runtime-shader filter for one pass plus the node-local
    /// scale (paint px per document px).
    fn shader_effect_filter(
        &mut self,
        rect: Rect,
        sksl: &str,
        uniforms: &[(&str, &[f32])],
        backdrop: bool,
    ) -> Option<(skia_safe::ImageFilter, (f32, f32))> {
        if !(rect.size.x > 0.0 && rect.size.y > 0.0) {
            return None;
        }
        let scale = effect_scale(rect, uniforms);
        let effect = self.shader_cache.get_or_compile(sksl)?;
        let child = if backdrop {
            BACKGROUND_CHILD
        } else {
            OBJECT_CHILD
        };
        if !effect.children().iter().any(|c| c.name() == child) {
            // Skia aborts on an unknown child name; the program must declare it.
            return None;
        }
        let mut builder = RuntimeShaderBuilder::new(effect);
        for (name, values) in uniforms {
            // Same tolerance as shader fills: a bad uniform is skipped.
            let _ = builder.set_uniform_float(*name, values);
        }
        let shader = skia_safe::image_filters::runtime_shader(&builder, child, None)?;
        let crop = skia_safe::Rect::from_wh(rect.size.x / scale.0, rect.size.y / scale.1);
        let filter = skia_safe::image_filters::crop(crop, skia_safe::TileMode::Decal, shader)?;
        Some((filter, scale))
    }

    /// Record that the save at `base` belongs to the effect layer now on
    /// top of `canvas`, so the restore that pops the layer also pops it.
    fn mark_effect_layer(&self, canvas: &skia_safe::Canvas, base: usize) {
        let key = canvas as *const skia_safe::Canvas as usize;
        let mut marks = self.effect_layer_marks.borrow_mut();
        marks.retain(|mark| mark.0 != key || mark.1 <= base);
        marks.push((key, canvas.save_count(), base));
    }

    /// Pop the most recent save; when it was an effect layer, also pop the
    /// node-local transform save beneath it.
    pub fn restore(&self, canvas: &skia_safe::Canvas) {
        let key = canvas as *const skia_safe::Canvas as usize;
        let before = canvas.save_count();
        canvas.restore();
        let mut marks = self.effect_layer_marks.borrow_mut();
        // Forget marks a `restore_to` (or a previous frame) left behind.
        marks.retain(|mark| mark.0 != key || mark.1 <= before);
        if let Some(at) = marks
            .iter()
            .rposition(|mark| mark.0 == key && mark.1 == before)
        {
            let (_, _, base) = marks.remove(at);
            canvas.restore_to_count(base);
        }
    }
}

/// Paint px per node-local document px: `rect` over the `size` uniform,
/// else 1 (the same rule as a shader fill's local matrix).
fn effect_scale(rect: Rect, uniforms: &[(&str, &[f32])]) -> (f32, f32) {
    uniforms
        .iter()
        .find(|(name, values)| *name == "size" && values.len() == 2)
        .map(|(_, values)| (values[0], values[1]))
        .filter(|(w, h)| w.is_finite() && h.is_finite() && *w > 0.0 && *h > 0.0)
        .map(|(w, h)| (rect.size.x / w, rect.size.y / h))
        .filter(|(sx, sy)| sx.is_finite() && sy.is_finite() && *sx > 0.0 && *sy > 0.0)
        .unwrap_or((1.0, 1.0))
}

#[cfg(test)]
#[path = "shader_effect_tests.rs"]
mod tests;
