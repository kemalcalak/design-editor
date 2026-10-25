//! The uniform rows and the Code row of a shader fill / effect body (#164).
//!
//! One row per user uniform the program declares
//! (`de_editor_core::shader_uniform_specs`), labelled by its `@label`:
//!
//! - a ranged float: a slider plus a number;
//! - any other float: a number;
//! - a vector: one number per component (`x y z w`);
//! - a colour: swatch + hex, which opens the HSV picker.
//!
//! The Code row under them opens the source editor and says whether the
//! program compiles (the hosts compile it, `de_editor_core::shader_check`).
//! The primary shader fill paints this under its preset row, a shader
//! effect under its effect row; both walk the same rects here, so paint,
//! the action walker and the input walker cannot drift.

use crate::theme::Theme;
use crate::util::format_panel_number;
use crate::widgets::icons::{draw_icon, Icon};
use crate::widgets::property_panel::PropertyPanelAction;
use crate::widgets::property_panel_inputs::{
    paint_input_with_prefix_focused_state, INPUT_HEIGHT, INPUT_RADIUS, INPUT_TEXT_BASELINE, PAD_X,
};
use crate::widgets::property_panel_sections::EditContext;
use crate::widgets::text_metrics;
use crate::widgets::PaintCx;
use crate::{Point2D, Rect, TextLayout};
use de_editor_core::shader_check::ShaderCheckCache;
use de_editor_core::{
    ColorTarget, PropertyFocus, ShaderCheck, ShaderProgram, ShaderTarget, UniformKind,
};

/// Space between two uniform rows.
const ROW_GAP: f32 = 4.0;
const ROW_STEP: f32 = INPUT_HEIGHT + ROW_GAP;
/// Share of the row the label takes.
const LABEL_SHARE: f32 = 0.4;
/// The number next to a slider.
const SLIDER_NUMBER_W: f32 = 48.0;
const FIELD_GAP: f32 = 4.0;
const LABEL_FONT_SIZE: f32 = 11.0;
/// The Code button's width.
const CODE_BUTTON_W: f32 = 76.0;
/// Space under the Code row.
const TRAILING_GAP: f32 = 6.0;
/// Side of a boolean row's checkbox.
const SWITCH_SIDE: f32 = 16.0;
/// Prefixes of a vector's component fields.
const COMPONENTS: [&str; 4] = ["x", "y", "z", "w"];

/// One uniform as the panel shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct ShaderUniformRow {
    pub label: String,
    pub kind: UniformKind,
    pub range: Option<(f32, f32)>,
    /// The number fields' values (none for a colour).
    pub numbers: Vec<f32>,
    /// The colour as `#RRGGBB[AA]`, for a colour row.
    pub hex: Option<String>,
}

/// A shader body: its uniform rows and the program's compile state.
#[derive(Debug, Clone, PartialEq)]
pub struct ShaderBody {
    pub rows: Vec<ShaderUniformRow>,
    pub check: ShaderCheck,
}

impl ShaderBody {
    /// The body of `program`; `checks` (the hosts' compile results) is
    /// `None` where the snapshot has no editor state at hand.
    pub fn of(program: ShaderProgram<'_>, checks: Option<&ShaderCheckCache>) -> Self {
        let rows = program
            .specs()
            .into_iter()
            .map(|spec| ShaderUniformRow {
                label: spec.display_label().to_owned(),
                kind: spec.kind,
                range: spec.range,
                numbers: spec.numbers_in(program.uniforms),
                hex: spec.hex_in(program.uniforms),
            })
            .collect();
        let check = match (program.sksl, checks) {
            (Some(sksl), Some(checks)) => checks.status(sksl),
            _ => ShaderCheck::Pending,
        };
        Self { rows, check }
    }
}

/// Fill in what lives on the editor state, which the node-only snapshot
/// builder cannot see: the hosts' compile results, and the current value
/// of every `$variable`-bound uniform (resolved like the canvas does,
/// with the variable named in the row label).
pub fn attach_checks(
    snapshot: &mut crate::widgets::property_panel::NodeSnapshot,
    node: &jian_ops_schema::node::PenNode,
    state: &de_editor_core::EditorState,
) {
    let checks = &state.editor_ui.shader_editing.checks;
    let refresh = |body: &mut ShaderBody, target| {
        let Some(program) = de_editor_core::shader_edit::shader_program(node, target) else {
            return;
        };
        // A variable-bound uniform shows its value under the active theme
        // (#165); the node-only snapshot has no variables at hand.
        let resolved = program.uniforms.and_then(|uniforms| {
            de_editor_core::variables_resolve::resolve_shader_uniforms(
                uniforms,
                &state.doc,
                state.active_children(),
                state.selection.anchor.as_str(),
                &state.ui.variables.active_theme,
            )
        });
        if let Some(resolved) = resolved.as_ref() {
            let shown = de_editor_core::shader_edit::ShaderProgram {
                sksl: program.sksl,
                uniforms: Some(resolved),
            };
            body.rows = ShaderBody::of(shown, None).rows;
            // Name the variable a row follows: an edit writes a plain
            // value over it.
            let specs = program.specs();
            for (row, spec) in body.rows.iter_mut().zip(specs) {
                if let Some(jian_ops_schema::style::ShaderUniformValue::Color(reference)) =
                    program.uniforms.and_then(|map| map.get(&spec.name))
                {
                    if de_editor_core::variables_resolve::is_variable_ref(reference) {
                        row.label = format!("{} ({reference})", row.label);
                    }
                }
            }
        }
        body.check = program
            .sksl
            .map_or(ShaderCheck::Pending, |sksl| checks.status(sksl));
    };
    for (index, fill) in snapshot.fills.iter_mut().enumerate() {
        if let Some(body) = fill.shader.as_mut() {
            refresh(body, ShaderTarget::Fill(index));
        }
    }
    for (index, effect) in snapshot.effects.iter_mut().enumerate() {
        if let Some(body) = effect.shader.as_mut() {
            refresh(body, ShaderTarget::Effect(index));
        }
    }
}

/// The body of shader paint `target` in the panel snapshot.
pub fn body_for(
    snapshot: &crate::widgets::property_panel::NodeSnapshot,
    target: ShaderTarget,
) -> Option<&ShaderBody> {
    match target {
        ShaderTarget::Fill(index) => snapshot.fills.get(index)?.shader.as_ref(),
        ShaderTarget::Effect(index) => snapshot.effects.get(index)?.shader.as_ref(),
    }
}

/// The primary fill body's item count the fill walkers carry: the
/// shader's uniform rows, else the gradient stops / mesh vertices.
pub fn primary_body_items(snapshot: &crate::widgets::property_panel::NodeSnapshot) -> usize {
    snapshot
        .fills
        .first()
        .and_then(|fill| fill.shader.as_ref())
        .map_or(snapshot.gradient_stops.len(), |body| body.rows.len())
}

/// Height of the uniform rows plus the Code row.
pub fn body_height(body: &ShaderBody) -> f32 {
    body_height_for_rows(body.rows.len())
}

/// Height of `rows` uniform rows plus the Code row (the fill section's
/// walkers carry only the count).
pub fn body_height_for_rows(rows: usize) -> f32 {
    rows as f32 * ROW_STEP + INPUT_HEIGHT + TRAILING_GAP
}

fn row_rect(x: f32, y: f32, width: f32, index: usize) -> Rect {
    Rect::xywh(
        x + PAD_X,
        y + index as f32 * ROW_STEP,
        width - PAD_X * 2.0,
        INPUT_HEIGHT,
    )
}

/// `(label, control)` halves of a row.
fn split_row(row: Rect) -> (Rect, Rect) {
    let label_w = (row.size.x * LABEL_SHARE).round();
    (
        Rect::xywh(row.origin.x, row.origin.y, label_w - FIELD_GAP, row.size.y),
        Rect::xywh(
            row.origin.x + label_w,
            row.origin.y,
            row.size.x - label_w,
            row.size.y,
        ),
    )
}

/// `n` equal fields across `control`.
fn fields(control: Rect, n: usize) -> Vec<Rect> {
    let n = n.max(1);
    let w = (control.size.x - FIELD_GAP * (n as f32 - 1.0)) / n as f32;
    (0..n)
        .map(|i| {
            Rect::xywh(
                control.origin.x + i as f32 * (w + FIELD_GAP),
                control.origin.y,
                w,
                control.size.y,
            )
        })
        .collect()
}

/// The checkbox of a boolean row, at the start of its control.
fn switch_rect(control: Rect) -> Rect {
    let side = SWITCH_SIDE.min(control.size.y);
    Rect::xywh(
        control.origin.x,
        control.origin.y + (control.size.y - side) / 2.0,
        side,
        side,
    )
}

/// A ranged float's `(slider, number)`.
fn slider_and_number(control: Rect) -> (Rect, Rect) {
    let number = Rect::xywh(
        control.origin.x + control.size.x - SLIDER_NUMBER_W,
        control.origin.y,
        SLIDER_NUMBER_W,
        control.size.y,
    );
    let slider = Rect::xywh(
        control.origin.x + 4.0,
        control.origin.y,
        (control.size.x - SLIDER_NUMBER_W - FIELD_GAP - 8.0).max(8.0),
        control.size.y,
    );
    (slider, number)
}

/// The slider value at `x` for `range`.
pub fn slider_value(slider: Rect, range: (f32, f32), x: f32) -> f32 {
    let t = ((x - slider.origin.x) / slider.size.x.max(1.0)).clamp(0.0, 1.0);
    let (lo, hi) = range;
    let value = lo + t * (hi - lo);
    // Two decimals is finer than a 100 px track can resolve anyway.
    (value * 100.0).round() / 100.0
}

/// The number fields of a row: `(component, rect)`.
fn number_rects(row: &ShaderUniformRow, control: Rect) -> Vec<(usize, Rect)> {
    match (row.kind, row.range) {
        (UniformKind::Float | UniformKind::Int(1), Some(_)) => {
            vec![(0, slider_and_number(control).1)]
        }
        (UniformKind::Float | UniformKind::Int(1), None) => vec![(0, control)],
        (UniformKind::Vector(n) | UniformKind::Int(n), _) => {
            fields(control, n).into_iter().enumerate().collect()
        }
        (UniformKind::Color { .. } | UniformKind::Bool, _) => Vec::new(),
    }
}

fn code_button_rect(x: f32, y: f32, width: f32, rows: usize) -> Rect {
    let row = row_rect(x, y, width, rows);
    Rect::xywh(row.origin.x, row.origin.y, CODE_BUTTON_W, row.size.y)
}

/// Action rects of a body starting at `y`: sliders, colour swatches and
/// the Code button.
pub fn push_action_rects(
    out: &mut Vec<(PropertyPanelAction, Rect)>,
    target: ShaderTarget,
    body: &ShaderBody,
    x: f32,
    y: f32,
    width: f32,
) {
    for (index, row) in body.rows.iter().enumerate() {
        let (_, control) = split_row(row_rect(x, y, width, index));
        match (row.kind, row.range) {
            (UniformKind::Bool, _) => out.push((
                PropertyPanelAction::ToggleShaderUniform {
                    target,
                    index,
                    on: row.numbers.first().is_none_or(|v| *v == 0.0),
                },
                switch_rect(control),
            )),
            (UniformKind::Float | UniformKind::Int(1), Some(_)) => out.push((
                PropertyPanelAction::DragShaderUniform {
                    target,
                    index,
                    value: row.numbers.first().copied().unwrap_or(0.0),
                },
                slider_and_number(control).0,
            )),
            (UniformKind::Color { .. }, _) => out.push((
                PropertyPanelAction::OpenColorPicker(ColorTarget::ShaderUniform { target, index }),
                control,
            )),
            _ => {}
        }
    }
    out.push((
        PropertyPanelAction::OpenShaderSource(target),
        code_button_rect(x, y, width, body.rows.len()),
    ));
}

/// The drag action for row `index` with the value under `x`; `slider` is
/// the slider rect the action walker emitted for that row.
pub fn drag_value(
    target: ShaderTarget,
    index: usize,
    body: &ShaderBody,
    slider: Rect,
    x: f32,
) -> Option<PropertyPanelAction> {
    let range = body.rows.get(index)?.range?;
    Some(PropertyPanelAction::DragShaderUniform {
        target,
        index,
        value: slider_value(slider, range, x),
    })
}

/// Number-field rects of a body starting at `y`.
pub fn push_input_rects(
    out: &mut Vec<(PropertyFocus, Rect)>,
    target: ShaderTarget,
    body: &ShaderBody,
    x: f32,
    y: f32,
    width: f32,
) {
    for (index, row) in body.rows.iter().enumerate() {
        let (_, control) = split_row(row_rect(x, y, width, index));
        for (component, rect) in number_rects(row, control) {
            out.push((
                PropertyFocus::ShaderUniform {
                    target,
                    index,
                    component,
                },
                rect,
            ));
        }
    }
}

/// The value a number field shows when it is not being edited.
pub fn field_value(body: &ShaderBody, index: usize, component: usize) -> Option<String> {
    let value = body.rows.get(index)?.numbers.get(component)?;
    Some(format_panel_number(*value))
}

#[allow(clippy::too_many_arguments)]
pub fn paint_body(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    edit: &EditContext<'_>,
    locale: de_editor_core::Locale,
    target: ShaderTarget,
    body: &ShaderBody,
    x: f32,
    y: f32,
    width: f32,
) {
    for (index, row) in body.rows.iter().enumerate() {
        let (label, control) = split_row(row_rect(x, y, width, index));
        paint_label(cx, theme, label, &row.label);
        match (row.kind, row.range) {
            (UniformKind::Float | UniformKind::Int(1), Some(range)) => {
                let value = row.numbers.first().copied().unwrap_or(range.0);
                paint_slider(cx, theme, slider_and_number(control).0, range, value);
            }
            (UniformKind::Bool, _) => {
                let checked = row.numbers.first().is_some_and(|v| *v != 0.0);
                jian_widgets::components::checkbox::Checkbox {
                    checked,
                    enabled: true,
                }
                .paint(
                    cx.backend,
                    switch_rect(control),
                    &crate::widgets::button::tokens_from_theme(theme),
                );
            }
            (UniformKind::Color { .. }, _) => {
                paint_colour(cx, theme, control, row.hex.as_deref().unwrap_or("#000000"));
            }
            _ => {}
        }
        let vector = matches!(row.kind, UniformKind::Vector(_))
            || matches!(row.kind, UniformKind::Int(n) if n > 1);
        for (component, rect) in number_rects(row, control) {
            let focus = PropertyFocus::ShaderUniform {
                target,
                index,
                component,
            };
            let fallback = row
                .numbers
                .get(component)
                .map(|v| format_panel_number(*v))
                .unwrap_or_default();
            paint_input_with_prefix_focused_state(
                cx,
                theme,
                rect,
                if vector {
                    COMPONENTS[component.min(3)]
                } else {
                    ""
                },
                edit.value_for(focus, &fallback),
                edit.focus == Some(focus),
                edit.caret_at(focus),
                edit.select_all_at(focus),
                edit.input_at(focus),
                edit.now_ms,
            );
        }
    }
    paint_code_row(cx, theme, locale, body, x, y, width);
}

fn paint_label(cx: &mut PaintCx<'_>, theme: &Theme, rect: Rect, label: &str) {
    let text = text_metrics::fit_chrome(cx.backend, label, rect.size.x.max(0.0), LABEL_FONT_SIZE);
    let layout = TextLayout::single_run(
        &text,
        text_metrics::CHROME_FONT_FAMILY,
        LABEL_FONT_SIZE,
        theme.muted_foreground.to_jian(),
        Point2D::new(0.0, 0.0),
    );
    cx.backend.draw_text(
        &layout,
        Point2D::new(rect.origin.x, rect.origin.y + INPUT_TEXT_BASELINE),
    );
}

/// Track, fill and knob. The knob is a ring (unlike the effect amount
/// slider's dot) so it stays visible over the filled track at a range end.
fn paint_slider(cx: &mut PaintCx<'_>, theme: &Theme, slider: Rect, range: (f32, f32), value: f32) {
    let (lo, hi) = range;
    let t = ((value - lo) / (hi - lo)).clamp(0.0, 1.0);
    let track = Rect::xywh(
        slider.origin.x,
        slider.origin.y + (slider.size.y - 4.0) / 2.0,
        slider.size.x,
        4.0,
    );
    cx.backend.fill_round_rect(track, 2.0, theme.border);
    cx.backend.fill_round_rect(
        Rect::xywh(track.origin.x, track.origin.y, track.size.x * t, 4.0),
        2.0,
        theme.primary,
    );
    let knob = Rect::xywh(
        track.origin.x + track.size.x * t - 5.0,
        track.origin.y - 3.0,
        10.0,
        10.0,
    );
    cx.backend.fill_oval(knob, theme.primary);
    cx.backend.fill_oval(
        Rect::xywh(knob.origin.x + 2.5, knob.origin.y + 2.5, 5.0, 5.0),
        theme.primary_foreground,
    );
}

fn paint_colour(cx: &mut PaintCx<'_>, theme: &Theme, control: Rect, hex: &str) {
    cx.backend
        .fill_round_rect(control, INPUT_RADIUS, theme.muted);
    let swatch = Rect::xywh(control.origin.x + 6.0, control.origin.y + 6.0, 16.0, 16.0);
    let color = crate::util::parse_hex_color(hex).unwrap_or(crate::Color::TRANSPARENT);
    jian_widgets::components::swatch::Swatch {
        color,
        radius: 3.0,
        border: true,
    }
    .paint(
        cx.backend,
        swatch,
        &crate::widgets::button::tokens_from_theme(theme),
    );
    let text_x = control.origin.x + 30.0;
    let text = text_metrics::fit_in_family(
        cx.backend,
        &hex.to_ascii_uppercase(),
        (control.origin.x + control.size.x - 6.0 - text_x).max(0.0),
        12.0,
        text_metrics::MONO_FONT_FAMILY,
    );
    let layout = TextLayout::single_run(
        &text,
        text_metrics::MONO_FONT_FAMILY,
        12.0,
        theme.foreground.to_jian(),
        Point2D::new(0.0, 0.0),
    );
    cx.backend.draw_text(
        &layout,
        Point2D::new(text_x, control.origin.y + INPUT_TEXT_BASELINE),
    );
}

/// A muted button with the code glyph and `label`: the shader Code row's
/// button and the Script section's Edit script (#19).
pub(crate) fn paint_code_button(cx: &mut PaintCx<'_>, theme: &Theme, button: Rect, label: &str) {
    cx.backend
        .fill_round_rect(button, INPUT_RADIUS, theme.muted);
    draw_icon(
        cx.backend,
        Icon::Code,
        Point2D::new(
            button.origin.x + 8.0,
            button.origin.y + (button.size.y - 14.0) / 2.0,
        ),
        14.0,
        theme.foreground,
        1.4,
    );
    let label = text_metrics::fit_chrome(cx.backend, label, button.size.x - 32.0, LABEL_FONT_SIZE);
    let layout = TextLayout::single_run(
        &label,
        text_metrics::CHROME_FONT_FAMILY,
        LABEL_FONT_SIZE,
        theme.foreground.to_jian(),
        Point2D::new(0.0, 0.0),
    );
    cx.backend.draw_text(
        &layout,
        Point2D::new(
            button.origin.x + 28.0,
            button.origin.y + INPUT_TEXT_BASELINE,
        ),
    );
}

/// The Code button and the compile state beside it.
fn paint_code_row(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    locale: de_editor_core::Locale,
    body: &ShaderBody,
    x: f32,
    y: f32,
    width: f32,
) {
    let button = code_button_rect(x, y, width, body.rows.len());
    paint_code_button(
        cx,
        theme,
        button,
        de_i18n::translate(locale, "shader.editCode"),
    );
    let (key, color) = match body.check {
        ShaderCheck::Pending => return,
        ShaderCheck::Ok => ("shader.compiles", theme.muted_foreground),
        ShaderCheck::Error(_) => ("shader.compileError", theme.destructive),
    };
    let row = row_rect(x, y, width, body.rows.len());
    let status_x = button.origin.x + button.size.x + 8.0;
    let status = de_i18n::translate(locale, key);
    let status = text_metrics::fit_chrome(
        cx.backend,
        status,
        (row.origin.x + row.size.x - status_x).max(0.0),
        LABEL_FONT_SIZE,
    );
    let layout = TextLayout::single_run(
        &status,
        text_metrics::CHROME_FONT_FAMILY,
        LABEL_FONT_SIZE,
        color.to_jian(),
        Point2D::new(0.0, 0.0),
    );
    cx.backend.draw_text(
        &layout,
        Point2D::new(status_x, row.origin.y + INPUT_TEXT_BASELINE),
    );
}

#[cfg(test)]
#[path = "property_panel_shader_body_tests.rs"]
mod tests;
