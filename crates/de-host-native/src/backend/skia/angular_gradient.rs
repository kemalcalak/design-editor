//! Angular (conic / sweep) gradient paint methods on [`NativeBackend`].
//!
//! Skia's sweep gradient starts at 3 o'clock; the canonical rotation is
//! CSS `conic-gradient`'s (0° = 12 o'clock, clockwise), so the shader's
//! local matrix turns it by `rotation - 90°` about the centre.

use super::gradient::{fold_opacity, gradient_color_arrays, premul_interpolation};
use super::{to_sk_rect, NativeBackend};
use de_editor_ui::{Color, Rect};

/// Build the sweep shader for an angular gradient over `rect`, or
/// `None` when skia refuses the (degenerate) input.
pub(super) fn angular_gradient_shader(
    rect: Rect,
    stops: &[(f32, Color)],
    cx_frac: f32,
    cy_frac: f32,
    rotation_deg: f32,
    opacity: f32,
) -> Option<skia_safe::Shader> {
    let center = skia_safe::Point::new(
        rect.origin.x + rect.size.x * cx_frac.clamp(0.0, 1.0),
        rect.origin.y + rect.size.y * cy_frac.clamp(0.0, 1.0),
    );
    let rotation = if rotation_deg.is_finite() {
        rotation_deg
    } else {
        0.0
    };
    let local = skia_safe::Matrix::rotate_deg_pivot(rotation - 90.0, center);
    let (colors, offsets) = gradient_color_arrays(stops, opacity);
    let grad_colors = skia_safe::gradient::Colors::new(
        &colors[..],
        Some(offsets.as_slice()),
        skia_safe::TileMode::Clamp,
        None,
    );
    let gradient = skia_safe::gradient::Gradient::new(grad_colors, premul_interpolation());
    skia_safe::gradient::shaders::sweep_gradient(center, (0.0, 360.0), &gradient, &local)
}

impl NativeBackend {
    /// Fill a (round-)rectangle with an angular gradient centred at
    /// (`cx_frac`, `cy_frac`) of `rect`. Stops + opacity follow the
    /// linear convention.
    #[allow(clippy::too_many_arguments)]
    pub fn fill_round_rect_angular_gradient(
        &self,
        canvas: &skia_safe::Canvas,
        rect: Rect,
        radius: f32,
        stops: &[(f32, Color)],
        cx_frac: f32,
        cy_frac: f32,
        rotation_deg: f32,
        opacity: f32,
    ) {
        if stops.is_empty() {
            return;
        }
        let Some(shader) =
            angular_gradient_shader(rect, stops, cx_frac, cy_frac, rotation_deg, opacity)
        else {
            self.fill_round_rect(canvas, rect, radius, fold_opacity(stops[0].1, opacity));
            return;
        };
        let mut paint = skia_safe::Paint::default();
        paint.set_anti_alias(true);
        paint.set_shader(shader);
        canvas.draw_round_rect(to_sk_rect(rect), radius, radius, &paint);
    }

    /// Fill a document SVG path (fitted into `rect`) with an angular
    /// gradient; geometry matches the round-rect variant.
    #[allow(clippy::too_many_arguments)]
    pub fn fill_svg_path_in_rect_angular_gradient_with_fill_rule(
        &mut self,
        canvas: &skia_safe::Canvas,
        d: &str,
        rect: Rect,
        stops: &[(f32, Color)],
        cx_frac: f32,
        cy_frac: f32,
        rotation_deg: f32,
        opacity: f32,
        even_odd: bool,
    ) {
        if stops.is_empty() {
            return;
        }
        let Some(path) = self.fitted_svg_path(d, rect, Some(even_odd)) else {
            return;
        };
        let shader = angular_gradient_shader(rect, stops, cx_frac, cy_frac, rotation_deg, opacity);
        self.draw_path_with_gradient(canvas, &path, shader, stops, opacity);
    }
}
