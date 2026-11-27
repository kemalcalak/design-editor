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
use super::rules::{font_size, font_weight, layout, text_content};
use super::verbs::calls_to_action;

/// A run of identical boxes this long, filling a row, may be slots.
const SLOT_RUN_MIN: usize = 3;
/// Corners at most this round are square.
const SQUARE_RADIUS: f64 = 0.5;
/// A box at most this thin is a rule between cells, not a cell.
const RULE_MAX: f64 = 2.0;

/// How a cell is sized across, as a table column: a px width (by its bits)
/// or `fill_container`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) enum Column {
    Px(u64),
    Fill,
}

/// The rows of a table that share one set of columns.
#[derive(Default, Clone, Copy)]
pub(crate) struct ColumnRun {
    /// How many rows have these columns.
    pub rows: usize,
    /// Some cell of those rows holds a value (a number, a unit, a price, a
    /// tick).
    pub values: bool,
}

/// The rows of a table by their set of columns.
pub(crate) type TableColumns = HashMap<Vec<Column>, ColumnRun>;

/// What a row of boxes is.
#[derive(Default)]
pub(crate) struct RowFacts {
    /// The row's columns when every box is a square, unstroked container
    /// sized in px or `fill_container`: the shape of a table row.
    pub columns: Option<Vec<Column>>,
    /// Some box's label is a single character (`%`, `7`): a keypad or a
    /// calculator, never a table.
    pub keys: bool,
    /// Some box's label holds a value (see [`is_value`]).
    pub values: bool,
    /// The row is a run of identical, iconless boxes none of which acts
    /// (see [`slot_run`]): slots when what precedes the row says so.
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

/// A value a table cell holds: a number, a unit or a price (`1.62 kg`,
/// `11 hrs`, `$9`), or a tick / cross mark.
fn is_value(label: &str) -> bool {
    label
        .chars()
        .any(|c| c.is_ascii_digit() || matches!(c, '✓' | '✔' | '✗' | '✕' | '✘' | '—'))
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
        || label_text(node).is_some_and(|label| calls_to_action(&label))
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
/// action and none showing an icon (a social sign-in row shows logos).
/// Tabs and footer links share that shape (`DAY | WEEK | MONTH`), so the
/// run is only slots when its labels are placeholders or what precedes
/// the row names its patrons (`label_box_context::patron_run`).
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
                && label_text(kid).is_some()
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
    let labels: Vec<String> = boxes.iter().filter_map(|kid| label_text(kid)).collect();
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
        keys: labels.iter().any(|label| label.chars().count() < 2),
        values: labels.iter().any(|label| is_value(label)),
        slots: slot_run(&boxes),
        label_boxes: children(row)
            .iter()
            .filter(|kid| facts.is_button(kid))
            .count(),
    }
}

/// The rows of `table` by their set of columns.
pub(crate) fn table_columns(table: &PenNode, facts: &Facts) -> TableColumns {
    let mut runs: TableColumns = HashMap::new();
    for row in children(table) {
        let row = facts.row(row);
        if let Some(columns) = row.columns.clone() {
            let run = runs.entry(columns).or_default();
            run.rows += 1;
            run.values |= row.values;
        }
    }
    runs
}
