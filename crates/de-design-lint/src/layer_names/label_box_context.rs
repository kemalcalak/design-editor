//! A label box read with what sits around it (#453). On its own a filled
//! box around one short label is a button or a badge (`label_box.rs`), but
//! its neighbours often say otherwise:
//!
//! - a table cell is a square, unstroked box in a row of them whose
//!   columns repeat in another row of the same table (a spec table's
//!   `Weight` | `1.62 kg`); a lone row (a dialog's `Cancel` | `Delete`) or a
//!   grid of single characters (a keypad) is no table;
//! - a slot holds a place for content: its label asks for a logo, an image
//!   or the like (`Drop logo`, `placeholder.rs`), or it is one of a run of
//!   identical, iconless wordmark boxes filling a row (sponsor slots);
//! - a box whose row holds no other button or badge labels something
//!   rather than being pressed when it leads a row of text (a stage chip
//!   before its line-up), or is a pill opening with a price (`$50 off at
//!   launch`) or set as a tag in tracked capitals (`PRIMARY LOCKUP`).
//!
//! A tap, a button role, an action icon or a label opening with a verb
//! (`Get tickets`, `Choose file`) keeps the box a button.

use jian_ops_schema::node::container::{AlignItems, JustifyContent, LayoutMode};
use jian_ops_schema::node::text::TextAlign;
use jian_ops_schema::node::PenNode;

use crate::node_util::{
    children, corner_radius_numeric, has_stroke, node_kind, numeric_height, NodeKind,
};

use super::facts::Facts;
use super::label_box::{authored_kind, is_tappable, shows_action_icon};
use super::label_box_rows::{fills_across, is_row, label_of, verb_led};
use super::money::is_price;
use super::placeholder::slot_thing;
use super::rules::{font_size, layout, text_content, Place, Role};

/// A table holds words or figures somewhere; a grid of single characters
/// is a keypad.
const CELL_LABEL_MIN_CHARS: usize = 2;
/// Letter spacing of at least this many font sizes reads as tracked.
const TRACKED_EM: f64 = 0.05;
/// A tag in tracked capitals names something in two or more words
/// (`PRIMARY LOCKUP`, `SOLD OUT`); one word in capitals is as often a
/// call to action (`RSVP`).
const TAG_MIN_WORDS: usize = 2;
/// A corner radius this large rounds any label box into a pill.
const PILL_RADIUS: f64 = 100.0;
/// A price is read from at most this many leading words (`Rp 50.000`).
const PRICE_WORDS: usize = 2;

/// What the box `node` (read alone as `role`) is at `place`.
pub(crate) fn in_place(role: Role, node: &PenNode, place: Place<'_>, facts: &Facts) -> Role {
    let label_box = matches!(
        role,
        Role::PrimaryButton | Role::SecondaryButton | Role::Badge
    );
    if !label_box || authored_kind(node).is_some() || is_tappable(node) || shows_action_icon(node) {
        return role;
    }
    let Some(text) = label_of(node).and_then(text_content) else {
        return role;
    };
    if verb_led(&text) {
        return role;
    }
    if is_table_cell(place, facts) {
        return Role::Cell;
    }
    if !looks_like_input(node) {
        if let Some(thing) = slot_thing(&text) {
            return Role::Slot(Some(thing));
        }
    }
    let row = place.parent.map(|row| facts.row(row));
    if row.as_ref().is_some_and(|row| row.slots && !row.acts) {
        return Role::Slot(None);
    }
    // Another button or badge in the row makes this one part of a group.
    let alone = row.as_ref().is_none_or(|row| row.label_boxes <= 1);
    let button = matches!(role, Role::PrimaryButton | Role::SecondaryButton);
    if button && alone && (leads_a_row(place) || (is_pill(node) && labels_as_pill(node, &text))) {
        return Role::Badge;
    }
    role
}

/// A box in a table row: its row's boxes are square, unstroked containers
/// whose columns repeat in another row of the same table, none of them
/// acts, and some label is longer than a key's.
fn is_table_cell(place: Place<'_>, facts: &Facts) -> bool {
    let (Some(row), Some(table)) = (place.parent, place.grandparent) else {
        return false;
    };
    let facts_of_row = facts.row(row);
    let Some(columns) = facts_of_row.columns.as_ref() else {
        return false;
    };
    !facts_of_row.acts
        && facts_of_row.longest_label >= CELL_LABEL_MIN_CHARS
        && facts
            .table(table)
            .get(columns)
            .is_some_and(|rows| *rows >= 2)
}

/// A stroked box whose label sits at its start: a text field, which keeps
/// its placeholder text (`Your email here`).
fn looks_like_input(node: &PenNode) -> bool {
    has_stroke(node) && !centres_content(node)
}

/// Whether `node` centres its label across.
fn centres_content(node: &PenNode) -> bool {
    let container = match node {
        PenNode::Frame(n) => &n.container,
        PenNode::Group(n) => &n.container,
        PenNode::Rectangle(n) => &n.container,
        _ => return false,
    };
    match layout(node) {
        Some(LayoutMode::Horizontal) => {
            matches!(container.justify_content, Some(JustifyContent::Center))
        }
        Some(LayoutMode::Vertical) => matches!(container.align_items, Some(AlignItems::Center)),
        _ => matches!(
            label_of(node),
            Some(PenNode::Text(text)) if matches!(text.text_align, Some(TextAlign::Center))
        ),
    }
}

/// The box opens a row whose next layer is a text filling the rest: a key
/// before its value (`Main` before the stage's line-up). A file input
/// (`Choose file` before `No file chosen`) is kept by its verb.
fn leads_a_row(place: Place<'_>) -> bool {
    let Some(row) = place.parent else {
        return false;
    };
    is_row(row)
        && place.index == 0
        && children(row)
            .get(1)
            .is_some_and(|next| node_kind(next) == NodeKind::Text && fills_across(next))
}

/// Fully rounded ends.
fn is_pill(node: &PenNode) -> bool {
    let radius = corner_radius_numeric(node);
    radius >= PILL_RADIUS || numeric_height(node).is_some_and(|h| radius >= h / 2.0 - 0.5)
}

/// A pill's label that names rather than asks: a leading price or a tag in
/// tracked capitals.
fn labels_as_pill(node: &PenNode, text: &str) -> bool {
    opens_with_price(text) || label_of(node).is_some_and(|label| tracked_tag(label, text))
}

/// A label that opens with an amount of money (`$50 off at launch`,
/// `Rp 50.000 hemat`, `99 TL indirim`), read by `money.rs` in all 15
/// languages. A count (`3 seats left`, `1-click install`) is no price.
fn opens_with_price(text: &str) -> bool {
    let words: Vec<&str> = text.split_whitespace().collect();
    (1..=PRICE_WORDS.min(words.len())).any(|n| is_price(&words[..n].join(" ")))
}

/// Two or more words in capitals with open letter spacing, the type of a
/// tag (`PRIMARY LOCKUP`).
fn tracked_tag(label: &PenNode, text: &str) -> bool {
    let PenNode::Text(node) = label else {
        return false;
    };
    let font = font_size(label);
    let tracked = node
        .letter_spacing
        .is_some_and(|spacing| font > 0.0 && spacing / font >= TRACKED_EM);
    tracked
        && text.split_whitespace().count() >= TAG_MIN_WORDS
        && text.chars().any(char::is_alphabetic)
        && !text.chars().any(char::is_lowercase)
}
