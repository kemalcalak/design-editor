//! What the rules ask about a layer again and again (is it a card, a
//! button, does it hold text, what is its heading, does it open with a
//! header band), answered once per layer and remembered for one naming
//! pass. Without it the heading search re-tests every descendant of every
//! container, quadratic in depth.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use jian_ops_schema::node::PenNode;

use crate::node_util::{children, is_node_visible, node_kind};

use super::label_box_rows::{row_facts, table_columns, RowFacts, TableColumns};
use super::rules::{button_role, is_card_shaped, text_content};
use super::text_roles::{header_band_of, heading_of};

type Key = *const PenNode;

fn key(node: &PenNode) -> Key {
    std::ptr::from_ref(node)
}

/// Answers about the layers of one naming pass, keyed by node address.
/// Valid only while the tree it was asked about is borrowed unchanged.
#[derive(Default)]
pub(crate) struct Facts {
    button: RefCell<HashMap<Key, bool>>,
    card: RefCell<HashMap<Key, bool>>,
    has_text: RefCell<HashMap<Key, bool>>,
    heading: RefCell<HashMap<Key, Option<String>>>,
    header_band: RefCell<HashMap<Key, bool>>,
    row: RefCell<HashMap<Key, Rc<RowFacts>>>,
    table: RefCell<HashMap<Key, Rc<TableColumns>>>,
}

fn remember<V: Clone>(
    map: &RefCell<HashMap<Key, V>>,
    node: &PenNode,
    compute: impl FnOnce() -> V,
) -> V {
    if let Some(value) = map.borrow().get(&key(node)) {
        return value.clone();
    }
    // Computed outside the borrow: `compute` may ask about descendants.
    let value = compute();
    map.borrow_mut().insert(key(node), value.clone());
    value
}

/// A frame, group or rectangle with children.
pub(crate) fn is_container(node: &PenNode) -> bool {
    node_kind(node).is_container() && !children(node).is_empty()
}

impl Facts {
    /// A small box around one short label: a button, a badge or a circle
    /// of initials (see `button_role`).
    pub(crate) fn is_button(&self, node: &PenNode) -> bool {
        remember(&self.button, node, || {
            is_container(node) && button_role(node).is_some()
        })
    }

    /// A decorated box holding text that is not a button.
    pub(crate) fn is_card(&self, node: &PenNode) -> bool {
        remember(&self.card, node, || {
            is_container(node) && !self.is_button(node) && is_card_shaped(node, self)
        })
    }

    /// A card or a button: a unit with names of its own inside whatever
    /// holds it.
    pub(crate) fn is_card_or_button(&self, node: &PenNode) -> bool {
        self.is_button(node) || self.is_card(node)
    }

    /// Whether a visible, non-empty text sits anywhere below `node`.
    pub(crate) fn has_text(&self, node: &PenNode) -> bool {
        remember(&self.has_text, node, || {
            children(node)
                .iter()
                .filter(|kid| is_node_visible(kid))
                .any(|kid| {
                    text_content(kid).is_some_and(|text| !text.is_empty()) || self.has_text(kid)
                })
        })
    }

    /// Whether the first child of `screen` is its header band (see
    /// [`header_band_of`]).
    pub(crate) fn header_band(&self, screen: &PenNode) -> bool {
        remember(&self.header_band, screen, || header_band_of(screen, self))
    }

    /// What the row `row` of label boxes is (see [`row_facts`]).
    pub(crate) fn row(&self, row: &PenNode) -> Rc<RowFacts> {
        remember(&self.row, row, || Rc::new(row_facts(row, self)))
    }

    /// How many rows of `table` share each set of columns (see
    /// [`table_columns`]).
    pub(crate) fn table(&self, table: &PenNode) -> Rc<TableColumns> {
        remember(&self.table, table, || Rc::new(table_columns(table, self)))
    }

    /// The heading `node` is named after (see [`heading_of`]).
    pub(crate) fn heading(&self, node: &PenNode) -> Option<String> {
        remember(&self.heading, node, || heading_of(node, self))
    }
}
