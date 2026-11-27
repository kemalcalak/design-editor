//! What a row of label boxes is, answered once per row (`Facts::row`) and
//! once per table (`Facts::table`), so reading each box with its
//! neighbours stays linear in the boxes of a page (#453).

use std::collections::HashMap;

use jian_ops_schema::node::container::LayoutMode;
use jian_ops_schema::node::PenNode;
use jian_ops_schema::sizing::{SizingBehavior, SizingKeyword};

use crate::node_util::{
    children, corner_radius_numeric, first_fill_color, has_stroke, is_node_visible, node_fills,
    node_kind, numeric_height, numeric_width, stroke, width_sizing, NodeKind,
};

use super::facts::Facts;
use super::label_box::{authored_kind, is_tappable, shows_action_icon};
use super::placeholder::slot_thing;
use super::rules::{font_size, font_weight, layout, text_content};

/// A run of identical boxes this long, filling a row, may be slots.
const SLOT_RUN_MIN: usize = 3;
/// Corners at most this round are square.
const SQUARE_RADIUS: f64 = 0.5;
/// A box at most this thin is a rule between cells, not a cell.
const RULE_MAX: f64 = 2.0;

/// English verbs that open a pressed label (`Copy`, `Get tickets`, `Choose
/// file`). A box whose label starts with one is an action, whatever its
/// neighbours look like.
const VERBS: &[&str] = &[
    "accept",
    "add",
    "apply",
    "attach",
    "back",
    "book",
    "browse",
    "buy",
    "call",
    "cancel",
    "choose",
    "close",
    "confirm",
    "connect",
    "contact",
    "continue",
    "copy",
    "create",
    "decline",
    "delete",
    "discover",
    "donate",
    "download",
    "edit",
    "explore",
    "export",
    "follow",
    "get",
    "go",
    "import",
    "install",
    "join",
    "learn",
    "log",
    "login",
    "next",
    "open",
    "order",
    "pay",
    "pick",
    "play",
    "previous",
    "print",
    "register",
    "remove",
    "reply",
    "reserve",
    "retry",
    "rsvp",
    "save",
    "search",
    "see",
    "select",
    "send",
    "share",
    "shop",
    "show",
    "sign",
    "start",
    "submit",
    "subscribe",
    "try",
    "upgrade",
    "upload",
    "view",
    "visit",
    "watch",
];

/// How a cell is sized across, as a table column: a px width (by its bits)
/// or `fill_container`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) enum Column {
    Px(u64),
    Fill,
}

/// How many rows of a table have each set of columns.
pub(crate) type TableColumns = HashMap<Vec<Column>, usize>;

/// What a row of boxes is.
#[derive(Default)]
pub(crate) struct RowFacts {
    /// The row's columns when every box is a square, unstroked container
    /// sized in px or `fill_container`: the shape of a table row.
    pub columns: Option<Vec<Column>>,
    /// The longest label among the row's boxes, in characters.
    pub longest_label: usize,
    /// Some box is wired to a tap, given a button role or shows an action
    /// icon or a verb: the row is a group of actions.
    pub acts: bool,
    /// The row is a run of identical slot boxes (see [`slot_run`]).
    pub slots: bool,
    /// How many of the row's layers read as buttons or badges on their own.
    pub label_boxes: usize,
}

pub(crate) fn is_row(node: &PenNode) -> bool {
    matches!(layout(node), Some(LayoutMode::Horizontal))
}

/// The one text child of a label box.
pub(crate) fn label_of(node: &PenNode) -> Option<&PenNode> {
    let mut texts = children(node)
        .iter()
        .filter(|kid| node_kind(kid) == NodeKind::Text);
    let label = texts.next()?;
    texts.next().is_none().then_some(label)
}

/// Whether a label opens with an English action verb (`Get tickets`).
pub(crate) fn verb_led(label: &str) -> bool {
    label
        .split(|c: char| !c.is_alphanumeric())
        .find(|word| !word.is_empty())
        .is_some_and(|word| VERBS.contains(&word.to_lowercase().as_str()))
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

/// Whether `node` is sized `fill_container` across (a text by its own
/// width).
pub(crate) fn fills_across(node: &PenNode) -> bool {
    let sizing = match node {
        PenNode::Text(text) => text.width.as_ref(),
        _ => width_sizing(node),
    };
    matches!(
        sizing,
        Some(SizingBehavior::Keyword(SizingKeyword::FillContainer))
    )
}

fn column(node: &PenNode) -> Option<Column> {
    if fills_across(node) {
        return Some(Column::Fill);
    }
    numeric_width(node).map(|px| Column::Px(px.to_bits()))
}

fn label_text(node: &PenNode) -> Option<String> {
    label_of(node).and_then(text_content)
}

/// Whether the box `node` acts on its own: a tap, a button role, an action
/// icon or a verb in its label.
fn acts(node: &PenNode) -> bool {
    is_tappable(node)
        || shows_action_icon(node)
        || authored_kind(node).is_some_and(|kind| kind == super::label_box::LabelBox::Button)
        || label_text(node).is_some_and(|label| verb_led(&label))
}

/// A label set as a wordmark: capitals, at least three letters (`SOUNDX`,
/// `NORTH SHORE`).
fn wordmark(label: &str) -> bool {
    label.chars().filter(|c| c.is_alphabetic()).count() >= 3
        && !label.chars().any(char::is_lowercase)
}

/// The look of a box and its label, which every slot of a run shares: a
/// segmented control or a tab row marks its selection by fill, stroke or
/// type, so its boxes differ.
fn same_look(a: &PenNode, b: &PenNode) -> bool {
    let (Some(la), Some(lb)) = (label_of(a), label_of(b)) else {
        return false;
    };
    numeric_height(a) == numeric_height(b)
        && node_fills(a) == node_fills(b)
        && stroke(a) == stroke(b)
        && font_size(la) == font_size(lb)
        && font_weight(la) == font_weight(lb)
        && first_fill_color(la) == first_fill_color(lb)
}

/// A run of at least three identical boxes filling a row, none of them an
/// action and none showing an icon (a social sign-in row shows logos), each
/// holding content a slot holds: a placeholder or a wordmark (sponsor slots).
fn slot_run(boxes: &[&PenNode]) -> bool {
    let Some(first) = boxes.first() else {
        return false;
    };
    boxes.len() >= SLOT_RUN_MIN
        && boxes.iter().all(|kid| {
            fills_across(kid)
                && !acts(kid)
                && authored_kind(kid).is_none()
                && children(kid).len() == 1
                && same_look(kid, first)
                && label_text(kid)
                    .is_some_and(|label| wordmark(&label) || slot_thing(&label).is_some())
        })
}

/// What the row `row` is.
pub(crate) fn row_facts(row: &PenNode, facts: &Facts) -> RowFacts {
    if !is_row(row) {
        return RowFacts::default();
    }
    let boxes: Vec<&PenNode> = children(row)
        .iter()
        .filter(|kid| is_node_visible(kid) && !is_rule(kid))
        .collect();
    let cells = boxes.len() >= 2
        && boxes.iter().all(|kid| {
            node_kind(kid).is_container()
                && !children(kid).is_empty()
                && is_square(kid)
                && !has_stroke(kid)
        });
    RowFacts {
        columns: cells
            .then(|| boxes.iter().map(|kid| column(kid)).collect())
            .flatten(),
        longest_label: boxes
            .iter()
            .filter_map(|kid| label_text(kid))
            .map(|label| label.chars().count())
            .max()
            .unwrap_or(0),
        acts: boxes.iter().any(|kid| acts(kid)),
        slots: slot_run(&boxes),
        label_boxes: children(row)
            .iter()
            .filter(|kid| facts.is_button(kid))
            .count(),
    }
}

/// How many rows of `table` have each set of columns.
pub(crate) fn table_columns(table: &PenNode, facts: &Facts) -> TableColumns {
    let mut counts = HashMap::new();
    for row in children(table) {
        if let Some(columns) = facts.row(row).columns.clone() {
            *counts.entry(columns).or_insert(0) += 1;
        }
    }
    counts
}
