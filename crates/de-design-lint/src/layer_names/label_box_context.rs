//! A label box read with what sits around it (#453). On its own a filled
//! box around one short label is a button or a badge (`label_box.rs`), but
//! its neighbours often say otherwise:
//!
//! - a table cell is one of a run of square, unstroked boxes filling a row
//!   whose columns line up with the rows beside it (a spec table's
//!   `Weight` | `1.62 kg`);
//! - a slot holds a place for content: its label asks for it (`Drop logo`),
//!   or it is one of a run of identical boxes filling a row (sponsor slots);
//! - a pill whose row holds no button labels something rather than being
//!   pressed when it leads a row of text (a stage chip before its line-up),
//!   opens with an amount (`$50 off at launch`) or is set in tracked capitals
//!   (`PRIMARY LOCKUP`).
//!
//! An authored role, a tap or an action icon keeps the box what it is.

use jian_ops_schema::node::container::LayoutMode;
use jian_ops_schema::node::PenNode;
use jian_ops_schema::sizing::{SizingBehavior, SizingKeyword};

use crate::node_util::{
    children, corner_radius_numeric, has_stroke, is_node_visible, node_fills, node_kind,
    numeric_height, numeric_width, width_sizing, NodeKind,
};

use super::label_box::{authored_kind, is_tappable, shows_action_icon};
use super::rules::{button_role, font_size, layout, text_content, Place, Role};

/// A run of identical boxes this long, filling a row, reads as slots.
const SLOT_RUN_MIN: usize = 3;
/// Slot labels are words, not keys (`1`, `C` on a keypad).
const SLOT_LABEL_MIN_CHARS: usize = 3;
/// Letter spacing of at least this many font sizes reads as tracked.
const TRACKED_EM: f64 = 0.05;
/// A corner radius this large rounds any label box into a pill.
const PILL_RADIUS: f64 = 100.0;
/// Corners at most this round are square.
const SQUARE_RADIUS: f64 = 0.5;
/// A box at most this thin is a rule between cells, not a cell.
const RULE_MAX: f64 = 2.0;

/// Currency signs that may open an amount-led label.
const CURRENCY_SIGNS: &[char] = &['$', '€', '£', '¥', '₹', '₺', '₩', '₽', '₫', '฿', '₱'];

/// What the box `node` (read alone as `role`) is at `place`.
pub(crate) fn in_place(role: Role, node: &PenNode, place: Place<'_>) -> Role {
    let label_box = matches!(
        role,
        Role::PrimaryButton | Role::SecondaryButton | Role::Badge
    );
    if !label_box || authored_kind(node).is_some() || is_tappable(node) || shows_action_icon(node) {
        return role;
    }
    let Some(label) = label_of(node) else {
        return role;
    };
    if is_table_cell(node, place) {
        return Role::Cell;
    }
    if let Some(thing) = text_content(label).as_deref().and_then(placeholder_thing) {
        return Role::Slot(Some(thing));
    }
    if is_slot_run(node, place) {
        return Role::Slot(None);
    }
    let button = matches!(role, Role::PrimaryButton | Role::SecondaryButton);
    if button
        && !row_holds_buttons(node, place)
        && (leads_a_row(place)
            || (is_pill(node) && (opens_with_amount(label) || tracked_capitals(label))))
    {
        return Role::Badge;
    }
    role
}

/// The one text child of a label box.
fn label_of(node: &PenNode) -> Option<&PenNode> {
    let mut texts = children(node)
        .iter()
        .filter(|kid| node_kind(kid) == NodeKind::Text);
    let label = texts.next()?;
    texts.next().is_none().then_some(label)
}

fn is_row(node: &PenNode) -> bool {
    matches!(layout(node), Some(LayoutMode::Horizontal))
}

fn is_square(node: &PenNode) -> bool {
    corner_radius_numeric(node) <= SQUARE_RADIUS
}

/// A thin line between cells (`vertical-rule`, 1 × 72).
fn is_rule(node: &PenNode) -> bool {
    children(node).is_empty()
        && numeric_width(node)
            .into_iter()
            .chain(numeric_height(node))
            .any(|size| size <= RULE_MAX)
}

/// The visible boxes of a row, leaving out the rules between them.
fn row_boxes(row: &PenNode) -> Vec<&PenNode> {
    children(row)
        .iter()
        .filter(|kid| is_node_visible(kid) && !is_rule(kid))
        .collect()
}

/// How a box is sized across: a number, `fill_container` or `fit_content`
/// (anything else, such as a variable, is not compared).
#[derive(PartialEq)]
enum Across {
    Px(f64),
    Fill,
    Fit,
}

fn across(node: &PenNode) -> Option<Across> {
    let sizing = match node {
        PenNode::Text(text) => text.width.as_ref(),
        _ => width_sizing(node),
    };
    match sizing {
        Some(SizingBehavior::Number(px)) if px.is_finite() => Some(Across::Px(*px)),
        Some(SizingBehavior::Keyword(SizingKeyword::FillContainer)) => Some(Across::Fill),
        Some(SizingBehavior::Keyword(SizingKeyword::FitContent)) => Some(Across::Fit),
        _ => None,
    }
}

/// The px column widths of a row of boxes, when every box has one.
fn column_widths(boxes: &[&PenNode]) -> Option<Vec<f64>> {
    boxes.iter().map(|kid| numeric_width(kid)).collect()
}

/// A square, unstroked box among two or more like it filling a table row:
/// every box of the row is a container with square corners and no stroke
/// of its own, and they are all as wide as each other or their columns line
/// up with another row beside this one.
fn is_table_cell(node: &PenNode, place: Place<'_>) -> bool {
    let Some(row) = place.parent else {
        return false;
    };
    if !is_square(node) || has_stroke(node) || !is_row(row) {
        return false;
    }
    let boxes = row_boxes(row);
    let cells = boxes.len() >= 2
        && boxes.iter().all(|kid| {
            node_kind(kid).is_container()
                && !children(kid).is_empty()
                && is_square(kid)
                && !has_stroke(kid)
        });
    if !cells {
        return false;
    }
    // Equal widths fill a row; boxes that each hug their own content
    // (`fit_content`) say nothing about columns.
    let first = across(boxes[0]);
    if matches!(first, Some(Across::Px(_) | Across::Fill))
        && boxes.iter().all(|kid| across(kid) == first)
    {
        return true;
    }
    let Some(widths) = column_widths(&boxes) else {
        return false;
    };
    place.grandparent.is_some_and(|table| {
        children(table).iter().any(|other| {
            !std::ptr::eq(other, row)
                && is_row(other)
                && column_widths(&row_boxes(other)).as_ref() == Some(&widths)
        })
    })
}

/// What a placeholder label asks for: `Drop logo`, `Drop logo here`, `Your
/// logo here` and `Logo placeholder` all hold a `Logo`.
fn placeholder_thing(label: &str) -> Option<String> {
    let lower = label.to_lowercase();
    let thing = if let Some(rest) = lower.strip_prefix("drop ") {
        rest.strip_suffix(" here").unwrap_or(rest)
    } else if let Some(rest) = lower.strip_suffix(" placeholder") {
        rest
    } else {
        lower.strip_prefix("your ")?.strip_suffix(" here")?
    };
    let words = thing.split_whitespace().count();
    if !(1..=2).contains(&words) || !thing.chars().all(|c| c.is_alphabetic() || c == ' ') {
        return None;
    }
    let mut chars = thing.chars();
    let first = chars.next()?;
    Some(first.to_uppercase().chain(chars).collect())
}

/// One of a run of identical boxes filling a row: at least three, each a
/// label box as wide, as tall and filled the same as `node`, each holding
/// a word rather than a key. Buttons of a group differ (one is selected or
/// primary); slots do not.
fn is_slot_run(node: &PenNode, place: Place<'_>) -> bool {
    let Some(row) = place.parent else {
        return false;
    };
    if !is_row(row) || across(node) != Some(Across::Fill) {
        return false;
    }
    let boxes = row_boxes(row);
    boxes.len() >= SLOT_RUN_MIN
        && boxes.iter().all(|kid| {
            across(kid) == Some(Across::Fill)
                && numeric_height(kid) == numeric_height(node)
                && node_fills(kid) == node_fills(node)
                && !is_tappable(kid)
                && !shows_action_icon(kid)
                && label_of(kid)
                    .and_then(text_content)
                    .is_some_and(|text| text.chars().count() >= SLOT_LABEL_MIN_CHARS)
        })
}

/// Whether another box in `node`'s row reads as a button: then this one is
/// part of a group of actions.
fn row_holds_buttons(node: &PenNode, place: Place<'_>) -> bool {
    place.parent.is_some_and(|row| {
        children(row).iter().any(|kid| {
            !std::ptr::eq(kid, node)
                && matches!(
                    button_role(kid),
                    Some(Role::PrimaryButton | Role::SecondaryButton)
                )
        })
    })
}

/// The box opens a row whose next layer is a text filling the rest: a key
/// before its value (`Main` before the stage's line-up).
fn leads_a_row(place: Place<'_>) -> bool {
    let Some(row) = place.parent else {
        return false;
    };
    let kids = children(row);
    is_row(row)
        && place.index == 0
        && kids.get(1).is_some_and(|next| {
            node_kind(next) == NodeKind::Text && across(next) == Some(Across::Fill)
        })
}

/// Fully rounded ends.
fn is_pill(node: &PenNode) -> bool {
    let radius = corner_radius_numeric(node);
    radius >= PILL_RADIUS || numeric_height(node).is_some_and(|h| radius >= h / 2.0 - 0.5)
}

/// A label that opens with an amount (`$50 off at launch`, `20% off`).
fn opens_with_amount(label: &PenNode) -> bool {
    text_content(label)
        .and_then(|text| text.chars().next())
        .is_some_and(|first| first.is_ascii_digit() || CURRENCY_SIGNS.contains(&first))
}

/// A label in capitals with open letter spacing, the type of a tag
/// (`PRIMARY LOCKUP`).
fn tracked_capitals(label: &PenNode) -> bool {
    let PenNode::Text(text) = label else {
        return false;
    };
    let font = font_size(label);
    let tracked = text
        .letter_spacing
        .is_some_and(|spacing| font > 0.0 && spacing / font >= TRACKED_EM);
    let content = text_content(label).unwrap_or_default();
    let letters = content.chars().filter(|c| c.is_alphabetic()).count();
    tracked && letters >= 3 && !content.chars().any(char::is_lowercase)
}
