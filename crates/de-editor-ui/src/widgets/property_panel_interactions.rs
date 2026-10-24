//! Interactions section: the read-only screen marker, then one row per
//! authored trigger (Tap / Hover / Long press) with its action, and a
//! Transition row under a navigate (#154). The Motion section
//! (`property_panel_motion.rs`) paints directly below it in the same
//! slot, so its summary rides on [`InteractionSummary::motion`].
//!
//! Rows per trigger:
//! - one action: a dropdown row opening the shared popover (Navigate
//!   to <screen> / Back / Remove), plus a Slide | Fade row when the
//!   action navigates — the verb is the screen transition Preview plays
//!   (`push` slides, `replace` cross-fades);
//! - several actions (hand-authored or MCP-written): each read-only,
//!   then a "Remove all" row.
//!
//! The header "+" opens the popover's list: the triggers not authored
//! yet, then "Animation", which adds a Motion row. The Motion section
//! only appears once the node has an animation, so a node with neither
//! stays one header tall (the #83 compact budget). The edits live in
//! `de_editor_core::interaction_edit`; the popover is in
//! `property_panel_interactions_menu.rs`.

use crate::theme::Theme;
use crate::widgets::property_panel::PropertyPanelAction;
use crate::widgets::property_panel_inputs::{
    paint_dropdown, paint_section_divider, paint_section_label_with_add, section_add_target,
    INPUT_HEIGHT, PAD_X, SECTION_GAP, SECTION_HEADER_HEIGHT,
};
use crate::widgets::property_panel_motion::MotionSummary;
use crate::widgets::PaintCx;
use crate::{Point2D, Rect, TextLayout};
use de_editor_core::pen_node_ext::PenNodeExt;
use jian_ops_schema::events::Action;
use jian_ops_schema::node::PenNode;

pub use crate::widgets::property_panel_interactions_menu::{
    interaction_menu_rect, interaction_menu_row_rects, interaction_menu_rows,
    paint_interaction_menu, InteractionMenuHit, INTERACTION_MENU_ROW_H, INTERACTION_MENU_W,
};
pub use de_editor_core::interaction_edit::{
    InteractionMenuTarget, InteractionTrigger, NavigateVerb,
};

/// One authored action, summarized for display. `Other` covers every
/// action key besides navigation (`set`, `fetch`, …), shown read-only.
#[derive(Debug, Clone, PartialEq)]
pub enum TapActionSummary {
    Navigate { verb: NavigateVerb, path: String },
    Pop,
    Other(String),
}

/// One authored trigger and its actions (never empty).
#[derive(Debug, Clone, PartialEq)]
pub struct TriggerSummary {
    pub trigger: InteractionTrigger,
    pub actions: Vec<TapActionSummary>,
}

/// Interaction and motion state for the selected node — always present
/// (empty when nothing is authored). `screen` is `Some` only for a
/// top-level Frame carrying an authored `screen` route.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct InteractionSummary {
    pub screen: Option<String>,
    /// Authored triggers, in [`InteractionTrigger::ALL`] order.
    pub triggers: Vec<TriggerSummary>,
    /// The Motion section's rows (`animations`).
    pub motion: Vec<MotionSummary>,
}

impl InteractionSummary {
    /// The actions authored for `trigger` (empty when none).
    pub fn actions(&self, trigger: InteractionTrigger) -> &[TapActionSummary] {
        self.triggers
            .iter()
            .find(|t| t.trigger == trigger)
            .map_or(&[], |t| t.actions.as_slice())
    }

    /// Triggers with no action yet, in list order — the "+" menu rows.
    pub fn unauthored_triggers(&self) -> Vec<InteractionTrigger> {
        InteractionTrigger::ALL
            .into_iter()
            .filter(|t| self.actions(*t).is_empty())
            .collect()
    }
}

/// Build the summary from a node's `events`, `animations` and (when
/// `is_top_level`) its `screen` marker.
pub(crate) fn interactions_of(node: &PenNode, is_top_level: bool) -> InteractionSummary {
    let triggers = node
        .events()
        .map(|handlers| {
            InteractionTrigger::ALL
                .into_iter()
                .filter_map(|trigger| {
                    let actions = trigger.actions(handlers)?;
                    (!actions.is_empty()).then(|| TriggerSummary {
                        trigger,
                        actions: actions.iter().map(parse_action).collect(),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    InteractionSummary {
        screen: is_top_level
            .then(|| node.screen())
            .flatten()
            .map(str::to_string),
        triggers,
        motion: crate::widgets::property_panel_motion::motion_of(node),
    }
}

fn parse_action(action: &Action) -> TapActionSummary {
    let Some((key, body)) = action.single() else {
        return TapActionSummary::Other(String::new());
    };
    if let Some(verb) = NavigateVerb::from_key(key) {
        return TapActionSummary::Navigate {
            verb,
            path: unescape_path_literal(body),
        };
    }
    match key {
        "pop" => TapActionSummary::Pop,
        other => TapActionSummary::Other(other.to_string()),
    }
}

/// `body` is the JSON string VALUE `"\"/path\""` — the Tier-1 expression
/// source of a string literal (`wire_screen_navigation`'s contract).
/// Strip the embedded quote pair for display; any other shape (a bare
/// hand-authored path) shows as-is so the row never goes blank.
fn unescape_path_literal(body: &serde_json::Value) -> String {
    let Some(s) = body.as_str() else {
        return body.to_string();
    };
    s.strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
        .unwrap_or(s)
        .to_string()
}

/// Scan the active page's top-level frames for authored `screen` paths —
/// the popover's "Navigate to <path>" rows, in document order.
pub fn document_screen_paths(state: &de_editor_core::EditorState) -> Vec<String> {
    state
        .active_children()
        .iter()
        .filter_map(|n| n.screen().map(str::to_string))
        .collect()
}

// ── Rows: one list drives height, hit rects and paint ───────────────

/// One painted row of the section.
#[derive(Debug, Clone, PartialEq)]
enum Row<'a> {
    Screen(&'a str),
    /// A trigger's single action — opens the popover.
    Action(InteractionTrigger, &'a TapActionSummary),
    /// The Slide | Fade row under a single navigate.
    Transition(InteractionTrigger, NavigateVerb),
    /// One of several actions, read-only.
    ReadOnly(InteractionTrigger, &'a TapActionSummary),
    RemoveAll(InteractionTrigger),
}

const ROW_STEP: f32 = INPUT_HEIGHT + 6.0;
/// Width of the Transition row's label column.
const TRANSITION_LABEL_W: f32 = 76.0;

fn rows(summary: &InteractionSummary) -> Vec<Row<'_>> {
    let mut out = Vec::new();
    if let Some(screen) = &summary.screen {
        out.push(Row::Screen(screen));
    }
    for t in &summary.triggers {
        match t.actions.as_slice() {
            [] => {}
            [only] => {
                out.push(Row::Action(t.trigger, only));
                if let TapActionSummary::Navigate { verb, .. } = only {
                    out.push(Row::Transition(t.trigger, *verb));
                }
            }
            many => {
                out.extend(many.iter().map(|a| Row::ReadOnly(t.trigger, a)));
                out.push(Row::RemoveAll(t.trigger));
            }
        }
    }
    out
}

/// Total vertical space the Interactions section consumes (the Motion
/// section below it adds its own height).
pub fn interactions_section_height(interactions: &InteractionSummary) -> f32 {
    SECTION_HEADER_HEIGHT + rows(interactions).len() as f32 * ROW_STEP + 6.0 + SECTION_GAP
}

fn row_rect(x: f32, y: f32, width: f32) -> Rect {
    Rect::xywh(x + PAD_X, y, width - PAD_X * 2.0, INPUT_HEIGHT)
}

/// The Slide | Fade cells of a Transition row at `row`.
fn transition_cells(row: Rect) -> Vec<Rect> {
    let segments = Rect::xywh(
        row.origin.x + TRANSITION_LABEL_W,
        row.origin.y,
        row.size.x - TRANSITION_LABEL_W,
        row.size.y,
    );
    crate::widgets::property_panel_segments::segment_rects(segments, 2)
}

const TRANSITION_VERBS: [NavigateVerb; 2] = [NavigateVerb::Push, NavigateVerb::Replace];

/// Emit the section's clickable rects (header "+", action rows,
/// transition cells, "Remove all"), then the Motion section's — the
/// same row walk paint uses.
pub fn push_interaction_action_rects(
    out: &mut Vec<(PropertyPanelAction, Rect)>,
    interactions: &InteractionSummary,
    x: f32,
    y: f32,
    width: f32,
) {
    // The "+" always shows: an animation can always be added.
    out.push((
        PropertyPanelAction::ToggleInteractionMenu(InteractionMenuTarget::AddTrigger),
        section_add_target(x, y, width),
    ));
    let mut row_y = y + SECTION_HEADER_HEIGHT;
    for row in rows(interactions) {
        let rect = row_rect(x, row_y, width);
        match row {
            Row::Action(trigger, _) => out.push((
                PropertyPanelAction::ToggleInteractionMenu(InteractionMenuTarget::Trigger(trigger)),
                rect,
            )),
            Row::Transition(trigger, _) => {
                for (verb, cell) in TRANSITION_VERBS.into_iter().zip(transition_cells(rect)) {
                    out.push((
                        PropertyPanelAction::SetInteractionTransition { trigger, verb },
                        cell,
                    ));
                }
            }
            Row::RemoveAll(trigger) => {
                out.push((PropertyPanelAction::RemoveInteraction(trigger), rect));
            }
            Row::Screen(_) | Row::ReadOnly(..) => {}
        }
        row_y += ROW_STEP;
    }
    crate::widgets::property_panel_motion::push_motion_action_rects(
        out,
        &interactions.motion,
        x,
        y + interactions_section_height(interactions),
        width,
    );
}

/// Paint the Interactions section, then the Motion section below it.
/// Returns the y where the next section starts.
#[allow(clippy::too_many_arguments)]
pub fn paint_interactions_section(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    interactions: &InteractionSummary,
    edit: &crate::widgets::property_panel_sections::EditContext<'_>,
    locale: de_editor_core::Locale,
    x: f32,
    y: f32,
    width: f32,
) -> f32 {
    let title = translate(locale, "interactions.title", "Interactions");
    let mut y = paint_section_label_with_add(cx, theme, title, x, y, width);
    for row in rows(interactions) {
        let rect = row_rect(x, y, width);
        match row {
            Row::Screen(screen) => {
                let label = translate(locale, "interactions.screen", "Screen");
                paint_plain_row(cx, theme, rect, &format!("{label}: {screen}"));
            }
            Row::Action(trigger, action) => {
                paint_dropdown(cx, theme, rect, &action_label(locale, trigger, action));
            }
            Row::Transition(_, verb) => paint_transition_row(cx, theme, locale, rect, verb),
            Row::ReadOnly(trigger, action) => {
                paint_plain_row(cx, theme, rect, &action_label(locale, trigger, action));
            }
            Row::RemoveAll(_) => paint_dropdown(
                cx,
                theme,
                rect,
                translate(locale, "interactions.removeAll", "Remove all"),
            ),
        }
        y += ROW_STEP;
    }
    y += 6.0;
    paint_section_divider(cx, theme, x, y, width);
    crate::widgets::property_panel_motion::paint_motion_section(
        cx,
        theme,
        &interactions.motion,
        edit,
        locale,
        x,
        y + SECTION_GAP,
        width,
    )
}

fn paint_transition_row(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    locale: de_editor_core::Locale,
    rect: Rect,
    verb: NavigateVerb,
) {
    paint_plain_row(
        cx,
        theme,
        rect,
        translate(locale, "interactions.transition", "Transition"),
    );
    let segments = Rect::xywh(
        rect.origin.x + TRANSITION_LABEL_W,
        rect.origin.y,
        rect.size.x - TRANSITION_LABEL_W,
        rect.size.y,
    );
    let labels = [
        translate(locale, "interactions.slide", "Slide"),
        translate(locale, "interactions.fade", "Fade"),
    ];
    let active = TRANSITION_VERBS.iter().position(|v| *v == verb);
    crate::widgets::property_panel_segments::paint_segment_row(
        cx, theme, segments, &labels, active, 11.0,
    );
}

/// The trigger's row prefix, e.g. `Tap →`.
pub(crate) fn trigger_prefix(
    locale: de_editor_core::Locale,
    trigger: InteractionTrigger,
) -> &'static str {
    match trigger {
        InteractionTrigger::Tap => translate(locale, "interactions.tapPrefix", "Tap →"),
        InteractionTrigger::Hover => translate(locale, "interactions.hoverPrefix", "Hover →"),
        InteractionTrigger::LongPress => {
            translate(locale, "interactions.longPressPrefix", "Long press →")
        }
    }
}

/// Row label for one authored action, e.g. `Tap → Navigate /profile`,
/// `Hover → Back (pop)` or `Long press → set` (unknown key, read-only).
/// The verb is not repeated here: the Transition row shows it.
fn action_label(
    locale: de_editor_core::Locale,
    trigger: InteractionTrigger,
    action: &TapActionSummary,
) -> String {
    let prefix = trigger_prefix(locale, trigger);
    match action {
        TapActionSummary::Navigate { path, .. } => format!(
            "{prefix} {} {path}",
            translate(locale, "interactions.navigate", "Navigate"),
        ),
        TapActionSummary::Pop => {
            format!(
                "{prefix} {}",
                translate(locale, "interactions.back", "Back (pop)")
            )
        }
        TapActionSummary::Other(key) => format!("{prefix} {key}"),
    }
}

pub(crate) fn paint_plain_row(cx: &mut PaintCx<'_>, theme: &Theme, rect: Rect, text: &str) {
    let layout = TextLayout::single_run(
        text,
        crate::widgets::text_metrics::CHROME_FONT_FAMILY,
        12.0,
        (theme.muted_foreground).to_jian(),
        Point2D::new(0.0, 0.0),
    );
    cx.backend.draw_text(
        &layout,
        Point2D::new(rect.origin.x + 2.0, rect.origin.y + 19.0),
    );
}

/// Locale lookup with an English fallback for a key missing from the
/// catalogs (debug builds would otherwise show the raw key).
pub(crate) fn translate(
    locale: de_editor_core::Locale,
    key: &'static str,
    fallback: &'static str,
) -> &'static str {
    let translated = crate::i18n::translate(locale, key);
    if translated == key {
        fallback
    } else {
        translated
    }
}

#[cfg(test)]
#[path = "property_panel_interactions_tests.rs"]
mod tests;
