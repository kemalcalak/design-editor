//! Gradient resolution for the scene builder.
//!
//! Pure code motion out of the `layout_scene.rs` spine (800-line ceiling):
//! payload → [`SceneGradient`] conversion and node-opacity folding.

use jian_scene::layout_scene::{SceneGradient, SceneGradientStop};

use super::array_to_color;
use crate::payload::{GradientPayload, GradientStopPayload};

/// Fold node opacity into a gradient by scaling its alpha multiplier
/// only. The backend folds `opacity` into every stop colour when it
/// builds the shader (`gradient_color_arrays`), so scaling the stops
/// here too would dim the gradient twice. Leave stops at their
/// authored alpha; the single `opacity` multiplier carries node
/// opacity.
pub(super) fn scale_gradient_opacity(g: SceneGradient, k: f32) -> SceneGradient {
    match g {
        SceneGradient::Linear {
            angle_deg,
            opacity,
            stops,
        } => SceneGradient::Linear {
            angle_deg,
            opacity: (opacity * k).clamp(0.0, 1.0),
            stops,
        },
        SceneGradient::Radial {
            cx,
            cy,
            radius,
            opacity,
            stops,
        } => SceneGradient::Radial {
            cx,
            cy,
            radius,
            opacity: (opacity * k).clamp(0.0, 1.0),
            stops,
        },
        SceneGradient::Angular {
            cx,
            cy,
            rotation_deg,
            opacity,
            stops,
        } => SceneGradient::Angular {
            cx,
            cy,
            rotation_deg,
            opacity: (opacity * k).clamp(0.0, 1.0),
            stops,
        },
        SceneGradient::Mesh {
            rows,
            cols,
            colors,
            opacity,
        } => SceneGradient::Mesh {
            rows,
            cols,
            colors,
            opacity: (opacity * k).clamp(0.0, 1.0),
        },
    }
}

/// Convert a [`GradientPayload`] into the paint-only
/// [`SceneGradient`]. Each stop's `[r,g,b,a]` is unpacked into a
/// [`Color`]; the body's opacity rides through unchanged so the
/// painter can fold it into the per-stop alpha at draw time.
pub(super) fn payload_gradient_to_scene(g: &GradientPayload) -> SceneGradient {
    match g {
        GradientPayload::Linear {
            angle_deg,
            opacity,
            stops,
        } => SceneGradient::Linear {
            angle_deg: *angle_deg,
            opacity: *opacity,
            stops: stops.iter().map(stop_to_scene).collect(),
        },
        GradientPayload::Radial {
            cx,
            cy,
            radius,
            opacity,
            stops,
        } => SceneGradient::Radial {
            cx: *cx,
            cy: *cy,
            radius: *radius,
            opacity: *opacity,
            stops: stops.iter().map(stop_to_scene).collect(),
        },
        GradientPayload::Angular {
            cx,
            cy,
            rotation_deg,
            opacity,
            stops,
        } => SceneGradient::Angular {
            cx: *cx,
            cy: *cy,
            rotation_deg: *rotation_deg,
            opacity: *opacity,
            stops: stops.iter().map(stop_to_scene).collect(),
        },
        GradientPayload::Mesh {
            rows,
            cols,
            colors,
            opacity,
        } => SceneGradient::Mesh {
            rows: *rows,
            cols: *cols,
            colors: colors.iter().map(|c| array_to_color(*c)).collect(),
            opacity: *opacity,
        },
    }
}

fn stop_to_scene(s: &GradientStopPayload) -> SceneGradientStop {
    SceneGradientStop {
        offset: s.offset,
        color: array_to_color(s.color),
    }
}
