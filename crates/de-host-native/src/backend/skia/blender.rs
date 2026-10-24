//! Blend-mode → skia paint mapping, including the two scene modes skia has
//! no built-in for (#14).
//!
//! Linear burn (`Cb + Cs - 1`, clamped at 0) and linear dodge (`Cb + Cs`,
//! clamped at 1) run as SkSL runtime blenders. Both use the same separable
//! compositing as skia's built-in modes, written for premultiplied colours:
//! `src·(1-Da) + dst·(1-Sa) + Sa·Da·B(dst/Da, src/Sa)`, which with the linear
//! `B` reduces to the bodies below. The web host's `op_ck_bridge.js` carries
//! the same two programs for CanvasKit.

use de_editor_ui::ImageBlendMode;
use skia_safe::{Blender, Data, RuntimeEffect};

/// SkSL for linear burn.
pub(super) const LINEAR_BURN_SKSL: &str = "half4 main(half4 src, half4 dst) {\
    half3 both = max(src.rgb * dst.a + dst.rgb * src.a - src.a * dst.a, half3(0));\
    return half4(src.rgb * (1 - dst.a) + dst.rgb * (1 - src.a) + both,\
                 src.a + dst.a * (1 - src.a));\
}";

/// SkSL for linear dodge.
pub(super) const LINEAR_DODGE_SKSL: &str = "half4 main(half4 src, half4 dst) {\
    half3 both = min(src.rgb * dst.a + dst.rgb * src.a, half3(src.a * dst.a));\
    return half4(src.rgb * (1 - dst.a) + dst.rgb * (1 - src.a) + both,\
                 src.a + dst.a * (1 - src.a));\
}";

thread_local! {
    // Compiled once per thread; `None` records a failed compile so a broken
    // program is not retried every paint.
    static LINEAR_BURN: Option<Blender> = compile(LINEAR_BURN_SKSL);
    static LINEAR_DODGE: Option<Blender> = compile(LINEAR_DODGE_SKSL);
}

fn compile(sksl: &str) -> Option<Blender> {
    RuntimeEffect::make_for_blender(sksl, None)
        .ok()?
        .make_blender(Data::new_empty(), None)
}

/// The runtime blender for a mode skia has no built-in for.
fn runtime_blender(mode: ImageBlendMode) -> Option<Blender> {
    match mode {
        ImageBlendMode::LinearBurn => LINEAR_BURN.with(Clone::clone),
        ImageBlendMode::LinearDodge => LINEAR_DODGE.with(Clone::clone),
        _ => None,
    }
}

/// Set `mode` on `paint`: a built-in skia blend mode, or the runtime blender
/// for the linear modes (falling back to their closest built-in when the
/// program does not compile).
pub(super) fn apply_blend(paint: &mut skia_safe::Paint, mode: ImageBlendMode) {
    match runtime_blender(mode) {
        Some(blender) => {
            paint.set_blender(blender);
        }
        None => {
            paint.set_blend_mode(to_skia_blend_mode(mode));
        }
    }
}

/// The skia built-in for `mode`. The linear modes map to their closest
/// built-in, used only when their runtime blender is unavailable.
pub(super) fn to_skia_blend_mode(mode: ImageBlendMode) -> skia_safe::BlendMode {
    match mode {
        ImageBlendMode::Normal => skia_safe::BlendMode::SrcOver,
        ImageBlendMode::Darken => skia_safe::BlendMode::Darken,
        ImageBlendMode::Multiply => skia_safe::BlendMode::Multiply,
        ImageBlendMode::Screen => skia_safe::BlendMode::Screen,
        ImageBlendMode::Overlay => skia_safe::BlendMode::Overlay,
        ImageBlendMode::Lighten => skia_safe::BlendMode::Lighten,
        ImageBlendMode::Difference => skia_safe::BlendMode::Difference,
        ImageBlendMode::Hue => skia_safe::BlendMode::Hue,
        ImageBlendMode::Saturation => skia_safe::BlendMode::Saturation,
        ImageBlendMode::Color => skia_safe::BlendMode::Color,
        ImageBlendMode::Luminosity => skia_safe::BlendMode::Luminosity,
        ImageBlendMode::SoftLight => skia_safe::BlendMode::SoftLight,
        ImageBlendMode::ColorDodge => skia_safe::BlendMode::ColorDodge,
        ImageBlendMode::ColorBurn => skia_safe::BlendMode::ColorBurn,
        ImageBlendMode::HardLight => skia_safe::BlendMode::HardLight,
        ImageBlendMode::Exclusion => skia_safe::BlendMode::Exclusion,
        ImageBlendMode::LinearBurn => skia_safe::BlendMode::Multiply,
        ImageBlendMode::LinearDodge => skia_safe::BlendMode::Plus,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Paint `src` over `dst` with `mode` on a 1×1 raster and read it back.
    fn blend(mode: ImageBlendMode, dst: skia_safe::Color4f, src: skia_safe::Color4f) -> [u8; 4] {
        let mut surface = skia_safe::surfaces::raster_n32_premul((1, 1)).unwrap();
        surface.canvas().clear(skia_safe::Color::TRANSPARENT);
        surface
            .canvas()
            .draw_paint(&skia_safe::Paint::new(dst, None));
        let mut paint = skia_safe::Paint::new(src, None);
        apply_blend(&mut paint, mode);
        surface.canvas().draw_paint(&paint);
        let c = surface
            .image_snapshot()
            .peek_pixels()
            .expect("raster pixels")
            .get_color((0, 0));
        [c.r(), c.g(), c.b(), c.a()]
    }

    fn near(actual: [u8; 4], expected: [u8; 4]) -> bool {
        actual
            .iter()
            .zip(expected)
            .all(|(a, e)| (*a as i32 - e as i32).abs() <= 2)
    }

    #[test]
    fn both_linear_programs_compile() {
        assert!(compile(LINEAR_BURN_SKSL).is_some());
        assert!(compile(LINEAR_DODGE_SKSL).is_some());
    }

    #[test]
    fn linear_burn_adds_and_subtracts_one_clamped_at_zero() {
        let dst = skia_safe::Color4f::new(0.8, 0.5, 0.2, 1.0);
        let src = skia_safe::Color4f::new(0.6, 0.6, 0.6, 1.0);
        // 0.8+0.6-1 = 0.4, 0.5+0.6-1 = 0.1, 0.2+0.6-1 < 0 → 0.
        let out = blend(ImageBlendMode::LinearBurn, dst, src);
        assert!(near(out, [102, 26, 0, 255]), "{out:?}");
    }

    #[test]
    fn linear_dodge_adds_clamped_at_one() {
        let dst = skia_safe::Color4f::new(0.2, 0.5, 0.8, 1.0);
        let src = skia_safe::Color4f::new(0.3, 0.3, 0.3, 1.0);
        let out = blend(ImageBlendMode::LinearDodge, dst, src);
        assert!(near(out, [128, 204, 255, 255]), "{out:?}");
    }

    #[test]
    fn linear_modes_over_transparent_keep_the_source() {
        let src = skia_safe::Color4f::new(0.4, 0.2, 0.6, 1.0);
        for mode in [ImageBlendMode::LinearBurn, ImageBlendMode::LinearDodge] {
            let out = blend(mode, skia_safe::Color4f::new(0.0, 0.0, 0.0, 0.0), src);
            assert!(near(out, [102, 51, 153, 255]), "{mode:?}: {out:?}");
        }
    }
}
