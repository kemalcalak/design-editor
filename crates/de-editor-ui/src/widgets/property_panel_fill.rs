//! Fill-section paint code split out of
//! `property_panel_sections.rs` to honour the 800-line ceiling.
//! Contains: `fill_type_label`, `paint_fill_type_picker`,
//! `paint_fill_section`, the multi-fill loop (`paint_one_fill`) and the
//! per-fill height helpers. The per-type body paints live in the
//! sibling `property_panel_fill_body` module.

use crate::theme::Theme;
use crate::widgets::icons::{draw_icon, Icon};
use crate::widgets::property_panel::NodeSnapshot;
// Per-type body paints live in the sibling `property_panel_fill_body`
// module (split off for the 800-line cap). `stop_hex_rgb_only` is
// re-exported so existing `property_panel_fill::stop_hex_rgb_only`
// paths keep resolving.
use crate::widgets::property_panel_color_variables::paint_color_variable_button;
use crate::widgets::property_panel_fill_body::paint_fill_gradient_body;
pub use crate::widgets::property_panel_fill_body::stop_hex_rgb_only;
use crate::widgets::property_panel_fill_image_body::paint_fill_image_body;
pub use crate::widgets::property_panel_fill_picker::{
    fill_type_at, fill_type_picker_hit, fill_type_picker_rect, fill_type_picker_viewport,
};
use crate::widgets::property_panel_fill_picker::{popup_anchor, tokens_from_theme, FILL_TYPES};
use crate::widgets::property_panel_inputs::{
    paint_section_divider, paint_section_label_with_add, COLOR_VARIABLE_BUTTON_W, INPUT_HEIGHT,
    INPUT_RADIUS, PAD_X, SECTION_GAP,
};
use crate::widgets::property_panel_layout::fill_body_height_with_stops;
use crate::widgets::property_panel_sections::{EditContext, PropertyLabels};
use crate::widgets::text_metrics;
use crate::widgets::PaintCx;
use crate::{Color, Point2D, Rect, TextLayout};
use de_editor_core::PropertyFocus;
use jian_ops_schema::node::path::PathFillRule;
pub use jian_widgets::components::select::SelectHit;
use jian_widgets::components::select::{Select, SelectItem, SelectState};

const FILL_SWATCH_SIZE: f32 = 16.0;
/// Width of the leading swatch's hit strip (opens the colour picker).
const FILL_SWATCH_HIT_W: f32 = 28.0;
const FILL_VALUE_X: f32 = 30.0;
const FILL_OPACITY_WIDTH: f32 = 48.0;
const FILL_MENU_WIDTH: f32 = 24.0;
const FILL_ROW_GAP: f32 = 4.0;

/// Shared single-row geometry (#83) used by paint and every input /
/// action walker: one field holding swatch, value (mono hex or the
/// fill type) and opacity, then the optional colour-variable button
/// and the trailing `⋯` menu button.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct FillRowRects {
    pub(crate) frame: Rect,
    pub(crate) swatch: Rect,
    pub(crate) swatch_hit: Rect,
    pub(crate) value: Rect,
    pub(crate) opacity: Rect,
    pub(crate) variable: Option<Rect>,
    pub(crate) menu: Rect,
}

pub(crate) fn fill_row_rects(x: f32, y: f32, width: f32, show_variable: bool) -> FillRowRects {
    let right = x + width - PAD_X;
    let menu = Rect::xywh(right - FILL_MENU_WIDTH, y, FILL_MENU_WIDTH, INPUT_HEIGHT);
    let variable = show_variable.then(|| {
        Rect::xywh(
            menu.origin.x - FILL_ROW_GAP - COLOR_VARIABLE_BUTTON_W,
            y,
            COLOR_VARIABLE_BUTTON_W,
            INPUT_HEIGHT,
        )
    });
    let field_right = variable.map_or(menu.origin.x, |v| v.origin.x) - FILL_ROW_GAP;
    let field_x = x + PAD_X;
    let field = Rect::xywh(field_x, y, (field_right - field_x).max(0.0), INPUT_HEIGHT);
    let opacity = Rect::xywh(
        field_right - FILL_OPACITY_WIDTH,
        y,
        FILL_OPACITY_WIDTH,
        INPUT_HEIGHT,
    );
    let value_x = field_x + FILL_VALUE_X;
    FillRowRects {
        frame: field,
        swatch: Rect::xywh(
            field_x + 6.0,
            y + (INPUT_HEIGHT - FILL_SWATCH_SIZE) / 2.0,
            FILL_SWATCH_SIZE,
            FILL_SWATCH_SIZE,
        ),
        swatch_hit: Rect::xywh(field_x, y, FILL_SWATCH_HIT_W, INPUT_HEIGHT),
        value: Rect::xywh(
            value_x,
            y,
            (opacity.origin.x - value_x).max(0.0),
            INPUT_HEIGHT,
        ),
        opacity,
        variable,
        menu,
    }
}

/// Display label for a fill-type variant (Solid / Linear / Radial /
/// Mesh / Shader / Image), localised against `locale` via the `fill.*`
/// keys.
pub fn fill_type_label(
    locale: de_editor_core::Locale,
    t: de_editor_core::FillType,
) -> &'static str {
    use de_editor_core::FillType;
    let key = match t {
        FillType::Solid => "fill.solid",
        FillType::LinearGradient => "fill.linear",
        FillType::RadialGradient => "fill.radial",
        FillType::AngularGradient => "fill.angular",
        FillType::MeshGradient => "fill.mesh",
        FillType::Shader => "fill.shader",
        FillType::Image => "fill.image",
    };
    de_i18n::translate(locale, key)
}

/// Paint the fill-type picker overlay (one row per `FILL_TYPES`). Called by
/// `PropertyPanel::paint` AFTER all other sections have painted
/// so the dropdown overlays them.
#[allow(clippy::too_many_arguments)]
pub fn paint_fill_type_picker(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    action_rect: Rect,
    viewport: Rect,
    state: &SelectState,
    active: de_editor_core::FillType,
    locale: de_editor_core::Locale,
) {
    let picker_rect = fill_type_picker_rect(action_rect, viewport);
    let items: Vec<SelectItem<'static>> = FILL_TYPES
        .iter()
        .map(|t| SelectItem {
            label: fill_type_label(locale, *t),
            selected: *t == active,
            disabled: false,
        })
        .collect();
    let select = Select {
        state,
        items: &items,
    };
    select.paint(
        cx.backend,
        popup_anchor(picker_rect),
        picker_rect,
        &tokens_from_theme(theme),
    );
}
// ── Fill section ──────────────────────────────────────────────────

/// Height of the body under fill `index`'s row. Only the primary fill
/// (index 0) paints a gradient-stop or image editor (they key off the
/// primary fill); every other fill, and a Solid fill at any index
/// (its hex lives on the row, #83), is the row alone.
pub fn fill_row_body_height(
    fill_type: de_editor_core::FillType,
    is_primary: bool,
    primary_stop_count: usize,
) -> f32 {
    use de_editor_core::FillType;
    match fill_type {
        // The hex lives on the row itself (#83), so Solid has no body.
        FillType::Solid => 0.0,
        // Gradients, mesh (vertex grid), shader (preset row) and image
        // bodies all key off the primary fill.
        FillType::LinearGradient
        | FillType::RadialGradient
        | FillType::AngularGradient
        | FillType::MeshGradient
        | FillType::Shader
        | FillType::Image => {
            if is_primary {
                fill_body_height_with_stops(fill_type, primary_stop_count)
            } else {
                0.0
            }
        }
    }
}

/// Total height (px) one fill consumes — its row plus its body.
/// Mirrors the y-walk in `paint_one_fill`.
pub fn fill_row_height(
    fill_type: de_editor_core::FillType,
    is_primary: bool,
    primary_stop_count: usize,
) -> f32 {
    INPUT_HEIGHT + 6.0 + fill_row_body_height(fill_type, is_primary, primary_stop_count)
}

/// Each fill with the top y of its row, starting at `first_row_y` (the
/// row under the section header). Paint and both walkers step through
/// the fills with this, so a fill's body height moves every later row
/// identically in all three.
pub(crate) fn fill_row_tops<'a>(
    fills: &'a [crate::widgets::property_panel_snapshot::FillSummary],
    first_row_y: f32,
    primary_stop_count: usize,
) -> impl Iterator<
    Item = (
        usize,
        &'a crate::widgets::property_panel_snapshot::FillSummary,
        f32,
    ),
> + 'a {
    let mut y = first_row_y;
    fills.iter().enumerate().map(move |(index, fill)| {
        let row_y = y;
        y += fill_row_height(fill.fill_type, index == 0, primary_stop_count);
        (index, fill, row_y)
    })
}

/// Vertical offset (px) of fill `index`'s row relative to the first
/// fill's row — the sum of every prior fill's height. `0.0` for
/// `index == 0`.
pub fn fill_row_offset(
    fills: &[crate::widgets::property_panel_snapshot::FillSummary],
    index: usize,
    primary_stop_count: usize,
) -> f32 {
    fills
        .iter()
        .take(index)
        .enumerate()
        .map(|(i, f)| fill_row_height(f.fill_type, i == 0, primary_stop_count))
        .sum()
}

/// Paint-context + geometry args threaded through; a struct adds no gain.
#[allow(clippy::too_many_arguments)]
pub fn paint_fill_section(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    snapshot: &NodeSnapshot,
    edit: &EditContext<'_>,
    labels: &PropertyLabels,
    visible: crate::widgets::property_panel_layout::VisibleSections,
    locale: de_editor_core::Locale,
    x: f32,
    y: f32,
    width: f32,
) -> f32 {
    // Header row + "+" (appends a fill). One editable row per fill.
    let header_y = y;
    let mut y = paint_section_label_with_add(cx, theme, labels.fill, x, y, width);
    if let Some(rule) = snapshot.path_fill_rule {
        paint_fill_rule_control(cx, theme, labels, rule, x, header_y, width);
    }
    let primary_stop_count = snapshot.gradient_stops.len();
    for (index, fill, row_y) in fill_row_tops(&snapshot.fills, y, primary_stop_count) {
        y = paint_one_fill(
            cx,
            theme,
            snapshot,
            edit,
            index,
            fill,
            index == 0,
            // The same rules the input and action walkers apply, so the
            // painted chip / button match the click targets.
            fill.variable_ref
                .as_deref()
                .filter(|_| !visible.fill_hex_editable(index)),
            visible.fill_row_shows_variable(index),
            locale,
            x,
            row_y,
            width,
            primary_stop_count,
        );
    }
    y += 12.0;
    paint_section_divider(cx, theme, x, y, width);
    y + SECTION_GAP
}

pub fn fill_rule_segment_rects(x: f32, y: f32, width: f32) -> [(PathFillRule, Rect); 2] {
    let total_w = 112.0;
    let segment_w = total_w / 2.0;
    let left = x + width - PAD_X - 28.0 - 6.0 - total_w;
    [
        (
            PathFillRule::Nonzero,
            Rect::xywh(left, y - 2.0, segment_w, 28.0),
        ),
        (
            PathFillRule::Evenodd,
            Rect::xywh(left + segment_w, y - 2.0, segment_w, 28.0),
        ),
    ]
}

fn paint_fill_rule_control(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    labels: &PropertyLabels,
    active: PathFillRule,
    x: f32,
    y: f32,
    width: f32,
) {
    let segments = fill_rule_segment_rects(x, y, width);
    let outer = Rect::xywh(
        segments[0].1.origin.x,
        segments[0].1.origin.y,
        segments[0].1.size.x + segments[1].1.size.x,
        segments[0].1.size.y,
    );
    cx.backend.fill_round_rect(outer, 7.0, theme.muted);
    cx.backend.stroke_round_rect(outer, 7.0, theme.border, 1.0);
    for (rule, rect) in segments {
        if rule == active {
            cx.backend.fill_round_rect(rect, 6.0, theme.primary);
        }
        let label = match rule {
            PathFillRule::Nonzero => labels.fill_rule_nonzero,
            PathFillRule::Evenodd => labels.fill_rule_evenodd,
        };
        let text = TextLayout::single_run(
            label,
            crate::widgets::text_metrics::CHROME_FONT_FAMILY,
            11.0,
            if rule == active {
                theme.primary_foreground
            } else {
                theme.muted_foreground
            }
            .to_jian(),
            Point2D::new(0.0, 0.0),
        );
        let text_w = text_metrics::measure_chrome(cx.backend, label, 11.0);
        cx.backend.draw_text(
            &text,
            Point2D::new(
                rect.origin.x + (rect.size.x - text_w) / 2.0,
                rect.origin.y + 18.0,
            ),
        );
    }
}

/// Paint one fill row (#83): a single field — swatch, mono hex (or
/// the fill type for gradient / image fills), opacity — then the
/// colour-variable button and the `⋯` menu button, followed by the
/// primary fill's gradient / image body. Returns the y just past the
/// body. Geometry mirrors `fill_row_height` / the layout walkers.
#[allow(clippy::too_many_arguments)]
fn paint_one_fill(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    snapshot: &NodeSnapshot,
    edit: &EditContext<'_>,
    fill_index: usize,
    fill: &crate::widgets::property_panel_snapshot::FillSummary,
    is_primary: bool,
    fill_variable_ref: Option<&str>,
    show_variable_button: bool,
    locale: de_editor_core::Locale,
    x: f32,
    y: f32,
    width: f32,
    primary_stop_count: usize,
) -> f32 {
    use de_editor_core::FillType;
    let fill_type = fill.fill_type;
    let row = fill_row_rects(x, y, width, show_variable_button);
    let hex_focus = PropertyFocus::FillHex(fill_index);
    let opacity_focus = PropertyFocus::FillOpacity(fill_index);
    cx.backend
        .fill_round_rect(row.frame, INPUT_RADIUS, theme.muted);
    let field_focused = (edit.focus == Some(hex_focus) && fill_variable_ref.is_none())
        || edit.focus == Some(opacity_focus);
    if field_focused {
        cx.backend
            .stroke_round_rect(row.frame, INPUT_RADIUS, theme.primary, 1.5);
    }
    paint_fill_swatch(cx, theme, fill_type, fill.color, row.swatch);
    let baseline = row.frame.origin.y + crate::widgets::property_panel_inputs::INPUT_TEXT_BASELINE;
    match (fill_type, fill_variable_ref) {
        (FillType::Solid, Some(name)) => {
            paint_row_text(
                cx,
                theme,
                &format!("${name}"),
                text_metrics::CHROME_FONT_FAMILY,
                row.value,
                baseline,
            );
        }
        (FillType::Solid, None) => {
            // A canonical solid may carry alpha in its colour
            // (`#RRGGBBAA`) as well as in the body's opacity; surface all
            // eight digits so focusing and blurring cannot drop it.
            let hex_owned = crate::util::color_to_hex_with_alpha(fill.color);
            let hex_text = edit.value_for(hex_focus, &hex_owned);
            if !edit.paint_mono_input_view_at(cx, theme, hex_focus, row.value, 12.0, 0.0, baseline)
            {
                paint_row_text(
                    cx,
                    theme,
                    hex_text,
                    text_metrics::MONO_FONT_FAMILY,
                    row.value,
                    baseline,
                );
            }
        }
        _ => {
            paint_row_text(
                cx,
                theme,
                fill_type_label(locale, fill_type),
                text_metrics::CHROME_FONT_FAMILY,
                row.value,
                baseline,
            );
        }
    }
    paint_fill_opacity(
        cx,
        theme,
        edit,
        opacity_focus,
        fill.opacity,
        row.opacity,
        baseline,
    );
    if let Some(rect) = row.variable {
        paint_color_variable_button(cx, theme, rect, fill_variable_ref.is_some());
    }
    draw_icon(
        cx.backend,
        Icon::MoreHorizontal,
        Point2D::new(
            row.menu.origin.x + (row.menu.size.x - 14.0) / 2.0,
            row.menu.origin.y + (row.menu.size.y - 14.0) / 2.0,
        ),
        14.0,
        theme.muted_foreground,
        1.4,
    );
    let body_y = y + INPUT_HEIGHT + 6.0;
    // Every body keys off the primary fill (the gradient-stop, mesh,
    // shader-preset and image-fill editors operate on `fills[0]`), so
    // only the primary fill paints one.
    use crate::widgets::property_panel_fill_mesh_shader as mesh_shader;
    match fill_type {
        FillType::MeshGradient if is_primary => {
            mesh_shader::paint_fill_mesh_body(cx, theme, snapshot, x, body_y, width);
        }
        FillType::Shader if is_primary => {
            mesh_shader::paint_fill_shader_body(cx, theme, snapshot, locale, x, body_y, width);
        }
        FillType::LinearGradient | FillType::RadialGradient | FillType::AngularGradient
            if is_primary =>
        {
            let show_angle = fill_type.has_angle();
            paint_fill_gradient_body(
                cx, theme, edit, snapshot, locale, x, body_y, width, show_angle,
            );
        }
        FillType::Image if is_primary => {
            paint_fill_image_body(cx, theme, snapshot, locale, x, body_y, width);
        }
        _ => {}
    }
    body_y + fill_row_body_height(fill_type, is_primary, primary_stop_count)
}

/// One run of row text (hex, `$variable` or fill type), clipped to its
/// slot with a trailing ellipsis.
fn paint_row_text(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    text: &str,
    family: &str,
    slot: Rect,
    baseline: f32,
) {
    let fitted =
        text_metrics::fit_in_family(cx.backend, text, (slot.size.x - 6.0).max(0.0), 12.0, family);
    let layout = TextLayout::single_run(
        &fitted,
        family,
        12.0,
        (theme.foreground).to_jian(),
        Point2D::new(0.0, 0.0),
    );
    cx.backend
        .draw_text(&layout, Point2D::new(slot.origin.x, baseline));
}

/// The row's opacity readout / input: `100%`, with the live draft and
/// caret while focused.
#[allow(clippy::too_many_arguments)]
fn paint_fill_opacity(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    edit: &EditContext<'_>,
    focus: PropertyFocus,
    opacity: f32,
    rect: Rect,
    baseline: f32,
) {
    let owned = ((opacity * 100.0).round() as i32).to_string();
    let text = edit.value_for(focus, &owned);
    let text_x = rect.origin.x + 4.0;
    // `%` follows the number (`100%`), capped at the slot's right edge.
    let text_w =
        text_metrics::measure_in_family(cx.backend, text, 12.0, text_metrics::MONO_FONT_FAMILY);
    let unit_x = (text_x + text_w + 1.0).min(rect.origin.x + rect.size.x - 10.0);
    if !edit.paint_mono_input_view_at(
        cx,
        theme,
        focus,
        Rect::xywh(
            text_x,
            rect.origin.y,
            (unit_x - 2.0 - text_x).max(0.0),
            rect.size.y,
        ),
        12.0,
        0.0,
        baseline,
    ) {
        edit.paint_selection_at(cx, theme, focus, text, text_x, baseline, 12.0, unit_x);
        let layout = TextLayout::single_run(
            text,
            text_metrics::MONO_FONT_FAMILY,
            12.0,
            (theme.foreground).to_jian(),
            Point2D::new(0.0, 0.0),
        );
        cx.backend
            .draw_text(&layout, Point2D::new(text_x, baseline));
        if let Some(pos) = edit.caret_at(focus) {
            let w = text_metrics::measure_in_family(
                cx.backend,
                &text[..pos.min(text.len())],
                12.0,
                text_metrics::MONO_FONT_FAMILY,
            );
            cx.backend.fill_rect(
                Rect::xywh(text_x + w, rect.origin.y + 6.0, 1.5, rect.size.y - 12.0),
                theme.foreground,
            );
        }
    }
    let unit = TextLayout::single_run(
        "%",
        text_metrics::MONO_FONT_FAMILY,
        12.0,
        (theme.muted_foreground).to_jian(),
        Point2D::new(0.0, 0.0),
    );
    cx.backend.draw_text(&unit, Point2D::new(unit_x, baseline));
}

/// Swatch preview per fill type, so the row reads as a small preview
/// of what renders.
fn paint_fill_swatch(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    fill_type: de_editor_core::FillType,
    color: Color,
    swatch: Rect,
) {
    use de_editor_core::FillType;
    match fill_type {
        FillType::Solid | FillType::Shader => {
            // Shader fills preview their resolved fallback colour.
            cx.backend.fill_round_rect(swatch, 3.0, color);
        }
        FillType::LinearGradient => {
            // Two halves stand in for a black→white gradient.
            let half_w = swatch.size.x / 2.0;
            cx.backend.fill_round_rect(
                Rect::xywh(swatch.origin.x, swatch.origin.y, half_w, swatch.size.y),
                3.0,
                Color::BLACK,
            );
            cx.backend.fill_round_rect(
                Rect::xywh(
                    swatch.origin.x + half_w,
                    swatch.origin.y,
                    swatch.size.x - half_w,
                    swatch.size.y,
                ),
                3.0,
                Color::WHITE,
            );
        }
        FillType::RadialGradient => {
            cx.backend.fill_round_rect(swatch, 3.0, Color::BLACK);
            let inset = 3.0;
            cx.backend.fill_oval(
                Rect::xywh(
                    swatch.origin.x + inset,
                    swatch.origin.y + inset,
                    swatch.size.x - inset * 2.0,
                    swatch.size.y - inset * 2.0,
                ),
                Color::WHITE,
            );
        }
        FillType::AngularGradient => {
            // A black→white sweep (solid black on backends without one).
            cx.backend.fill_round_rect_angular_gradient(
                swatch,
                3.0,
                &[(0.0, Color::BLACK), (1.0, Color::WHITE)],
                0.5,
                0.5,
                0.0,
                1.0,
            );
        }
        FillType::MeshGradient => {
            // Four quadrants read as a corner mesh.
            let hw = swatch.size.x / 2.0;
            let hh = swatch.size.y / 2.0;
            let quad =
                |ox: f32, oy: f32| Rect::xywh(swatch.origin.x + ox, swatch.origin.y + oy, hw, hh);
            cx.backend.fill_round_rect(quad(0.0, 0.0), 0.0, color);
            cx.backend.fill_round_rect(quad(hw, 0.0), 0.0, Color::WHITE);
            cx.backend.fill_round_rect(quad(0.0, hh), 0.0, Color::BLACK);
            cx.backend.fill_round_rect(quad(hw, hh), 0.0, theme.muted);
        }
        FillType::Image => {
            cx.backend.fill_round_rect(swatch, 3.0, theme.background);
            draw_icon(
                cx.backend,
                Icon::ImagePlus,
                Point2D::new(swatch.origin.x + 2.0, swatch.origin.y + 2.0),
                12.0,
                theme.muted_foreground,
                1.4,
            );
        }
    }
    cx.backend.stroke_round_rect(swatch, 3.0, theme.border, 1.0);
}
