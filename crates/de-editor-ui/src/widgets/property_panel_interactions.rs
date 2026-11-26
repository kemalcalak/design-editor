//! Interactions section: the read-only screen marker, then one row per
//! authored trigger (Tap / Hover / Long press) with its action, and a
//! Transition row under a navigate (#154). The Motion section
//! (`property_panel_motion.rs`) paints directly below it in the same
//! slot, so its summary rides on [`InteractionSummary::motion`].
//!
//! Rows per trigger:
//! - one action: a dropdown row opening the shared popover (Navigate
//!   to <screen> / Scroll to <section> / Open URL / Back / Change to …
//!   / Remove), plus a Slide | Fade row when the action navigates — the
//!   verb is the screen transition Preview plays (`push` slides,
//!   `replace` cross-fades) — or a URL input row under Open URL (#176);
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
use de_editor_core::screen_marker_edit::ScreenMarkerEdit;
use jian_ops_schema::events::Action;
use jian_ops_schema::node::PenNode;

pub use crate::widgets::property_panel_interactions_menu::{
    interaction_menu_rect, interaction_menu_row_rects, interaction_menu_rows,
    interaction_menu_viewport, paint_interaction_menu, InteractionMenuHit, INTERACTION_MENU_ROW_H,
    INTERACTION_MENU_W,
};
use crate::widgets::property_panel_interactions_overlay::{
    overlay_action_label, overlay_input_focus, overlay_rows, paint_overlay_row, push_overlay_rects,
    OverlayRow,
};
pub use de_editor_core::interaction_edit::{
    InteractionMenuTarget, InteractionTrigger, NavigateVerb,
};

/// One authored action, summarized for display. `Other` covers every
/// action key the panel does not author (`set`, `fetch`, …), shown
/// read-only.
#[derive(Debug, Clone, PartialEq)]
pub enum TapActionSummary {
    Navigate {
        verb: NavigateVerb,
        path: String,
    },
    Pop,
    /// `scroll_to` a section, by node id (#153 / #176).
    ScrollTo {
        target: String,
    },
    /// `open_url`, with its address as typed (#176).
    OpenUrl {
        url: String,
    },
    /// `open_overlay` / `swap_overlay` / `close_overlay` (#372).
    Overlay(de_editor_core::overlay_action::OverlayAction),
    Other(String),
}

/// A section a "Scroll to" action can aim at: a named frame directly
/// under the page root that holds the selection.
#[derive(Debug, Clone, PartialEq)]
pub struct SectionTarget {
    pub id: String,
    pub name: String,
}

/// A routed screen of the selection's page, for the rows' target names
/// (#371).
#[derive(Debug, Clone, PartialEq)]
pub struct ScreenRef {
    pub route: String,
    pub name: String,
    pub id: String,
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
    /// A top-level Frame: it can be marked as a Preview screen (#223).
    pub can_be_screen: bool,
    /// Authored triggers, in [`InteractionTrigger::ALL`] order.
    pub triggers: Vec<TriggerSummary>,
    /// The Motion section's rows (`animations`).
    pub motion: Vec<MotionSummary>,
    /// The node-level `transition`, shown at the top of the Motion
    /// section (#176).
    pub transition: Option<de_editor_core::transition_edit::TransitionSummary>,
    /// "Scroll to" targets for the popover; filled from the document by
    /// the panel builder (the node alone can't know its page).
    pub sections: Vec<SectionTarget>,
    /// The page's routed screens, so a navigate row names its target and
    /// can jump to it (#371); filled by the panel builder.
    pub screens: Vec<ScreenRef>,
    /// The frames an overlay can show, on every page (#372); filled by
    /// the panel builder.
    pub overlay_targets: Vec<de_editor_core::overlay_edit::OverlayTarget>,
}

impl InteractionSummary {
    /// The actions authored for `trigger` (empty when none).
    pub fn actions(&self, trigger: InteractionTrigger) -> &[TapActionSummary] {
        self.triggers
            .iter()
            .find(|t| t.trigger == trigger)
            .map_or(&[], |t| t.actions.as_slice())
    }

    /// The address of `trigger`'s single Open URL action, if that is
    /// what it holds.
    pub fn open_url(&self, trigger: InteractionTrigger) -> Option<String> {
        match self.actions(trigger) {
            [TapActionSummary::OpenUrl { url }] => Some(url.clone()),
            _ => None,
        }
    }

    /// Display name of the section with node id `id` (the id itself when
    /// it is not one of [`Self::sections`]).
    pub fn section_name<'a>(&'a self, id: &'a str) -> &'a str {
        self.sections
            .iter()
            .find(|section| section.id == id)
            .map_or(id, |section| section.name.as_str())
    }

    /// The screen a route leads to, if the page has one.
    pub fn screen_for(&self, route: &str) -> Option<&ScreenRef> {
        self.screens.iter().find(|screen| screen.route == route)
    }

    /// The node `trigger`'s single action leads to, for the row's jump
    /// button (#371): a navigate's screen, a scroll target.
    pub fn target_of(&self, trigger: InteractionTrigger) -> Option<String> {
        match self.actions(trigger) {
            [TapActionSummary::Navigate { path, .. }] => {
                self.screen_for(path).map(|screen| screen.id.clone())
            }
            [TapActionSummary::ScrollTo { target }] => Some(target.clone()),
            // An overlay frame on this page (#372).
            [TapActionSummary::Overlay(action)] => {
                let target = action.target()?;
                self.overlay_targets
                    .iter()
                    .find(|t| t.id == target && t.page.is_none())
                    .map(|t| t.id.clone())
            }
            _ => None,
        }
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
        can_be_screen: is_top_level && matches!(node, PenNode::Frame(_)),
        triggers,
        motion: crate::widgets::property_panel_motion::motion_of(node),
        transition: node
            .motion_declarations()
            .0
            .map(de_editor_core::transition_edit::TransitionSummary::of),
        sections: Vec::new(),
        screens: Vec::new(),
        overlay_targets: Vec::new(),
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
    if let Some(overlay) = de_editor_core::overlay_action::OverlayAction::parse_body(key, body) {
        return TapActionSummary::Overlay(overlay);
    }
    match key {
        "pop" => TapActionSummary::Pop,
        "scroll_to" => match body.get("target").and_then(serde_json::Value::as_str) {
            Some(target) => TapActionSummary::ScrollTo {
                target: target.to_string(),
            },
            None => TapActionSummary::Other(key.to_string()),
        },
        "open_url" => match de_editor_core::interaction_edit::open_url_of(body) {
            Some(url) => TapActionSummary::OpenUrl { url },
            None => TapActionSummary::Other(key.to_string()),
        },
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
/// the popover's "Navigate to <path>" rows, in document order. The screen
/// holding the selection is left out (#190): navigating to the screen
/// you are already on does nothing.
pub fn document_screen_paths(state: &de_editor_core::EditorState) -> Vec<String> {
    let anchor = state.selection.anchor.as_str();
    state
        .active_children()
        .iter()
        .filter_map(|n| n.screen().map(|screen| (n, screen)))
        // Only frames that carry a screen are walked, and only when
        // something is selected.
        .filter(|(n, _)| anchor.is_empty() || !subtree_contains(n, anchor))
        .map(|(_, screen)| screen.to_string())
        .collect()
}

/// The active page's routed screens (top-level frames with a `screen`
/// marker), in document order (#371).
pub fn document_screens(state: &de_editor_core::EditorState) -> Vec<ScreenRef> {
    state
        .active_children()
        .iter()
        .filter_map(|node| {
            let route = node.screen()?;
            let name = node.base().name.as_deref().map(str::trim).unwrap_or("");
            Some(ScreenRef {
                route: route.to_string(),
                name: if name.is_empty() {
                    route.to_string()
                } else {
                    name.to_string()
                },
                id: node.id_str().to_string(),
            })
        })
        .collect()
}

/// The "Scroll to" targets for the selection: the named frames directly
/// under the top-level frame that holds it, except the one holding the
/// selection itself (a header link does not scroll to its own header).
pub fn document_section_targets(state: &de_editor_core::EditorState) -> Vec<SectionTarget> {
    let anchor = state.selection.anchor.as_str();
    let Some(root) = state
        .active_children()
        .iter()
        .find(|node| subtree_contains(node, anchor))
    else {
        return Vec::new();
    };
    root.children()
        .into_iter()
        .flatten()
        .filter(|child| matches!(child, PenNode::Frame(_)) && !subtree_contains(child, anchor))
        .filter_map(|child| {
            let name = child.base().name.as_deref()?.trim();
            (!name.is_empty()).then(|| SectionTarget {
                id: child.id_str().to_string(),
                name: name.to_string(),
            })
        })
        .collect()
}

fn subtree_contains(node: &PenNode, id: &str) -> bool {
    node.id_str() == id
        || node
            .children()
            .into_iter()
            .flatten()
            .any(|child| subtree_contains(child, id))
}

// ── Rows: one list drives height, hit rects and paint ───────────────

/// One painted row of the section.
#[derive(Debug, Clone, PartialEq)]
enum Row<'a> {
    Screen(&'a str),
    /// A marked screen's button (#223): Set as start screen, Remove
    /// screen.
    ScreenMarker(ScreenMarkerEdit),
    /// A trigger's single action — opens the popover.
    Action(InteractionTrigger, &'a TapActionSummary),
    /// The Slide | Fade row under a single navigate.
    Transition(InteractionTrigger, NavigateVerb),
    /// The URL input under a single Open URL.
    Url(InteractionTrigger),
    /// One option row under a single overlay action (#372).
    Overlay(
        InteractionTrigger,
        &'a de_editor_core::overlay_action::OverlayAction,
        OverlayRow,
    ),
    /// One of several actions, read-only.
    ReadOnly(InteractionTrigger, &'a TapActionSummary),
    RemoveAll(InteractionTrigger),
}

const ROW_STEP: f32 = INPUT_HEIGHT + 6.0;

fn rows(summary: &InteractionSummary) -> Vec<Row<'_>> {
    let mut out = Vec::new();
    // Unmarked: "Mark as screen" sits in the header "+" menu, so a plain
    // frame's panel keeps its height.
    if let Some(screen) = summary.screen.as_deref() {
        out.push(Row::Screen(screen));
        if screen != "/" {
            out.push(Row::ScreenMarker(ScreenMarkerEdit::Start));
        }
        out.push(Row::ScreenMarker(ScreenMarkerEdit::Remove));
    }
    for t in &summary.triggers {
        match t.actions.as_slice() {
            [] => {}
            [only] => {
                out.push(Row::Action(t.trigger, only));
                match only {
                    TapActionSummary::Navigate { verb, .. } => {
                        out.push(Row::Transition(t.trigger, *verb));
                    }
                    TapActionSummary::OpenUrl { .. } => out.push(Row::Url(t.trigger)),
                    TapActionSummary::Overlay(action) => out.extend(
                        overlay_rows(action)
                            .into_iter()
                            .map(|row| Row::Overlay(t.trigger, action, row)),
                    ),
                    _ => {}
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

/// Gap between an action row's dropdown and its jump button.
const JUMP_GAP: f32 = 4.0;

/// An action row with a jump button (#371): the dropdown keeps the row
/// minus a square at its right end, which is the button.
fn split_jump(row: Rect) -> (Rect, Rect) {
    let side = row.size.y;
    let dropdown = Rect::xywh(
        row.origin.x,
        row.origin.y,
        row.size.x - side - JUMP_GAP,
        side,
    );
    let jump = Rect::xywh(row.origin.x + row.size.x - side, row.origin.y, side, side);
    (dropdown, jump)
}

/// The Slide | Fade cells of a Transition row at `row`. The cells take
/// the whole row (#190): a "Transition" label column beside them left
/// too little room for the longer translations of both.
fn transition_cells(row: Rect) -> Vec<Rect> {
    crate::widgets::property_panel_segments::segment_rects(row, 2)
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
            Row::Action(trigger, _) => {
                let menu = PropertyPanelAction::ToggleInteractionMenu(
                    InteractionMenuTarget::Trigger(trigger),
                );
                if interactions.target_of(trigger).is_some() {
                    let (dropdown, jump) = split_jump(rect);
                    out.push((menu, dropdown));
                    out.push((PropertyPanelAction::GoToInteractionTarget(trigger), jump));
                } else {
                    out.push((menu, rect));
                }
            }
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
            Row::ScreenMarker(edit) => {
                out.push((PropertyPanelAction::EditScreenMarker(edit), rect));
            }
            Row::Overlay(trigger, action, row) => {
                let spec = match action {
                    de_editor_core::overlay_action::OverlayAction::Open(spec) => Some(spec),
                    _ => None,
                };
                push_overlay_rects(out, trigger, row, rect, spec);
            }
            Row::Screen(_) | Row::ReadOnly(..) | Row::Url(_) => {}
        }
        row_y += ROW_STEP;
    }
    crate::widgets::property_panel_motion::push_motion_action_rects(
        out,
        interactions,
        x,
        y + interactions_section_height(interactions),
        width,
    );
}

/// The text inputs of the section: one URL row per single Open URL.
pub fn interaction_input_rects(
    interactions: &InteractionSummary,
    x: f32,
    y: f32,
    width: f32,
) -> Vec<(de_editor_core::PropertyFocus, Rect)> {
    let mut out = Vec::new();
    let mut row_y = y + SECTION_HEADER_HEIGHT;
    for row in rows(interactions) {
        if let Row::Url(trigger) = row {
            out.push((
                de_editor_core::PropertyFocus::InteractionUrl(trigger),
                row_rect(x, row_y, width),
            ));
        }
        if let Some(focus) = match row {
            Row::Overlay(trigger, _, overlay) => overlay_input_focus(trigger, overlay),
            _ => None,
        } {
            // The hex input leaves room for the swatch at its right.
            let rect = row_rect(x, row_y, width);
            let input = Rect::xywh(
                rect.origin.x,
                rect.origin.y,
                rect.size.x - rect.size.y - 4.0,
                rect.size.y,
            );
            out.push((focus, input));
        }
        row_y += ROW_STEP;
    }
    out
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
                let label = action_label(locale, interactions, trigger, action);
                if interactions.target_of(trigger).is_some() {
                    let (dropdown, jump) = split_jump(rect);
                    paint_fitted_dropdown(cx, theme, dropdown, &label);
                    paint_jump_button(cx, theme, jump);
                } else {
                    paint_fitted_dropdown(cx, theme, rect, &label);
                }
            }
            Row::Transition(_, verb) => paint_transition_row(cx, theme, locale, rect, verb),
            Row::Url(trigger) => {
                let focus = de_editor_core::PropertyFocus::InteractionUrl(trigger);
                let value = interactions.open_url(trigger).unwrap_or_default();
                crate::widgets::property_panel_inputs::paint_input_with_prefix_focused_state(
                    cx,
                    theme,
                    rect,
                    translate(locale, "interactions.url", "URL"),
                    edit.value_for(focus, &value),
                    edit.focus == Some(focus),
                    edit.caret_at(focus),
                    edit.select_all_at(focus),
                    edit.input_at(focus),
                    edit.now_ms,
                );
            }
            Row::ReadOnly(trigger, action) => {
                let label = action_label(locale, interactions, trigger, action);
                paint_plain_row(cx, theme, rect, &label);
            }
            Row::Overlay(trigger, action, row) => paint_overlay_row(
                cx,
                theme,
                locale,
                interactions,
                edit,
                trigger,
                action,
                row,
                rect,
            ),
            Row::RemoveAll(_) => paint_dropdown(
                cx,
                theme,
                rect,
                translate(locale, "interactions.removeAll", "Remove all"),
            ),
            Row::ScreenMarker(edit) => {
                let (key, fallback) = match edit {
                    ScreenMarkerEdit::Mark => ("interactions.markScreen", "Mark as screen"),
                    ScreenMarkerEdit::Start => {
                        ("interactions.setStartScreen", "Set as start screen")
                    }
                    ScreenMarkerEdit::Remove => ("interactions.removeScreen", "Remove screen"),
                };
                paint_dropdown(cx, theme, rect, translate(locale, key, fallback));
            }
        }
        y += ROW_STEP;
    }
    y += 6.0;
    paint_section_divider(cx, theme, x, y, width);
    crate::widgets::property_panel_motion::paint_motion_section(
        cx,
        theme,
        interactions,
        edit,
        locale,
        x,
        y + SECTION_GAP,
        width,
    )
}

/// The square "go to target" button beside an action row.
fn paint_jump_button(cx: &mut PaintCx<'_>, theme: &Theme, rect: Rect) {
    const ICON: f32 = 14.0;
    cx.backend.fill_round_rect(rect, 6.0, theme.muted);
    crate::widgets::icons::draw_icon(
        cx.backend,
        crate::widgets::icons::Icon::ArrowRight,
        Point2D::new(
            rect.origin.x + (rect.size.x - ICON) / 2.0,
            rect.origin.y + (rect.size.y - ICON) / 2.0,
        ),
        ICON,
        theme.foreground,
        1.75,
    );
}

fn paint_transition_row(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    locale: de_editor_core::Locale,
    rect: Rect,
    verb: NavigateVerb,
) {
    let labels = [
        translate(locale, "interactions.slide", "Slide"),
        translate(locale, "interactions.fade", "Fade"),
    ];
    let active = TRANSITION_VERBS.iter().position(|v| *v == verb);
    crate::widgets::property_panel_segments::paint_segment_row(
        cx, theme, rect, &labels, active, 11.0,
    );
}

/// The trigger's row prefix, e.g. `Tap →`.
fn trigger_prefix(locale: de_editor_core::Locale, trigger: InteractionTrigger) -> &'static str {
    match trigger {
        InteractionTrigger::Tap => translate(locale, "interactions.tapPrefix", "Tap →"),
        InteractionTrigger::Hover => translate(locale, "interactions.hoverPrefix", "Hover →"),
        InteractionTrigger::LongPress => {
            translate(locale, "interactions.longPressPrefix", "Long press →")
        }
    }
}

/// Row label for one authored action, e.g. `Tap → Navigate /profile`,
/// `Tap → Scroll to Pricing`, `Hover → Back (pop)` or `Long press → set`
/// (unknown key, read-only). The verb is not repeated here: the
/// Transition row shows it.
pub(crate) fn action_label(
    locale: de_editor_core::Locale,
    interactions: &InteractionSummary,
    trigger: InteractionTrigger,
    action: &TapActionSummary,
) -> String {
    let prefix = trigger_prefix(locale, trigger);
    match action {
        // The screen's name, not only its route (#371).
        TapActionSummary::Navigate { path, .. } => format!(
            "{prefix} {} {}",
            translate(locale, "interactions.navigate", "Navigate"),
            interactions
                .screen_for(path)
                .map_or(path.as_str(), |screen| screen.name.as_str()),
        ),
        TapActionSummary::Pop => {
            format!(
                "{prefix} {}",
                translate(locale, "interactions.back", "Back (pop)")
            )
        }
        TapActionSummary::ScrollTo { target } => format!(
            "{prefix} {} {}",
            translate(locale, "interactions.scrollTo", "Scroll to"),
            interactions.section_name(target)
        ),
        TapActionSummary::OpenUrl { .. } => format!(
            "{prefix} {}",
            translate(locale, "interactions.openUrl", "Open URL")
        ),
        TapActionSummary::Overlay(action) => format!(
            "{prefix} {}",
            overlay_action_label(locale, interactions, action)
        ),
        TapActionSummary::Other(key) => format!("{prefix} {key}"),
    }
}

/// Width a dropdown keeps for its padding and chevron; the label gets the
/// rest.
pub(crate) const DROPDOWN_CHROME_W: f32 = 40.0;
/// Width a plain read-only row keeps for its inset.
pub(crate) const PLAIN_ROW_INSET_W: f32 = 4.0;

/// A dropdown whose label is ellipsized to stay clear of the chevron.
pub(crate) fn paint_fitted_dropdown(cx: &mut PaintCx<'_>, theme: &Theme, rect: Rect, text: &str) {
    let fitted = crate::widgets::text_metrics::fit_chrome(
        cx.backend,
        text,
        rect.size.x - DROPDOWN_CHROME_W,
        12.0,
    );
    paint_dropdown(cx, theme, rect, &fitted);
}

/// A muted read-only row; the text is ellipsized to the row width.
pub(crate) fn paint_plain_row(cx: &mut PaintCx<'_>, theme: &Theme, rect: Rect, text: &str) {
    let text = crate::widgets::text_metrics::fit_chrome(
        cx.backend,
        text,
        rect.size.x - PLAIN_ROW_INSET_W,
        12.0,
    );
    let layout = TextLayout::single_run(
        &text,
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

#[cfg(test)]
#[path = "property_panel_interactions_target_tests.rs"]
mod target_tests;
