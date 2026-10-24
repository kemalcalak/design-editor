//! Editable-input rect walker for the property panel.
//!
//! Split from `property_panel_layout.rs` so the action-rect walker
//! and input-rect walker stay under the repository file-size cap.

use crate::widgets::property_panel::{EffectSummary, FillSummary, PropertyPanelAction};
use crate::widgets::property_panel_fill::{fill_row_rects, fill_row_tops};
use crate::widgets::property_panel_inputs::{
    COLOR_VARIABLE_BUTTON_W, COLOR_VARIABLE_GAP, INPUT_HEIGHT, PAD_X, SECTION_HEADER_HEIGHT,
};
use crate::widgets::property_panel_layout::VisibleSections;
use crate::widgets::property_panel_stroke::stroke_side_input_rects;
use crate::{Point2D, Rect};
use de_editor_core::{FillType, PropertyFocus};

/// Emit `(GradientStopHex, rect)` + `(GradientStopOffset, rect)`
/// pairs for `stop_count` rows starting at `*y`, advancing `y` past
/// the last row. Geometry mirrors `paint_fill_gradient_body`'s row
/// layout so click targets land on the boxes the user sees.
fn push_gradient_stop_rects(
    rects: &mut Vec<(PropertyFocus, Rect)>,
    x0: f32,
    y: &mut f32,
    usable_w: f32,
    stop_count: usize,
) {
    let pct_w = 56.0;
    let remove_w = if stop_count > 2 { 26.0 } else { 0.0 };
    let remove_gap = if stop_count > 2 { 6.0 } else { 0.0 };
    let hex_w = usable_w - pct_w - 8.0 - remove_w - remove_gap;
    for index in 0..stop_count {
        rects.push((
            PropertyFocus::GradientStopHex(index),
            Rect {
                origin: Point2D::new(x0 + PAD_X, *y),
                size: Point2D::new(hex_w, INPUT_HEIGHT),
            },
        ));
        rects.push((
            PropertyFocus::GradientStopOffset(index),
            Rect {
                origin: Point2D::new(x0 + PAD_X + hex_w + 8.0, *y),
                size: Point2D::new(pct_w, INPUT_HEIGHT),
            },
        ));
        *y += INPUT_HEIGHT + 4.0;
    }
}

/// On-screen rects of every editable input. Same y walk as
/// `action_button_rects` so paint + hit-test stay aligned.
pub fn editable_input_rects(
    panel_rect: Rect,
    visible: VisibleSections,
    fills: &[FillSummary],
    effects: &[EffectSummary],
) -> Vec<(PropertyFocus, Rect)> {
    let x0 = panel_rect.origin.x;
    let w = panel_rect.size.x;
    let usable_w = w - PAD_X * 2.0;
    let half_w = (usable_w - 8.0) / 2.0;

    // Section origins come from the shared walk (#83); Interactions sits
    // below every editable section, so its summary does not matter here.
    let tops = crate::widgets::property_panel_section_tops::section_tops(
        panel_rect.origin.y,
        visible,
        effects,
        fills,
        &crate::widgets::property_panel_interactions::InteractionSummary::default(),
    );
    let mut y = tops.position + SECTION_HEADER_HEIGHT;
    let x_rect = Rect {
        origin: Point2D::new(x0 + PAD_X, y),
        size: Point2D::new(half_w, INPUT_HEIGHT),
    };
    let y_rect = Rect {
        origin: Point2D::new(x0 + PAD_X + half_w + 8.0, y),
        size: Point2D::new(half_w, INPUT_HEIGHT),
    };
    y += INPUT_HEIGHT + 6.0;
    let size_rects = visible.size_options.then(|| {
        [
            (
                PropertyFocus::SizeW,
                Rect {
                    origin: Point2D::new(x0 + PAD_X, y),
                    size: Point2D::new(half_w, INPUT_HEIGHT),
                },
            ),
            (
                PropertyFocus::SizeH,
                Rect {
                    origin: Point2D::new(x0 + PAD_X + half_w + 8.0, y),
                    size: Point2D::new(half_w, INPUT_HEIGHT),
                },
            ),
        ]
    });
    y += visible.position_size_row_h();
    let rotation_rect = Rect {
        origin: Point2D::new(x0 + PAD_X, y),
        size: Point2D::new(half_w, INPUT_HEIGHT),
    };
    let per_corner_expanded = visible.corner_grid_expanded();
    let radius_rect =
        (visible.corner_radius && !per_corner_expanded).then_some(if visible.corner_per_corner {
            crate::widgets::property_panel_corner::uniform_and_toggle_rects(x0, y, w).0
        } else {
            Rect {
                origin: Point2D::new(x0 + PAD_X + half_w + 8.0, y),
                size: Point2D::new(half_w, INPUT_HEIGHT),
            }
        });
    y += INPUT_HEIGHT;
    let mut rects = vec![
        (PropertyFocus::PositionX, x_rect),
        (PropertyFocus::PositionY, y_rect),
        (PropertyFocus::Rotation, rotation_rect),
    ];
    rects.extend(size_rects.into_iter().flatten());
    if let Some(radius_rect) = radius_rect {
        rects.push((PropertyFocus::PositionR, radius_rect));
    }
    if per_corner_expanded {
        rects.extend(crate::widgets::property_panel_corner::input_rects(x0, y, w));
    }
    if visible.flex_layout {
        crate::widgets::property_panel_flex::push_flex_input_rects(
            &mut rects,
            x0,
            tops.flex + SECTION_HEADER_HEIGHT,
            w,
            visible.flex_layout_mode,
            visible.layout_justify,
        );
    }
    if visible.text {
        crate::widgets::property_panel_text::push_text_input_rects(
            &mut rects, x0, tops.text, usable_w,
        );
    }
    if let Some(kind) = visible.widget {
        crate::widgets::property_panel_widget::push_widget_input_rects(
            &mut rects,
            kind,
            x0,
            tops.widget,
            usable_w,
        );
    }
    if visible.video {
        crate::widgets::property_panel_video::push_video_input_rects(&mut rects, x0, tops.video, w);
    }
    if visible.opacity {
        let mut y = tops.layer + SECTION_HEADER_HEIGHT;
        rects.push((
            PropertyFocus::Opacity,
            Rect {
                origin: Point2D::new(x0 + PAD_X, y),
                size: Point2D::new(
                    crate::widgets::property_panel_layer::opacity_input_width(w),
                    INPUT_HEIGHT,
                ),
            },
        ));
        if visible.polygon_sides {
            rects.push((
                PropertyFocus::PolygonSides,
                Rect {
                    origin: Point2D::new(x0 + PAD_X + half_w + 8.0, y),
                    size: Point2D::new(half_w, INPUT_HEIGHT),
                },
            ));
        }
        y += INPUT_HEIGHT;
        if visible.star_inner {
            y += 6.0;
            rects.push((
                PropertyFocus::StarInnerRadius,
                Rect {
                    origin: Point2D::new(x0 + PAD_X, y),
                    size: Point2D::new(half_w, INPUT_HEIGHT),
                },
            ));
        }
        if visible.ellipse_arc {
            y += 6.0;
            let col_w = (usable_w - 12.0) / 3.0;
            let focuses = [
                PropertyFocus::EllipseStart,
                PropertyFocus::EllipseSweep,
                PropertyFocus::EllipseInnerRadius,
            ];
            for (i, focus) in focuses.into_iter().enumerate() {
                rects.push((
                    focus,
                    Rect {
                        origin: Point2D::new(x0 + PAD_X + i as f32 * (col_w + 6.0), y),
                        size: Point2D::new(col_w, INPUT_HEIGHT),
                    },
                ));
            }
        }
    }
    if visible.fill {
        let first_row_y = tops.fill + SECTION_HEADER_HEIGHT;
        for (fi, fill, row_y) in fill_row_tops(fills, first_row_y, visible.gradient_stop_count) {
            let is_primary = fi == 0;
            let fill_type = fill.fill_type;
            // The row: opacity, plus the hex for a solid fill. A
            // variable-bound primary fill paints a `$name` chip, not an
            // editable hex.
            let row = fill_row_rects(x0, row_y, w, visible.fill_row_shows_variable(fi));
            rects.push((PropertyFocus::FillOpacity(fi), row.opacity));
            if fill_type == FillType::Solid && visible.fill_hex_editable(fi) {
                rects.push((PropertyFocus::FillHex(fi), row.value));
            }
            let y = row_y + INPUT_HEIGHT + 6.0;
            match fill_type {
                // Gradient / image bodies only paint editable inputs for
                // the primary fill (they key off `fills[0]`).
                // Linear / Angular: the Angle row above the stops; Radial:
                // the stops alone.
                FillType::LinearGradient | FillType::RadialGradient | FillType::AngularGradient
                    if is_primary =>
                {
                    let mut stop_y = y + SECTION_HEADER_HEIGHT;
                    if fill_type.has_angle() {
                        rects.push((
                            PropertyFocus::GradientAngle,
                            Rect {
                                origin: Point2D::new(x0 + PAD_X, y),
                                size: Point2D::new(usable_w, INPUT_HEIGHT),
                            },
                        ));
                        stop_y += INPUT_HEIGHT + 6.0;
                    }
                    push_gradient_stop_rects(
                        &mut rects,
                        x0,
                        &mut stop_y,
                        usable_w,
                        visible.gradient_stop_count,
                    );
                }
                // Solid edits on the row itself; the mesh vertex grid opens
                // the colour picker instead of focusing an input.
                FillType::Solid
                | FillType::LinearGradient
                | FillType::RadialGradient
                | FillType::AngularGradient
                | FillType::MeshGradient
                | FillType::Shader
                | FillType::Image => {}
            }
        }
    }
    if visible.stroke {
        let y = tops.stroke + SECTION_HEADER_HEIGHT;
        let show_var = visible.color_variable_count > 0 || visible.stroke_variable_bound;
        let variable_w = if show_var {
            COLOR_VARIABLE_BUTTON_W + COLOR_VARIABLE_GAP
        } else {
            0.0
        };
        // Inline width only in Single mode (Axis/Individual use the per-side
        // grid); must match paint_stroke_main_row so the hit rects don't
        // linger where the width is no longer painted.
        let inline = visible.stroke_edit_mode == de_editor_core::PaddingEditMode::Single;
        let stroke_width_w = if inline { 60.0 } else { 0.0 };
        let stroke_width_gap = if inline { 8.0 } else { 0.0 };
        let stroke_hex_w = usable_w - stroke_width_w - stroke_width_gap - variable_w;
        if !visible.stroke_variable_bound {
            rects.push((
                PropertyFocus::StrokeHex,
                Rect {
                    origin: Point2D::new(x0 + PAD_X, y),
                    size: Point2D::new(stroke_hex_w, INPUT_HEIGHT),
                },
            ));
        }
        if inline {
            rects.push((
                PropertyFocus::StrokeWidth,
                Rect {
                    origin: Point2D::new(x0 + PAD_X + stroke_hex_w + variable_w + 8.0, y),
                    size: Point2D::new(stroke_width_w, INPUT_HEIGHT),
                },
            ));
        }
        rects.extend(stroke_side_input_rects(x0, y, w, visible.stroke_edit_mode));
    }
    if visible.effects {
        let mut y = tops.effects + SECTION_HEADER_HEIGHT;
        for (index, _) in effects.iter().enumerate() {
            let row = crate::widgets::property_panel_effects::effect_row_rects(x0, y, w);
            rects.push((PropertyFocus::EffectRadius(index), row.value));
            y += crate::widgets::property_panel_effects::EFFECT_ROW_HEIGHT
                + crate::widgets::property_panel_effects::EFFECT_ROW_GAP;
        }
    }
    rects
}

/// Push the Fill section's action rects — header "+" (and fill-rule
/// segments), each row's swatch / colour-variable / `⋯` targets, the
/// primary fill's gradient-stop or image affordances, and the open
/// `⋯` menu's rows — for the section starting at `y`. Split out of
/// `action_button_rects_with_fill_picker` for the 800-line cap.
pub(crate) fn push_fill_action_rects(
    out: &mut Vec<(PropertyPanelAction, Rect)>,
    panel_rect: Rect,
    visible: VisibleSections,
    fills: &[FillSummary],
    y: f32,
) {
    let x0 = panel_rect.origin.x;
    let w = panel_rect.size.x;
    let usable_w = w - PAD_X * 2.0;
    // Header "+" appends a fill (TS-style stacked list).
    out.push((
        PropertyPanelAction::AddFill,
        crate::widgets::property_panel_inputs::section_add_target(x0, y, w),
    ));
    if visible.path_fill_rule.is_some() {
        for (rule, rect) in crate::widgets::property_panel_fill::fill_rule_segment_rects(x0, y, w) {
            out.push((PropertyPanelAction::SetFillRule(rule), rect));
        }
    }
    let primary_stop_count = visible.gradient_stop_count;
    // One row per fill — the same row tops paint steps through.
    let mut menu = None;
    for (fi, fill, row_y) in fill_row_tops(fills, y + SECTION_HEADER_HEIGHT, primary_stop_count) {
        let is_primary = fi == 0;
        let fill_type = fill.fill_type;
        let row = fill_row_rects(x0, row_y, w, visible.fill_row_shows_variable(fi));
        // The swatch opens the colour picker for a solid fill (the hex
        // focus hit-test runs after this, so a swatch click opens the
        // picker instead of focusing). The primary fill's picker drives
        // the colour-variable subsystem; other fills bind their own.
        if fill_type == FillType::Solid && visible.fill_hex_editable(fi) {
            let open = if is_primary {
                PropertyPanelAction::OpenColorPicker(de_editor_core::ColorTarget::Fill)
            } else {
                PropertyPanelAction::OpenFillColorPicker(fi)
            };
            out.push((open, row.swatch_hit));
        }
        if let Some(rect) = row.variable {
            out.push((
                PropertyPanelAction::ToggleColorVariablePicker(de_editor_core::ColorTarget::Fill),
                rect,
            ));
        }
        out.push((PropertyPanelAction::ToggleFillMenu(fi), row.menu));
        if visible.fill_menu == Some(fi) {
            menu = Some((fi, row.menu));
        }
        let y = row_y + INPUT_HEIGHT + 6.0; // the body, under the row
        match fill_type {
            // Gradient / image bodies key off the primary fill, so only
            // the primary fill emits their action rects (other fills are
            // retyped from their `⋯` menu).
            FillType::Image if is_primary => {
                out.push((
                    PropertyPanelAction::ToggleImageFillPopover,
                    Rect {
                        origin: Point2D::new(x0 + PAD_X, y),
                        size: Point2D::new(usable_w, INPUT_HEIGHT),
                    },
                ));
            }
            FillType::LinearGradient | FillType::RadialGradient | FillType::AngularGradient
                if is_primary =>
            {
                let mut stop_y = y;
                if fill_type.has_angle() {
                    stop_y += INPUT_HEIGHT + 6.0; // Angle row above the stops header.
                }
                out.push((
                    PropertyPanelAction::AddGradientStop,
                    crate::widgets::property_panel_inputs::section_add_target(x0, stop_y, w),
                ));
                stop_y += SECTION_HEADER_HEIGHT; // 色标 header
                for index in 0..visible.gradient_stop_count {
                    out.push((
                        PropertyPanelAction::OpenColorPicker(
                            de_editor_core::ColorTarget::GradientStop(index),
                        ),
                        Rect {
                            origin: Point2D::new(x0 + PAD_X, stop_y),
                            size: Point2D::new(
                                crate::widgets::property_panel_inputs::color_swatch_action_width(),
                                INPUT_HEIGHT,
                            ),
                        },
                    ));
                    if visible.gradient_stop_count > 2 {
                        out.push((
                            PropertyPanelAction::RemoveGradientStop(index),
                            Rect {
                                origin: Point2D::new(x0 + w - PAD_X - 22.0, stop_y),
                                size: Point2D::new(28.0, INPUT_HEIGHT),
                            },
                        ));
                    }
                    stop_y += INPUT_HEIGHT + 4.0;
                }
            }
            FillType::MeshGradient | FillType::Shader if is_primary => {
                crate::widgets::property_panel_fill_mesh_shader::push_mesh_shader_action_rects(
                    out,
                    fill_type,
                    x0,
                    y,
                    w,
                    visible.gradient_stop_count,
                    visible.shader_custom,
                );
            }
            // Solid acts on the row itself; a non-primary fill has no body.
            FillType::Solid
            | FillType::LinearGradient
            | FillType::RadialGradient
            | FillType::AngularGradient
            | FillType::MeshGradient
            | FillType::Shader
            | FillType::Image => {}
        }
    }
    // The open `⋯` menu overlays the rows below it, so its targets go
    // after every row; `hit_test_action` also checks them first.
    if let Some((index, button)) = menu {
        crate::widgets::property_panel_fill_menu::push_fill_menu_action_rects(
            out,
            index,
            fills.len(),
            button,
            w,
        );
    }
}
