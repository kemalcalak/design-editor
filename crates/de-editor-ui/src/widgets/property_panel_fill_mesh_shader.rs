//! Bodies under a primary Mesh gradient or Shader fill (#90).
//!
//! - **Mesh**: a Rows and a Columns stepper (`−` value `+`, #142) over a
//!   two-column grid of vertex swatches in stored order (row-major once
//!   the lattice was resized here, so a 2×2 lattice reads as its
//!   corners). A stepper resizes the lattice a row or column at a time
//!   (`de_editor_core::mesh_lattice`); a swatch click opens the HSV
//!   picker on that vertex through `ColorTarget::GradientStop(i)`: the
//!   snapshot lists mesh vertices as the primary fill's stops.
//! - **Shader**: one preset row (`de_editor_core::ShaderPreset`).

use crate::theme::Theme;
use crate::widgets::icons::{draw_icon, Icon};
use crate::widgets::property_panel::{NodeSnapshot, PropertyPanelAction};
use crate::widgets::property_panel_fill_body::stop_hex_rgb_only;
use crate::widgets::property_panel_inputs::{
    INPUT_HEIGHT, INPUT_RADIUS, INPUT_TEXT_BASELINE, PAD_X,
};
use crate::widgets::property_panel_segments::{paint_segment_row, segment_rects};
use crate::widgets::text_metrics;
use crate::widgets::PaintCx;
use crate::{Point2D, Rect, TextLayout};
use de_editor_core::mesh_lattice::{MESH_MAX_LINES, MESH_MIN_LINES};
use de_editor_core::{ColorTarget, FillType, MeshAxis, MeshLatticeEdit, ShaderPreset};

const MESH_COLUMNS: usize = 2;
const MESH_COLUMN_GAP: f32 = 8.0;
const MESH_ROW_GAP: f32 = 4.0;
/// Space under the last body row before the next fill row.
const BODY_TRAILING_GAP: f32 = 6.0;
/// The Rows and Columns stepper rows above the vertex grid.
const STEPPER_AXES: [MeshAxis; 2] = [MeshAxis::Rows, MeshAxis::Columns];
/// One stepper cell (`−`, the value, `+`).
const STEPPER_CELL_W: f32 = 28.0;
/// Row label size (the stroke style rows' label size).
const STEPPER_FONT_SIZE: f32 = 11.0;
/// The value between `−` and `+`, in the mono face like the vertex hex.
const STEPPER_VALUE_FONT_SIZE: f32 = 12.0;
const STEPPER_ICON_SIZE: f32 = 14.0;
/// A `−` / `+` at its limit paints at this share of the muted ink.
const STEPPER_DISABLED_ALPHA: f32 = 0.4;

/// Body height under a primary mesh fill with `vertex_count` vertices:
/// the two stepper rows plus the vertex grid.
pub(crate) fn mesh_body_height(vertex_count: usize) -> f32 {
    let rows = STEPPER_AXES.len() + vertex_count.div_ceil(MESH_COLUMNS);
    rows as f32 * (INPUT_HEIGHT + MESH_ROW_GAP) - MESH_ROW_GAP + BODY_TRAILING_GAP
}

/// Body height under a primary shader fill: the preset row.
pub(crate) fn shader_body_height() -> f32 {
    INPUT_HEIGHT + BODY_TRAILING_GAP
}

/// The stepper row for `axis_index` (0 = Rows, 1 = Columns).
fn stepper_row_rect(x: f32, body_y: f32, width: f32, axis_index: usize) -> Rect {
    Rect::xywh(
        x + PAD_X,
        body_y + axis_index as f32 * (INPUT_HEIGHT + MESH_ROW_GAP),
        width - PAD_X * 2.0,
        INPUT_HEIGHT,
    )
}

/// The right-aligned `−` value `+` well of a stepper row.
fn stepper_well(row: Rect) -> Rect {
    let w = STEPPER_CELL_W * 3.0;
    Rect::xywh(row.origin.x + row.size.x - w, row.origin.y, w, row.size.y)
}

/// `(remove, value, add)` cells of a stepper well.
fn stepper_cells(well: Rect) -> [Rect; 3] {
    std::array::from_fn(|i| {
        Rect::xywh(
            well.origin.x + i as f32 * STEPPER_CELL_W,
            well.origin.y,
            STEPPER_CELL_W,
            well.size.y,
        )
    })
}

/// One rect per mesh vertex, two per row, under the stepper rows.
fn mesh_vertex_rects(x: f32, body_y: f32, width: f32, count: usize) -> Vec<Rect> {
    let grid_y = body_y + STEPPER_AXES.len() as f32 * (INPUT_HEIGHT + MESH_ROW_GAP);
    let usable_w = width - PAD_X * 2.0;
    let cell_w = (usable_w - MESH_COLUMN_GAP * (MESH_COLUMNS as f32 - 1.0)) / MESH_COLUMNS as f32;
    (0..count)
        .map(|i| {
            let col = (i % MESH_COLUMNS) as f32;
            let row = (i / MESH_COLUMNS) as f32;
            Rect::xywh(
                x + PAD_X + col * (cell_w + MESH_COLUMN_GAP),
                grid_y + row * (INPUT_HEIGHT + MESH_ROW_GAP),
                cell_w,
                INPUT_HEIGHT,
            )
        })
        .collect()
}

fn shader_row_rect(x: f32, body_y: f32, width: f32) -> Rect {
    Rect::xywh(x + PAD_X, body_y, width - PAD_X * 2.0, INPUT_HEIGHT)
}

/// Action rects for the primary fill's mesh / shader body; nothing for
/// any other fill type. Both stepper buttons are always targets: one at
/// its limit is a no-op the host drops from history.
pub(crate) fn push_mesh_shader_action_rects(
    out: &mut Vec<(PropertyPanelAction, Rect)>,
    fill_type: FillType,
    x: f32,
    body_y: f32,
    width: f32,
    vertex_count: usize,
) {
    match fill_type {
        FillType::MeshGradient => {
            for (i, axis) in STEPPER_AXES.into_iter().enumerate() {
                let [remove, _, add] =
                    stepper_cells(stepper_well(stepper_row_rect(x, body_y, width, i)));
                out.push((
                    PropertyPanelAction::EditMeshLattice(MeshLatticeEdit::Remove(axis)),
                    remove,
                ));
                out.push((
                    PropertyPanelAction::EditMeshLattice(MeshLatticeEdit::Add(axis)),
                    add,
                ));
            }
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
            let cells = segment_rects(shader_row_rect(x, body_y, width), ShaderPreset::ALL.len());
            for (preset, cell) in ShaderPreset::ALL.into_iter().zip(cells) {
                out.push((PropertyPanelAction::SetShaderPreset(preset), cell));
            }
        }
        _ => {}
    }
}

/// The stepper rows, then the vertex grid: swatch plus `#RRGGBB` per
/// vertex.
pub(crate) fn paint_fill_mesh_body(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    snapshot: &NodeSnapshot,
    locale: de_editor_core::Locale,
    x: f32,
    body_y: f32,
    width: f32,
) {
    let (rows, cols) = snapshot
        .mesh_lattice
        .unwrap_or((MESH_MIN_LINES, MESH_MIN_LINES));
    for (i, axis) in STEPPER_AXES.into_iter().enumerate() {
        let (key, lines) = match axis {
            MeshAxis::Rows => ("fill.meshRows", rows),
            MeshAxis::Columns => ("fill.meshColumns", cols),
        };
        paint_stepper_row(
            cx,
            theme,
            stepper_row_rect(x, body_y, width, i),
            de_i18n::translate(locale, key),
            lines,
        );
    }
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
            Point2D::new(text_x, cell.origin.y + INPUT_TEXT_BASELINE),
        );
    }
}

/// Label on the left, `−` `lines` `+` well on the right; a button at
/// its limit paints dimmed.
fn paint_stepper_row(cx: &mut PaintCx<'_>, theme: &Theme, row: Rect, label: &str, lines: u32) {
    let well = stepper_well(row);
    let label = text_metrics::fit_chrome(
        cx.backend,
        label,
        (well.origin.x - row.origin.x - 6.0).max(0.0),
        STEPPER_FONT_SIZE,
    );
    let layout = TextLayout::single_run(
        &label,
        text_metrics::CHROME_FONT_FAMILY,
        STEPPER_FONT_SIZE,
        theme.muted_foreground.to_jian(),
        Point2D::new(0.0, 0.0),
    );
    cx.backend.draw_text(
        &layout,
        Point2D::new(row.origin.x, row.origin.y + INPUT_TEXT_BASELINE),
    );

    cx.backend.fill_round_rect(well, INPUT_RADIUS, theme.muted);
    let [remove, value, add] = stepper_cells(well);
    for (cell, icon, enabled) in [
        (remove, Icon::Minus, lines > MESH_MIN_LINES),
        (add, Icon::Plus, lines < MESH_MAX_LINES),
    ] {
        let color = if enabled {
            theme.foreground
        } else {
            crate::util::alpha(theme.muted_foreground, STEPPER_DISABLED_ALPHA)
        };
        draw_icon(
            cx.backend,
            icon,
            Point2D::new(
                cell.origin.x + (cell.size.x - STEPPER_ICON_SIZE) / 2.0,
                cell.origin.y + (cell.size.y - STEPPER_ICON_SIZE) / 2.0,
            ),
            STEPPER_ICON_SIZE,
            color,
            1.4,
        );
    }
    let text = lines.to_string();
    let text_w = text_metrics::measure_in_family(
        cx.backend,
        &text,
        STEPPER_VALUE_FONT_SIZE,
        text_metrics::MONO_FONT_FAMILY,
    );
    let layout = TextLayout::single_run(
        &text,
        text_metrics::MONO_FONT_FAMILY,
        STEPPER_VALUE_FONT_SIZE,
        theme.foreground.to_jian(),
        Point2D::new(0.0, 0.0),
    );
    cx.backend.draw_text(
        &layout,
        Point2D::new(
            value.origin.x + (value.size.x - text_w) / 2.0,
            value.origin.y + INPUT_TEXT_BASELINE,
        ),
    );
}

/// The preset row; a custom program highlights no cell.
pub(crate) fn paint_fill_shader_body(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    snapshot: &NodeSnapshot,
    locale: de_editor_core::Locale,
    x: f32,
    body_y: f32,
    width: f32,
) {
    let labels: Vec<&str> = ShaderPreset::ALL
        .iter()
        .map(|preset| de_i18n::translate(locale, preset.label_key()))
        .collect();
    let active = snapshot
        .shader_preset
        .and_then(|preset| ShaderPreset::ALL.iter().position(|p| *p == preset));
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
    fn mesh_grid_wraps_two_vertices_per_row_under_the_steppers() {
        let rects = mesh_vertex_rects(0.0, 100.0, 248.0, 4);
        assert_eq!(rects.len(), 4);
        let grid_y = 100.0 + 2.0 * (INPUT_HEIGHT + MESH_ROW_GAP);
        assert_eq!(rects[0].origin.y, grid_y);
        assert_eq!(rects[0].origin.y, rects[1].origin.y);
        assert!(rects[1].origin.x > rects[0].origin.x);
        assert_eq!(rects[2].origin.x, rects[0].origin.x);
        assert_eq!(rects[2].origin.y, grid_y + INPUT_HEIGHT + MESH_ROW_GAP);
        let last = rects[3];
        assert!(last.origin.y + last.size.y <= 100.0 + mesh_body_height(4));
        // A body with no vertices still holds the two steppers.
        let columns = stepper_row_rect(0.0, 100.0, 248.0, 1);
        assert!(columns.origin.y + columns.size.y <= 100.0 + mesh_body_height(0));
    }

    #[test]
    fn stepper_cells_sit_right_aligned_inside_the_row() {
        let row = stepper_row_rect(0.0, 0.0, 248.0, 0);
        let [remove, value, add] = stepper_cells(stepper_well(row));
        assert_eq!(add.origin.x + add.size.x, row.origin.x + row.size.x);
        assert!(remove.origin.x < value.origin.x && value.origin.x < add.origin.x);
        assert!(remove.origin.x > row.origin.x + 40.0, "room for the label");
    }
}
