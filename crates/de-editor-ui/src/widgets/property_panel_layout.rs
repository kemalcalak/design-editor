//! Layout walkers for the property panel — pure-math helpers that
//! emit the on-screen rect of every editable input and every
//! button/checkbox the panel paints, so hit-tests stay aligned
//! with paint at all section-visibility / fill-type combinations.
//!
//! Pulled out of `property_panel_sections.rs` to keep that file
//! under the 800-line ceiling.

use crate::widgets::property_panel::{EffectSummary, FillSummary, PropertyPanelAction};
use crate::widgets::property_panel_fill::fill_row_height;
use crate::widgets::property_panel_inputs::{
    COLOR_VARIABLE_BUTTON_W, COLOR_VARIABLE_GAP, INPUT_HEIGHT, PAD_X, SECTION_HEADER_HEIGHT,
};
use crate::widgets::property_panel_interactions::{
    push_interaction_action_rects, InteractionSummary,
};
use crate::widgets::property_panel_stroke::push_stroke_action_rects;
use crate::{Point2D, Rect};
use de_editor_core::FillType;

pub(crate) use crate::widgets::property_panel_visibility::SectionCapabilities;
pub use crate::widgets::property_panel_visibility::{ComponentButtonState, VisibleSections};

pub use crate::widgets::property_panel_input_layout::editable_input_rects;

pub use crate::widgets::property_panel_effects::{EFFECT_ROW_GAP, EFFECT_ROW_HEIGHT};
pub const COLOR_VARIABLE_MENU_W: f32 = 210.0;
pub const COLOR_VARIABLE_MENU_ROW_H: f32 = 32.0;
pub const COLOR_VARIABLE_MENU_PAD_Y: f32 = 6.0;

/// Total vertical space the Effects section consumes: the section
/// header plus one block per effect (or an 8 px filler when the node
/// has none). Paint (`paint_effects_section`) and the action-rect
/// walker both consult this so their y-math can never drift.
pub fn effects_section_height(effects: &[EffectSummary]) -> f32 {
    SECTION_HEADER_HEIGHT
        + if effects.is_empty() {
            8.0
        } else {
            effects
                .iter()
                .map(crate::widgets::property_panel_effects::effect_block_height)
                .sum::<f32>()
        }
}

/// Total vertical space the Fill section consumes: the section header
/// plus one row per fill (the row + any body), plus the 12 px trailing
/// gap painted before the divider. `primary_stop_count` sizes the
/// primary fill's gradient body (the gradient-stop editor keys off the
/// primary fill). Mirrors the y-walk in `paint_fill_section`.
pub fn fills_section_height(fills: &[FillSummary], primary_stop_count: usize) -> f32 {
    let rows: f32 = fills
        .iter()
        .enumerate()
        .map(|(i, f)| fill_row_height(f.fill_type, i == 0, primary_stop_count))
        .sum();
    SECTION_HEADER_HEIGHT + rows + 12.0
}

/// State for the 5 Size checkboxes (fill / hug / clip).
#[derive(Debug, Clone, Copy)]
pub struct SizeFlags {
    pub fill_width: bool,
    pub fill_height: bool,
    pub hug_width: bool,
    pub hug_height: bool,
    pub clip_content: bool,
}

/// Height of the body that follows a fill's row in the Fill
/// section, per fill type. Used by every layout walker so the
/// y-offset of sections after Fill stays aligned with paint when
/// the user flips fill types.
pub fn fill_body_height(fill_type: FillType) -> f32 {
    // Legacy 2-stop assumption — kept for callers that don't yet
    // thread `stop_count`. Prefer `fill_body_height_with_stops`.
    fill_body_height_with_stops(fill_type, 2)
}

/// Stop-aware variant — accounts for the actual number of gradient
/// stops so adding / removing a stop reflows the panel correctly.
pub fn fill_body_height_with_stops(fill_type: FillType, stop_count: usize) -> f32 {
    let stops = stop_count.max(0);
    let stops_block = SECTION_HEADER_HEIGHT
        + stops as f32 * (INPUT_HEIGHT + 4.0)
        + if stops == 0 { 0.0 } else { 2.0 };
    match fill_type {
        // The hex lives on the fill row itself (#83).
        FillType::Solid => 0.0,
        // The stops, under an Angle row (Angular: start rotation) when the
        // type has one — the same `has_angle` the paint and walkers read.
        FillType::LinearGradient | FillType::RadialGradient | FillType::AngularGradient => {
            let angle_row = if fill_type.has_angle() {
                INPUT_HEIGHT + 6.0
            } else {
                0.0
            };
            angle_row + stops_block + 6.0
        }
        // Mesh: the Rows / Columns steppers plus the vertex grid
        // (`stop_count` = vertices, #142); Shader: the preset row (#90).
        FillType::MeshGradient => {
            crate::widgets::property_panel_fill_mesh_shader::mesh_body_height(stop_count)
        }
        // Shader: `stop_count` = its user uniform rows (#164).
        FillType::Shader => {
            crate::widgets::property_panel_fill_mesh_shader::shader_body_height(stop_count)
        }
        FillType::Image => INPUT_HEIGHT + 6.0,
    }
}

/// The Layer section's optional second row: the ellipse arc controls or
/// the star inner radius (the two kinds never coincide).
fn layer_second_row_height(visible: VisibleSections) -> f32 {
    debug_assert!(!(visible.ellipse_arc && visible.star_inner));
    if visible.ellipse_arc || visible.star_inner {
        INPUT_HEIGHT + 6.0
    } else {
        0.0
    }
}

/// Height consumed by the Layer section. Polygon adds a same-row
/// side-count field; ellipse adds a second row for arc controls and a
/// star one for its inner radius.
pub fn layer_section_height(visible: VisibleSections) -> f32 {
    if !visible.opacity {
        return 0.0;
    }
    let extra_second_row = layer_second_row_height(visible);
    let extra_compositing_row = if visible.compositing {
        crate::widgets::property_panel_compositing::COMPOSITING_ROW_HEIGHT
    } else {
        0.0
    };
    SECTION_HEADER_HEIGHT + INPUT_HEIGHT + extra_second_row + extra_compositing_row + 12.0
}

/// Rects of every clickable button / checkbox in the panel —
/// flex-layout 3 buttons, size-options 5 checkboxes. Same y-walk
/// math as `editable_input_rects` so paint + hit-test stay in
/// sync regardless of which sections are filtered.
pub fn action_button_rects(
    panel_rect: Rect,
    visible: VisibleSections,
    effects: &[EffectSummary],
    fills: &[FillSummary],
    interactions: &InteractionSummary,
) -> Vec<(PropertyPanelAction, Rect)> {
    action_button_rects_with_fill_picker(
        panel_rect,
        visible,
        effects,
        fills,
        interactions,
        false,
        0,
        false,
        false,
        false,
        false,
        false,
    )
}

/// The shared action-walker rect for the fill type dropdown at
/// `index`. Open fill-type overlays must anchor to this rect so
/// paint, hit-testing, and popup row actions stay in lockstep.
pub fn fill_type_toggle_action_rect(
    panel_rect: Rect,
    visible: VisibleSections,
    effects: &[EffectSummary],
    fills: &[FillSummary],
    index: usize,
) -> Option<Rect> {
    action_button_rects(
        panel_rect,
        visible,
        effects,
        fills,
        &InteractionSummary::default(),
    )
    .into_iter()
    .find_map(|(action, rect)| {
        matches!(action, PropertyPanelAction::ToggleFillTypePicker(i) if i == index).then_some(rect)
    })
}

/// Height of one row in an Export-section inline select popup.
pub const EXPORT_PICKER_ROW_H: f32 = 30.0;

/// Total height (px) of the PropertyPanel's section content. The
/// scroll clamp uses it so the inspector cannot scroll past its
/// end. Computed as the furthest bottom edge across every action
/// rect + every editable-input rect (so it stays correct whichever
/// section happens to be last), plus a small trailing margin.
pub fn property_panel_content_height(
    panel_rect: Rect,
    visible: VisibleSections,
    effects: &[EffectSummary],
    fills: &[FillSummary],
    interactions: &InteractionSummary,
) -> f32 {
    let actions = action_button_rects_with_fill_picker(
        panel_rect,
        visible,
        effects,
        fills,
        interactions,
        false,
        0,
        false,
        false,
        false,
        false,
        false,
    );
    let mut inputs = editable_input_rects(panel_rect, visible, fills, effects);
    inputs.extend(
        crate::widgets::property_panel_motion::motion_input_rects_in_panel(
            panel_rect,
            visible,
            effects,
            fills,
            interactions,
        ),
    );
    let bottom = actions
        .iter()
        .map(|(_, r)| r.origin.y + r.size.y)
        .chain(inputs.iter().map(|(_, r)| r.origin.y + r.size.y))
        .fold(panel_rect.origin.y, f32::max);
    (bottom - panel_rect.origin.y) + 16.0
}

/// Same as `action_button_rects` but the picker-open flags add
/// hit-rects for popup rows that overlay later sections. Fill-type
/// rows use their dedicated viewport-aware overlay hit-test; the two
/// `export_*_picker_open` flags emit the Export section's inline
/// scale (3 rows) / format (5 rows) select popups. `effects` drives
/// the Effects section's per-effect "✕" and parameter-stepper rects
/// + that section's variable height.
#[allow(clippy::too_many_arguments)]
pub fn action_button_rects_with_fill_picker(
    panel_rect: Rect,
    visible: VisibleSections,
    effects: &[EffectSummary],
    fills: &[FillSummary],
    interactions: &InteractionSummary,
    _fill_picker_open: bool,
    _fill_type_picker_index: usize,
    font_picker_open: bool,
    font_weight_picker_open: bool,
    export_scale_picker_open: bool,
    export_format_picker_open: bool,
    layout_align_popover_open: bool,
) -> Vec<(PropertyPanelAction, Rect)> {
    let x0 = panel_rect.origin.x;
    let w = panel_rect.size.x;
    let usable_w = w - PAD_X * 2.0;
    let half_w = (usable_w - 8.0) / 2.0;
    let mut out: Vec<(PropertyPanelAction, Rect)> = Vec::new();

    // Every section starts at its shared top (#83), so this walker, the
    // input walker and paint cannot drift apart.
    let tops = crate::widgets::property_panel_section_tops::section_tops(
        panel_rect.origin.y,
        visible,
        effects,
        fills,
        interactions,
    );
    if visible.create_component {
        out.extend(crate::widgets::property_panel_instance::action_rects(
            x0,
            tops.component,
            w,
            visible.component_button,
        ));
    }
    // Position section: the header's flip buttons, X Y, then W H (#30),
    // then rotation / radius.
    crate::widgets::property_panel_flip::push_flip_action_rects(&mut out, x0, tops.position, w);
    let radius_row_y =
        tops.position + SECTION_HEADER_HEIGHT + INPUT_HEIGHT + 6.0 + visible.position_size_row_h();
    if visible.corner_per_corner {
        out.push((
            PropertyPanelAction::ToggleCornerExpand,
            crate::widgets::property_panel_corner::uniform_and_toggle_rects(x0, radius_row_y, w).1,
        ));
    }

    if visible.flex_layout {
        crate::widgets::property_panel_flex::push_flex_action_rects(
            &mut out,
            x0,
            tops.flex + SECTION_HEADER_HEIGHT,
            w,
            visible.flex_layout_mode,
            visible.layout_justify,
            layout_align_popover_open,
        );
    }

    if visible.size_options {
        let mut y = tops.size + visible.size_section_lead_h();
        let row_h = crate::widgets::property_panel_inputs::size_check_row_height();
        out.push((
            PropertyPanelAction::ToggleSizeFillWidth,
            Rect {
                origin: Point2D::new(x0 + PAD_X, y),
                size: Point2D::new(half_w, row_h),
            },
        ));
        out.push((
            PropertyPanelAction::ToggleSizeFillHeight,
            Rect {
                origin: Point2D::new(x0 + PAD_X + half_w + 8.0, y),
                size: Point2D::new(half_w, row_h),
            },
        ));
        y += row_h;
        out.push((
            PropertyPanelAction::ToggleSizeHugWidth,
            Rect {
                origin: Point2D::new(x0 + PAD_X, y),
                size: Point2D::new(half_w, row_h),
            },
        ));
        out.push((
            PropertyPanelAction::ToggleSizeHugHeight,
            Rect {
                origin: Point2D::new(x0 + PAD_X + half_w + 8.0, y),
                size: Point2D::new(half_w, row_h),
            },
        ));
        y += row_h;
        if visible.clip_content {
            out.push((
                PropertyPanelAction::ToggleSizeClipContent,
                Rect {
                    origin: Point2D::new(x0 + PAD_X, y),
                    size: Point2D::new(usable_w, row_h),
                },
            ));
        }
    }

    if visible.icon {
        crate::widgets::property_panel_icon::push_icon_action_rects(&mut out, x0, tops.icon, w);
    }

    if visible.text {
        let y = tops.text;
        out.extend(crate::widgets::property_panel_text::text_action_rects(
            x0, y, usable_w,
        ));
        // The font-family picker is an overlay now (searchable list,
        // `property_panel_typography.rs`) — its rows are hit-tested
        // BEFORE this walker, not emitted from it.
        let _ = font_picker_open;
        if font_weight_picker_open {
            out.extend(
                crate::widgets::property_panel_text::font_weight_picker_action_rects(
                    x0, y, usable_w,
                ),
            );
        }
    }

    if let Some(kind) = visible.widget {
        crate::widgets::property_panel_widget::push_widget_action_rects(
            &mut out,
            kind,
            visible.widget_checked,
            x0,
            tops.widget,
            usable_w,
        );
    }

    if visible.annotation.is_some() {
        crate::widgets::property_panel_annotation::push_annotation_action_rects(
            &mut out,
            x0,
            tops.annotation,
            usable_w,
        );
    }

    if let Some(rows) = visible.script {
        crate::widgets::property_panel_script::push_script_action_rects(
            &mut out,
            rows,
            x0,
            tops.script,
            usable_w,
        );
    }

    if let Some(pages_allowed) = visible.browser {
        crate::widgets::property_panel_browser::push_browser_action_rects(
            &mut out,
            x0,
            tops.browser,
            usable_w,
            pages_allowed,
        );
    }

    if visible.image {
        crate::widgets::property_panel_image_node::push_image_action_rects_with_video(
            &mut out,
            x0,
            tops.image,
            w,
            visible.image_warning,
            visible.video,
        );
    }
    if visible.video {
        crate::widgets::property_panel_video::push_video_action_rects(&mut out, x0, tops.video, w);
    }

    if visible.opacity && visible.compositing {
        let mut compositing_y = tops.layer + SECTION_HEADER_HEIGHT + INPUT_HEIGHT;
        compositing_y += layer_second_row_height(visible);
        compositing_y += crate::widgets::property_panel_compositing::COMPOSITING_ROW_GAP;
        for (target, rect) in
            crate::widgets::property_panel_compositing::node_trigger_rects(x0, compositing_y, w)
        {
            out.push((PropertyPanelAction::ToggleCompositingPicker(target), rect));
        }
    }
    if visible.fill {
        crate::widgets::property_panel_input_layout::push_fill_action_rects(
            &mut out, panel_rect, visible, fills, tops.fill,
        );
    }
    if visible.stroke {
        // Mirrors paint_stroke_section: header + hex/width row + side grid.
        let section_y = tops.stroke;
        let mut y = section_y;
        push_stroke_action_rects(
            &mut out,
            x0,
            section_y,
            w,
            visible.stroke_edit_mode,
            visible.stroke_mode_popover_open,
            visible.stroke_style_rows,
        );
        y += SECTION_HEADER_HEIGHT;
        // The stroke hex row's leading colour swatch opens the picker.
        let show_var = visible.color_variable_count > 0 || visible.stroke_variable_bound;
        let variable_w = if show_var {
            COLOR_VARIABLE_BUTTON_W + COLOR_VARIABLE_GAP
        } else {
            0.0
        };
        // Inline width only in Single mode (matches paint_stroke_main_row);
        // in per-side mode the hex fills the row so the variable button +
        // colour-picker anchor stay aligned.
        let inline = visible.stroke_edit_mode == de_editor_core::PaddingEditMode::Single;
        let width_w = if inline { 60.0 } else { 0.0 };
        let width_gap = if inline { 8.0 } else { 0.0 };
        let hex_w = usable_w - width_w - width_gap - variable_w;
        if !visible.stroke_variable_bound {
            out.push((
                PropertyPanelAction::OpenColorPicker(de_editor_core::ColorTarget::Stroke),
                Rect {
                    origin: Point2D::new(x0 + PAD_X, y),
                    size: Point2D::new(
                        crate::widgets::property_panel_inputs::color_swatch_action_width(),
                        INPUT_HEIGHT,
                    ),
                },
            ));
        }
        if show_var {
            let var_rect = Rect {
                origin: Point2D::new(x0 + PAD_X + hex_w + COLOR_VARIABLE_GAP, y),
                size: Point2D::new(COLOR_VARIABLE_BUTTON_W, INPUT_HEIGHT),
            };
            out.push((
                PropertyPanelAction::ToggleColorVariablePicker(de_editor_core::ColorTarget::Stroke),
                var_rect,
            ));
        }
    }
    if visible.effects {
        let mut y = tops.effects;
        // Mirrors `paint_effects_section`: header + one block per
        // effect. The header's "+" opens the effect-kind add-menu.
        let plus = crate::widgets::property_panel_inputs::section_add_target(x0, y, w);
        out.push((PropertyPanelAction::ToggleEffectAddPicker, plus));
        y += SECTION_HEADER_HEIGHT;
        for (ei, eff) in effects.iter().enumerate() {
            let rects = crate::widgets::property_panel_effects::effect_row_rects(x0, y, w);
            out.push((PropertyPanelAction::RemoveEffect(ei), rects.remove));
            out.push((
                PropertyPanelAction::SetEffectVisible(ei, !eff.visible),
                rects.eye,
            ));
            if let Some(body) = eff.shader.as_ref() {
                crate::widgets::property_panel_shader_body::push_action_rects(
                    &mut out,
                    de_editor_core::ShaderTarget::Effect(ei),
                    body,
                    x0,
                    crate::widgets::property_panel_effects::effect_body_y(y),
                    w,
                );
            }
            if !eff.kind.has_amount() {
                y += crate::widgets::property_panel_effects::effect_block_height(eff);
                continue;
            }
            let field = if matches!(
                eff.kind,
                crate::widgets::property_panel::EffectKind::Shadow
                    | crate::widgets::property_panel::EffectKind::InnerShadow
            ) {
                de_editor_core::EffectField::Blur
            } else {
                de_editor_core::EffectField::Radius
            };
            out.push((
                PropertyPanelAction::AdjustEffectParam {
                    effect: ei,
                    field,
                    new_value: eff.blur,
                },
                rects.slider,
            ));
            y += crate::widgets::property_panel_effects::effect_block_height(eff);
        }
    }
    if visible.interactions {
        push_interaction_action_rects(&mut out, interactions, x0, tops.interactions, w);
    }
    if visible.export {
        // The header's "+" / "−" expands or collapses the section (#83).
        out.push((
            PropertyPanelAction::ToggleExportSection,
            crate::widgets::property_panel_inputs::section_add_target(x0, tops.export, w),
        ));
    }
    if visible.export && visible.export_open {
        let mut y = tops.export;
        // Export section: header + the dropdown row (scale on the
        // left for raster formats, format on the right or across the
        // row) from `export_row_rects`, the geometry
        // `paint_export_section` paints. Each dropdown is its own toggle
        // rect; when its inline select popup is open the option rows
        // are emitted too. `hit_test_action` walks the result in
        // `rev()`, so a popup-row hit is tested before its toggle.
        y += SECTION_HEADER_HEIGHT;
        let (scale_rect, format_rect) = crate::widgets::property_panel_export::export_row_rects(
            x0,
            y,
            w,
            visible.export_scale_row,
        );
        if let Some(scale_rect) = scale_rect {
            out.push((PropertyPanelAction::ToggleExportScalePicker, scale_rect));
        }
        out.push((PropertyPanelAction::ToggleExportFormatPicker, format_rect));
        y += INPUT_HEIGHT + 12.0;
        // Full-width Export action button below the dropdown row.
        out.push((
            PropertyPanelAction::ExportImageNow,
            Rect {
                origin: Point2D::new(x0 + PAD_X, y),
                size: Point2D::new(usable_w, INPUT_HEIGHT),
            },
        ));
        // The Export section is the last section, pinned to the
        // bottom of the panel — so its select popups open UPWARD,
        // stacking their option rows directly above the dropdown.
        // Opening downward would clip the rows at the panel edge.
        // `paint_select_popup` derives the popup background from the
        // first / last row rect, so it follows whatever rows emit
        // here. `first_row_y` is placed so the background's bottom
        // edge lands `4 px` above the dropdown (matches the `6 px`
        // top/bottom padding `paint_select_popup` adds).
        if let Some(scale_rect) = scale_rect.filter(|_| export_scale_picker_open) {
            let count = 3.0;
            let first_row_y = scale_rect.origin.y - 4.0 - 6.0 - count * EXPORT_PICKER_ROW_H;
            for (i, scale) in [1.0_f32, 2.0, 3.0].into_iter().enumerate() {
                out.push((
                    PropertyPanelAction::SetExportScale(scale),
                    Rect {
                        origin: Point2D::new(
                            scale_rect.origin.x,
                            first_row_y + i as f32 * EXPORT_PICKER_ROW_H,
                        ),
                        size: Point2D::new(scale_rect.size.x, EXPORT_PICKER_ROW_H),
                    },
                ));
            }
        }
        if export_format_picker_open {
            // Every format the Export dialog offers (#224).
            let formats = de_editor_core::ExportFormat::ALL;
            let count = formats.len() as f32;
            let first_row_y = format_rect.origin.y - 4.0 - 6.0 - count * EXPORT_PICKER_ROW_H;
            for (i, fmt) in formats.into_iter().enumerate() {
                out.push((
                    PropertyPanelAction::SetExportFormat(fmt),
                    Rect {
                        origin: Point2D::new(
                            format_rect.origin.x,
                            first_row_y + i as f32 * EXPORT_PICKER_ROW_H,
                        ),
                        size: Point2D::new(format_rect.size.x, EXPORT_PICKER_ROW_H),
                    },
                ));
            }
        }
    }
    // The closing Code section (#30). Export's popups open upward, so
    // they never cover these chips.
    for (fw, rect) in crate::widgets::property_panel_code_chips::chip_rects(x0, tops.code, w) {
        out.push((
            PropertyPanelAction::Codegen(
                crate::widgets::property_panel_action::CodegenAction::OpenInCodeView(fw),
            ),
            rect,
        ));
    }

    out
}
