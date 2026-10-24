//! Paint extractors read by [`super::NodeSnapshot::from_node`] — the
//! per-fill head-row summaries plus the primary gradient's angle and
//! resolved stops.
//!
//! Split out of `property_panel_snapshot.rs` to keep both files under
//! the design-editor 800-line cap.

use super::{color_from_hex, FillSummary, GradientStopSummary};
use crate::Color;
use jian_ops_schema::node::PenNode;

/// LinearGradient `angle` (or AngularGradient `rotation`) for the
/// node's first fill, when it has one. Falls back to `0.0` (canonical
/// default) when the body omits it. `None` for other primary fills —
/// the Fill section uses that to hide the angle row.
pub(super) fn gradient_angle_of(node: &PenNode) -> Option<f32> {
    use jian_ops_schema::style::PenFill;
    match de_editor_core::fills::node_fills(node).and_then(|f| f.first())? {
        PenFill::LinearGradient(body) => Some(body.angle.unwrap_or(0.0)),
        PenFill::AngularGradient(body) => Some(body.rotation.unwrap_or(0.0)),
        _ => None,
    }
}

/// Resolved stops for the primary Linear / Radial gradient, or the
/// vertices of a primary mesh gradient in stored order (their `offset`
/// is 0: a vertex has a lattice position instead, and its colour is
/// edited through the same `ColorTarget::GradientStop(i)`). Empty for
/// Solid / Image / Shader / no-fill nodes.
pub(super) fn gradient_stops_of(node: &PenNode) -> Vec<GradientStopSummary> {
    use jian_ops_schema::style::PenFill;
    let Some(first) = de_editor_core::fills::node_fills(node).and_then(|f| f.first()) else {
        return Vec::new();
    };
    let summary = |offset: f32, hex: &str| GradientStopSummary {
        offset: offset.clamp(0.0, 1.0),
        hex: hex.to_string(),
        color: color_from_hex(hex).unwrap_or(Color::BLACK),
    };
    match first {
        PenFill::LinearGradient(b) => b
            .stops
            .iter()
            .map(|s| summary(s.offset, &s.color))
            .collect(),
        PenFill::RadialGradient(b) => b
            .stops
            .iter()
            .map(|s| summary(s.offset, &s.color))
            .collect(),
        PenFill::AngularGradient(b) => b
            .stops
            .iter()
            .map(|s| summary(s.offset, &s.color))
            .collect(),
        PenFill::MeshGradient(b) => b.stops.iter().map(|s| summary(0.0, &s.color)).collect(),
        _ => Vec::new(),
    }
}

/// Build one [`FillSummary`] per `PenFill` on the node, in authored
/// order. Each entry's representative colour mirrors `fills::fill_hex`
/// (Solid → its colour; gradient → first stop; image → white). An old
/// single-fill node yields exactly one entry; a node with no `fill`
/// field / no fills yields an empty list so the Fill section paints
/// just its header + "+".
pub(super) fn fills_of(node: &PenNode) -> Vec<FillSummary> {
    use jian_ops_schema::style::PenFill;
    let Some(fills) = de_editor_core::fills::node_fills(node) else {
        return Vec::new();
    };
    fills
        .iter()
        .enumerate()
        .map(|(index, fill)| {
            let fill_type = de_editor_core::fills::fill_type_of(fill);
            let (hex, opacity) = match fill {
                PenFill::Solid(b) => (Some(b.color.as_str()), b.opacity.unwrap_or(1.0)),
                PenFill::LinearGradient(b) => (
                    b.stops.first().map(|s| s.color.as_str()),
                    b.opacity.unwrap_or(1.0),
                ),
                PenFill::RadialGradient(b) => (
                    b.stops.first().map(|s| s.color.as_str()),
                    b.opacity.unwrap_or(1.0),
                ),
                PenFill::AngularGradient(b) => (
                    b.stops.first().map(|s| s.color.as_str()),
                    b.opacity.unwrap_or(1.0),
                ),
                PenFill::MeshGradient(b) => (
                    b.stops.first().map(|s| s.color.as_str()),
                    b.opacity.unwrap_or(1.0),
                ),
                PenFill::Shader(b) => (
                    // Head-row swatch uses the shader's first colour
                    // uniform (its visible fallback colour) when present;
                    // an image bound to a shader child is not one.
                    de_editor_core::shader_presets::first_color_uniform(b),
                    b.opacity.unwrap_or(1.0),
                ),
                PenFill::Image(b) => (None, b.opacity.unwrap_or(1.0)),
            };
            FillSummary {
                fill_type,
                color: hex.and_then(color_from_hex).unwrap_or(Color::WHITE),
                opacity,
                blend_mode: de_editor_core::fill_blend_mode_at(node, index),
                variable_ref: None,
                shader: de_editor_core::shader_edit::shader_program(
                    node,
                    de_editor_core::ShaderTarget::Fill(index),
                )
                .map(|program| {
                    crate::widgets::property_panel_shader_body::ShaderBody::of(program, None)
                }),
            }
        })
        .collect()
}
