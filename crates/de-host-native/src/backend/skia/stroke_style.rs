//! Stroke cap / join / dash (#89) on the native stroke paints. The canvas
//! painter hands each node's style over through
//! `RenderBackend::set_stroke_style`; every `NativeBackend` stroke entry
//! point runs its paint through [`NativeBackend::apply_stroke_style`] after
//! setting its own defaults, so an unset field keeps them.

use de_editor_ui::layout_scene::{StrokeCap, StrokeJoin, StrokeStyle};

use super::NativeBackend;

impl NativeBackend {
    /// The style for the strokes that follow, until the next call.
    pub fn set_stroke_style(&mut self, style: StrokeStyle) {
        self.stroke_style = style;
    }

    /// Whether a non-plain style is set.
    pub(in crate::backend) fn has_stroke_style(&self) -> bool {
        !self.stroke_style.is_plain()
    }

    /// Override `paint`'s cap / join / path effect with the set style.
    pub(in crate::backend) fn apply_stroke_style(&self, paint: &mut skia_safe::Paint) {
        apply_style(paint, &self.stroke_style);
    }
}

fn apply_style(paint: &mut skia_safe::Paint, style: &StrokeStyle) {
    if let Some(cap) = style.cap {
        paint.set_stroke_cap(match cap {
            StrokeCap::Butt => skia_safe::PaintCap::Butt,
            StrokeCap::Round => skia_safe::PaintCap::Round,
            StrokeCap::Square => skia_safe::PaintCap::Square,
        });
    }
    if let Some(join) = style.join {
        paint.set_stroke_join(match join {
            StrokeJoin::Miter => skia_safe::PaintJoin::Miter,
            StrokeJoin::Round => skia_safe::PaintJoin::Round,
            StrokeJoin::Bevel => skia_safe::PaintJoin::Bevel,
        });
    }
    if let Some(dash) = style.dash {
        if let Some(effect) = skia_safe::PathEffect::dash(dash.lengths(), dash.offset()) {
            paint.set_path_effect(effect);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use de_editor_ui::layout_scene::StrokeDash;

    #[test]
    fn a_set_style_overrides_cap_join_and_dash_and_plain_keeps_defaults() {
        let mut paint = skia_safe::Paint::default();
        paint.set_stroke_cap(skia_safe::PaintCap::Round);
        apply_style(&mut paint, &StrokeStyle::default());
        assert_eq!(paint.stroke_cap(), skia_safe::PaintCap::Round);
        assert!(paint.path_effect().is_none());

        let style = StrokeStyle {
            cap: Some(StrokeCap::Square),
            join: Some(StrokeJoin::Bevel),
            dash: StrokeDash::new(&[4.0, 2.0], 0.0),
        };
        apply_style(&mut paint, &style);
        assert_eq!(paint.stroke_cap(), skia_safe::PaintCap::Square);
        assert_eq!(paint.stroke_join(), skia_safe::PaintJoin::Bevel);
        assert!(paint.path_effect().is_some());
    }
}
