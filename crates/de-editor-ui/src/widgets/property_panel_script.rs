//! Script section (#19): one row per input the program declares, the
//! Edit script button and the last run's status. Visible when the
//! selection is a script node (`VisibleSections.script`). Number, string,
//! colour and ref inputs are text fields (`PropertyFocus::ScriptInput`,
//! where `$name` binds a variable); a boolean toggles and an enum steps to
//! its next choice on click. Paint and both layout walkers step through
//! the same rows.

use jian_ops_schema::node::{PenNode, ScriptInputValue};

use crate::theme::Theme;
use crate::util::format_panel_number;
use crate::widgets::icons::{draw_icon, Icon};
use crate::widgets::property_panel::{NodeSnapshot, PropertyPanelAction};
use crate::widgets::property_panel_inputs::{
    paint_input_with_prefix_focused_state, paint_section_divider, paint_section_label,
    INPUT_HEIGHT, INPUT_RADIUS, INPUT_TEXT_BASELINE, PAD_X, SECTION_GAP, SECTION_HEADER_HEIGHT,
};
use crate::widgets::property_panel_sections::EditContext;
use crate::widgets::text_metrics;
use crate::widgets::PaintCx;
use crate::{Point2D, Rect, TextLayout};
use de_editor_core::script_inputs::{script_input_specs, ScriptInputKind};
use de_editor_core::script_regen::ScriptRunStatus;
use de_editor_core::{EditorState, PropertyFocus};

/// One row's vertical advance (input + gap).
const ROW_ADVANCE: f32 = INPUT_HEIGHT + 6.0;
/// Space under the last row, above the divider.
const BOTTOM_PAD: f32 = 12.0;
const LABEL_FONT_SIZE: f32 = 11.0;
const EDIT_BUTTON_W: f32 = 120.0;
/// Rows past this many are not shown (the masks below are 64 bits).
const MAX_ROWS: usize = 64;

/// How one input is edited.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptControl {
    /// A text field.
    Text,
    /// A checkbox, checked or not.
    Toggle(bool),
    /// A field whose click steps to the next enum choice.
    Cycle,
}

/// One input row.
#[derive(Debug, Clone, PartialEq)]
pub struct ScriptInputRow {
    pub name: String,
    pub control: ScriptControl,
    /// The value as the field shows it (a binding shows `$name`).
    pub value: String,
}

/// What the status line says.
#[derive(Debug, Clone, PartialEq)]
pub enum ScriptStatusLine {
    /// Nothing ran in this session.
    None,
    Running,
    Done {
        nodes: usize,
        elapsed_ms: u64,
    },
    Failed(String),
    /// The web editor has no script engine: it shows saved children only.
    DesktopOnly,
}

/// The selected script node's rows and status.
#[derive(Debug, Clone, PartialEq)]
pub struct ScriptSummary {
    pub rows: Vec<ScriptInputRow>,
    pub status: ScriptStatusLine,
}

/// The section's shape, small enough for the `Copy` `VisibleSections`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ScriptRows {
    pub count: usize,
    toggle_mask: u64,
    cycle_mask: u64,
    has_status: bool,
}

impl ScriptRows {
    pub fn of(summary: &ScriptSummary) -> Self {
        let mut rows = ScriptRows {
            count: summary.rows.len().min(MAX_ROWS),
            has_status: summary.status != ScriptStatusLine::None,
            ..Default::default()
        };
        for (i, row) in summary.rows.iter().take(MAX_ROWS).enumerate() {
            match row.control {
                ScriptControl::Toggle(_) => rows.toggle_mask |= 1 << i,
                ScriptControl::Cycle => rows.cycle_mask |= 1 << i,
                ScriptControl::Text => {}
            }
        }
        rows
    }

    fn control_at(&self, i: usize) -> ScriptControl {
        if self.toggle_mask & (1 << i) != 0 {
            ScriptControl::Toggle(false)
        } else if self.cycle_mask & (1 << i) != 0 {
            ScriptControl::Cycle
        } else {
            ScriptControl::Text
        }
    }
}

fn display(value: &ScriptInputValue) -> String {
    match value {
        ScriptInputValue::Bool(b) => b.to_string(),
        ScriptInputValue::Number(n) => format_panel_number(*n as f32),
        ScriptInputValue::Text(text) => text.clone(),
    }
}

/// `Some` for a script node. `state` supplies the last run's status.
pub fn script_summary(node: &PenNode, state: &EditorState) -> Option<ScriptSummary> {
    let PenNode::Script(script) = node else {
        return None;
    };
    let specs = script
        .script
        .as_deref()
        .map(script_input_specs)
        .unwrap_or_default();
    let rows = specs
        .iter()
        .map(|spec| {
            let value = script
                .inputs
                .as_ref()
                .and_then(|inputs| inputs.get(&spec.name))
                .or(spec.default.as_ref());
            let control = match &spec.kind {
                ScriptInputKind::Boolean => {
                    ScriptControl::Toggle(matches!(value, Some(ScriptInputValue::Bool(true))))
                }
                ScriptInputKind::Enum(_) => ScriptControl::Cycle,
                _ => ScriptControl::Text,
            };
            let value = match (value, &spec.kind) {
                (Some(value), _) => display(value),
                (None, ScriptInputKind::Enum(choices)) => choices[0].clone(),
                (None, _) => String::new(),
            };
            ScriptInputRow {
                name: spec.name.clone(),
                control,
                value,
            }
        })
        .collect();
    let id = de_editor_core::NodeId::new(script.frame.base.id.as_str());
    let status = match state.script_regen.status(&id) {
        Some(ScriptRunStatus::Running) => ScriptStatusLine::Running,
        Some(ScriptRunStatus::Done { nodes, elapsed_ms }) => ScriptStatusLine::Done {
            nodes: *nodes,
            elapsed_ms: *elapsed_ms,
        },
        Some(ScriptRunStatus::Failed { message, .. }) => ScriptStatusLine::Failed(message.clone()),
        None if cfg!(target_arch = "wasm32") => ScriptStatusLine::DesktopOnly,
        None => ScriptStatusLine::None,
    };
    Some(ScriptSummary { rows, status })
}

/// Section height without the trailing gap (callers add `SECTION_GAP`).
pub fn script_section_height(rows: ScriptRows) -> f32 {
    let lines = rows.count + 1 + usize::from(rows.has_status);
    SECTION_HEADER_HEIGHT + ROW_ADVANCE * lines as f32 + BOTTOM_PAD
}

fn row_rect(x0: f32, top: f32, usable_w: f32, i: usize) -> Rect {
    Rect {
        origin: Point2D::new(
            x0 + PAD_X,
            top + SECTION_HEADER_HEIGHT + ROW_ADVANCE * i as f32,
        ),
        size: Point2D::new(usable_w, INPUT_HEIGHT),
    }
}

fn edit_button_rect(x0: f32, top: f32, usable_w: f32, rows: usize) -> Rect {
    let row = row_rect(x0, top, usable_w, rows);
    Rect {
        origin: row.origin,
        size: Point2D::new(EDIT_BUTTON_W.min(usable_w), INPUT_HEIGHT),
    }
}

/// The toggle / cycle rows and the Edit script button.
pub fn push_script_action_rects(
    out: &mut Vec<(PropertyPanelAction, Rect)>,
    rows: ScriptRows,
    x0: f32,
    top: f32,
    usable_w: f32,
) {
    for i in 0..rows.count {
        let action = match rows.control_at(i) {
            ScriptControl::Toggle(_) => PropertyPanelAction::ToggleScriptInput(i),
            ScriptControl::Cycle => PropertyPanelAction::CycleScriptInput(i),
            ScriptControl::Text => continue,
        };
        out.push((action, row_rect(x0, top, usable_w, i)));
    }
    out.push((
        PropertyPanelAction::OpenScriptSource,
        edit_button_rect(x0, top, usable_w, rows.count),
    ));
}

/// The text-field rows.
pub fn push_script_input_rects(
    rects: &mut Vec<(PropertyFocus, Rect)>,
    rows: ScriptRows,
    x0: f32,
    top: f32,
    usable_w: f32,
) {
    for i in 0..rows.count {
        if rows.control_at(i) == ScriptControl::Text {
            rects.push((
                PropertyFocus::ScriptInput(i),
                row_rect(x0, top, usable_w, i),
            ));
        }
    }
}

/// Paint the section. Returns the y below it, gap included.
#[allow(clippy::too_many_arguments)]
pub fn paint_script_section(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    snapshot: &NodeSnapshot,
    edit: &EditContext<'_>,
    locale: de_editor_core::Locale,
    x: f32,
    y: f32,
    width: f32,
) -> f32 {
    let Some(summary) = snapshot.script.as_ref() else {
        return y;
    };
    let usable_w = width - PAD_X * 2.0;
    let top = y;
    paint_section_label(cx, theme, label(locale, "script.title"), x, y, width);
    for (i, row) in summary.rows.iter().take(MAX_ROWS).enumerate() {
        let rect = row_rect(x, top, usable_w, i);
        match row.control {
            ScriptControl::Toggle(checked) => paint_check_row(cx, theme, rect, &row.name, checked),
            ScriptControl::Text | ScriptControl::Cycle => {
                let focus = PropertyFocus::ScriptInput(i);
                let text = (row.control == ScriptControl::Text).then_some(focus);
                paint_input_with_prefix_focused_state(
                    cx,
                    theme,
                    rect,
                    &row.name,
                    text.map_or(row.value.as_str(), |f| edit.value_for(f, &row.value)),
                    text.is_some_and(|f| edit.focus == Some(f)),
                    text.and_then(|f| edit.caret_at(f)),
                    text.is_some_and(|f| edit.select_all_at(f)),
                    text.and_then(|f| edit.input_at(f)),
                    edit.now_ms,
                );
            }
        }
    }
    let rows = summary.rows.len().min(MAX_ROWS);
    paint_edit_button(cx, theme, locale, edit_button_rect(x, top, usable_w, rows));
    if let Some((text, color)) = status_text(locale, &summary.status, theme) {
        let rect = row_rect(x, top, usable_w, rows + 1);
        let text = text_metrics::fit_chrome(cx.backend, &text, rect.size.x, LABEL_FONT_SIZE);
        let layout = TextLayout::single_run(
            &text,
            text_metrics::CHROME_FONT_FAMILY,
            LABEL_FONT_SIZE,
            color.to_jian(),
            Point2D::new(0.0, 0.0),
        );
        cx.backend.draw_text(
            &layout,
            Point2D::new(rect.origin.x, rect.origin.y + INPUT_TEXT_BASELINE),
        );
    }
    let lines = rows + 1 + usize::from(summary.status != ScriptStatusLine::None);
    let y = top + SECTION_HEADER_HEIGHT + ROW_ADVANCE * lines as f32 + BOTTOM_PAD;
    paint_section_divider(cx, theme, x, y, width);
    y + SECTION_GAP
}

fn status_text(
    locale: de_editor_core::Locale,
    status: &ScriptStatusLine,
    theme: &Theme,
) -> Option<(String, crate::Color)> {
    Some(match status {
        ScriptStatusLine::None => return None,
        ScriptStatusLine::Running => (
            label(locale, "script.running").to_string(),
            theme.muted_foreground,
        ),
        ScriptStatusLine::Done { nodes, elapsed_ms } => (
            label(locale, "script.generated")
                .replace("{count}", &nodes.to_string())
                .replace("{ms}", &elapsed_ms.to_string()),
            theme.muted_foreground,
        ),
        ScriptStatusLine::Failed(message) => (message.clone(), theme.destructive),
        ScriptStatusLine::DesktopOnly => (
            label(locale, "script.desktopOnly").to_string(),
            theme.muted_foreground,
        ),
    })
}

fn paint_check_row(cx: &mut PaintCx<'_>, theme: &Theme, rect: Rect, name: &str, checked: bool) {
    let box_rect = Rect {
        origin: Point2D::new(rect.origin.x, rect.origin.y + (INPUT_HEIGHT - 16.0) / 2.0),
        size: Point2D::new(16.0, 16.0),
    };
    jian_widgets::components::checkbox::Checkbox {
        checked,
        enabled: true,
    }
    .paint(
        cx.backend,
        box_rect,
        &crate::widgets::button::tokens_from_theme(theme),
    );
    let layout = TextLayout::single_run(
        name,
        text_metrics::CHROME_FONT_FAMILY,
        12.0,
        theme.foreground.to_jian(),
        Point2D::new(0.0, 0.0),
    );
    cx.backend.draw_text(
        &layout,
        Point2D::new(rect.origin.x + 22.0, rect.origin.y + INPUT_TEXT_BASELINE),
    );
}

fn paint_edit_button(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    locale: de_editor_core::Locale,
    button: Rect,
) {
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
    let text = label(locale, "script.edit");
    let text = text_metrics::fit_chrome(cx.backend, text, button.size.x - 32.0, LABEL_FONT_SIZE);
    let layout = TextLayout::single_run(
        &text,
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

/// The panel's script actions, shared by both hosts' dispatch.
pub fn apply_script_action(state: &mut EditorState, action: &PropertyPanelAction) -> bool {
    let write = |state: &mut EditorState, edit: &dyn Fn(&mut EditorState) -> bool| {
        let before = state.snapshot_for_history();
        if edit(state) {
            state.history_push_past(before);
        }
    };
    match action {
        PropertyPanelAction::ToggleScriptInput(i) => {
            write(state, &|s| s.toggle_selected_script_input(*i));
        }
        PropertyPanelAction::CycleScriptInput(i) => {
            write(state, &|s| s.cycle_selected_script_input(*i));
        }
        PropertyPanelAction::OpenScriptSource => {
            let _ = state.open_script_source_editor();
        }
        _ => return false,
    }
    true
}

fn label(locale: de_editor_core::Locale, key: &'static str) -> &'static str {
    crate::i18n::translate(locale, key)
}

#[cfg(test)]
#[path = "property_panel_script_tests.rs"]
mod tests;
