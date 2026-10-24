//! The active stroke cap / join / dash (#89) applied to the native
//! stroke paints. The canvas painter scopes the style per node
//! (`de_editor_core::stroke_render_style`); every `NativeBackend` stroke
//! entry point runs its paint through [`apply_active_stroke_style`] after
//! setting its own defaults, so an unstyled stroke keeps them.

use de_editor_core::stroke_render_style::{self, LineCap, LineJoin, StrokeRenderStyle};

/// Override `paint`'s cap / join / path effect with the active style.
/// Returns whether a style was active.
pub(in crate::backend) fn apply_active_stroke_style(paint: &mut skia_safe::Paint) -> bool {
    let Some(style) = stroke_render_style::active() else {
        return false;
    };
    apply_stroke_style(paint, &style);
    true
}

/// Whether a node's stroke style is in scope.
pub(in crate::backend) fn is_active() -> bool {
    stroke_render_style::active().is_some()
}

fn apply_stroke_style(paint: &mut skia_safe::Paint, style: &StrokeRenderStyle) {
    if let Some(cap) = style.cap {
        paint.set_stroke_cap(match cap {
            LineCap::Butt => skia_safe::PaintCap::Butt,
            LineCap::Round => skia_safe::PaintCap::Round,
            LineCap::Square => skia_safe::PaintCap::Square,
        });
    }
    if let Some(join) = style.join {
        paint.set_stroke_join(match join {
            LineJoin::Miter => skia_safe::PaintJoin::Miter,
            LineJoin::Round => skia_safe::PaintJoin::Round,
            LineJoin::Bevel => skia_safe::PaintJoin::Bevel,
        });
    }
    if let Some(dash) = style.dash.as_deref() {
        if let Some(effect) = skia_safe::PathEffect::dash(dash, style.dash_offset) {
            paint.set_path_effect(effect);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_style_overrides_cap_join_and_dash() {
        let mut paint = skia_safe::Paint::default();
        paint.set_stroke_cap(skia_safe::PaintCap::Round);
        assert!(!apply_active_stroke_style(&mut paint));
        assert_eq!(paint.stroke_cap(), skia_safe::PaintCap::Round);

        let style = StrokeRenderStyle {
            cap: Some(LineCap::Square),
            join: Some(LineJoin::Bevel),
            dash: Some(vec![4.0, 2.0]),
            dash_offset: 0.0,
        };
        let _guard = stroke_render_style::enter(Some(&style), 1.0);
        assert!(apply_active_stroke_style(&mut paint));
        assert_eq!(paint.stroke_cap(), skia_safe::PaintCap::Square);
        assert_eq!(paint.stroke_join(), skia_safe::PaintJoin::Bevel);
        assert!(paint.path_effect().is_some());
    }
}
