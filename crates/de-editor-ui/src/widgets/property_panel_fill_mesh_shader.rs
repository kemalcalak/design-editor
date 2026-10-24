//! Bodies under a primary Mesh gradient or Shader fill (#90).
//!
//! - **Mesh**: a two-column grid of vertex swatches in stored order
//!   (row-major for a panel-made mesh, so a 2×2 lattice reads as its
//!   corners). A click opens the HSV picker on that vertex through
//!   `ColorTarget::GradientStop(i)`: the snapshot lists mesh vertices
//!   as the primary fill's stops. Adding vertices or resizing the
//!   lattice is a follow-up.
//! - **Shader**: one preset row (`de_editor_core::ShaderPreset`), plus a
//!   trailing "Custom" cell when the node has a custom program on it or
//!   stashed by a preset pick (#141).

use crate::theme::Theme;
use crate::widgets::property_panel::{NodeSnapshot, PropertyPanelAction};
use crate::widgets::property_panel_fill_body::stop_hex_rgb_only;
use crate::widgets::property_panel_inputs::{INPUT_HEIGHT, INPUT_RADIUS, PAD_X};
use crate::widgets::property_panel_segments::{paint_segment_row, segment_rects};
use crate::widgets::text_metrics;
use crate::widgets::PaintCx;
use crate::{Point2D, Rect, TextLayout};
use de_editor_core::{ColorTarget, FillType, ShaderPreset};

const MESH_COLUMNS: usize = 2;
const MESH_COLUMN_GAP: f32 = 8.0;
const MESH_ROW_GAP: f32 = 4.0;
/// Label of the preset row's "Custom" cell.
const CUSTOM_LABEL_KEY: &str = "fill.presetCustom";
/// Space under the last body row before the next fill row.
const BODY_TRAILING_GAP: f32 = 6.0;

/// Body height under a primary mesh fill with `vertex_count` vertices.
pub(crate) fn mesh_body_height(vertex_count: usize) -> f32 {
    let rows = vertex_count.div_ceil(MESH_COLUMNS);
    if rows == 0 {
        return 0.0;
    }
    rows as f32 * (INPUT_HEIGHT + MESH_ROW_GAP) - MESH_ROW_GAP + BODY_TRAILING_GAP
}

/// Body height under a primary shader fill: the preset row.
pub(crate) fn shader_body_height() -> f32 {
    INPUT_HEIGHT + BODY_TRAILING_GAP
}

/// One rect per mesh vertex, two per row.
fn mesh_vertex_rects(x: f32, body_y: f32, width: f32, count: usize) -> Vec<Rect> {
    let usable_w = width - PAD_X * 2.0;
    let cell_w = (usable_w - MESH_COLUMN_GAP * (MESH_COLUMNS as f32 - 1.0)) / MESH_COLUMNS as f32;
    (0..count)
        .map(|i| {
            let col = (i % MESH_COLUMNS) as f32;
            let row = (i / MESH_COLUMNS) as f32;
            Rect::xywh(
                x + PAD_X + col * (cell_w + MESH_COLUMN_GAP),
                body_y + row * (INPUT_HEIGHT + MESH_ROW_GAP),
                cell_w,
                INPUT_HEIGHT,
            )
        })
        .collect()
}

fn shader_row_rect(x: f32, body_y: f32, width: f32) -> Rect {
    Rect::xywh(x + PAD_X, body_y, width - PAD_X * 2.0, INPUT_HEIGHT)
}

/// Cells in the preset row: the presets, then "Custom" when offered.
fn shader_cell_count(custom: bool) -> usize {
    ShaderPreset::ALL.len() + usize::from(custom)
}

/// Action rects for the primary fill's mesh / shader body; nothing for
/// any other fill type.
pub(crate) fn push_mesh_shader_action_rects(
    out: &mut Vec<(PropertyPanelAction, Rect)>,
    fill_type: FillType,
    x: f32,
    body_y: f32,
    width: f32,
    vertex_count: usize,
    shader_custom: bool,
) {
    match fill_type {
        FillType::MeshGradient => {
            for (index, rect) in mesh_vertex_rects(x, body_y, width, vertex_count)
                .into_iter()
                .enumerate()
            {
                out.push((
                    PropertyPanelAction::OpenColorPicker(ColorTarget::GradientStop(index)),
                    rect,
                ));
            }
        }
        FillType::Shader => {
            let cells = segment_rects(
                shader_row_rect(x, body_y, width),
                shader_cell_count(shader_custom),
            );
            let actions = ShaderPreset::ALL
                .into_iter()
                .map(PropertyPanelAction::SetShaderPreset)
                .chain(shader_custom.then_some(PropertyPanelAction::RestoreCustomShader));
            out.extend(actions.zip(cells));
        }
        _ => {}
    }
}

/// The vertex grid: swatch plus `#RRGGBB` per vertex.
pub(crate) fn paint_fill_mesh_body(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    snapshot: &NodeSnapshot,
    x: f32,
    body_y: f32,
    width: f32,
) {
    let rects = mesh_vertex_rects(x, body_y, width, snapshot.gradient_stops.len());
    for (vertex, cell) in snapshot.gradient_stops.iter().zip(rects) {
        cx.backend.fill_round_rect(cell, INPUT_RADIUS, theme.muted);
        let swatch = Rect::xywh(cell.origin.x + 6.0, cell.origin.y + 5.0, 16.0, 16.0);
        jian_widgets::components::swatch::Swatch {
            color: vertex.color,
            radius: 3.0,
            border: false,
        }
        .paint(
            cx.backend,
            swatch,
            &crate::widgets::button::tokens_from_theme(theme),
        );
        let text_x = cell.origin.x + 30.0;
        let hex = text_metrics::fit_in_family(
            cx.backend,
            &stop_hex_rgb_only(&vertex.hex),
            (cell.origin.x + cell.size.x - 6.0 - text_x).max(0.0),
            12.0,
            text_metrics::MONO_FONT_FAMILY,
        );
        let layout = TextLayout::single_run(
            &hex,
            text_metrics::MONO_FONT_FAMILY,
            12.0,
            theme.foreground.to_jian(),
            Point2D::new(0.0, 0.0),
        );
        cx.backend.draw_text(
            &layout,
            Point2D::new(
                text_x,
                cell.origin.y + crate::widgets::property_panel_inputs::INPUT_TEXT_BASELINE,
            ),
        );
    }
}

/// The preset row. A custom program highlights the "Custom" cell; with
/// none offered it highlights nothing.
pub(crate) fn paint_fill_shader_body(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    snapshot: &NodeSnapshot,
    locale: de_editor_core::Locale,
    x: f32,
    body_y: f32,
    width: f32,
) {
    let custom = snapshot.shader_custom_available;
    let labels: Vec<&str> = ShaderPreset::ALL
        .iter()
        .map(|preset| preset.label_key())
        .chain(custom.then_some(CUSTOM_LABEL_KEY))
        .map(|key| de_i18n::translate(locale, key))
        .collect();
    let active = match snapshot.shader_preset {
        Some(preset) => ShaderPreset::ALL.iter().position(|p| *p == preset),
        None => custom.then_some(ShaderPreset::ALL.len()),
    };
    paint_segment_row(
        cx,
        theme,
        shader_row_rect(x, body_y, width),
        &labels,
        active,
        11.0,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mesh_grid_wraps_two_vertices_per_row() {
        let rects = mesh_vertex_rects(0.0, 100.0, 248.0, 4);
        assert_eq!(rects.len(), 4);
        assert_eq!(rects[0].origin.y, rects[1].origin.y);
        assert!(rects[1].origin.x > rects[0].origin.x);
        assert_eq!(rects[2].origin.x, rects[0].origin.x);
        assert_eq!(rects[2].origin.y, 100.0 + INPUT_HEIGHT + MESH_ROW_GAP);
        let last = rects[3];
        assert!(last.origin.y + last.size.y <= 100.0 + mesh_body_height(4));
        assert_eq!(mesh_body_height(0), 0.0);
    }
}
