//! What the texts of a unit (a screen, a section, a card, a button) are
//! for: which one is the heading the unit is named after, and the distinct
//! role each text gets (`Title`, `Subtitle`, `Price`, `Description`,
//! `Caption`, or its own content), so two texts of one card never share a
//! name. Hidden layers are never read.

use std::collections::HashMap;

use jian_ops_schema::node::container::LayoutMode;
use jian_ops_schema::node::PenNode;

use crate::node_util::{children, is_node_visible, numeric_height};

use super::facts::{is_container, Facts};
use super::money::{is_figure, is_price};
use super::reading::{bounds_in_parent, is_free_form, single_row, sort_reading, to_parent, Bounds};
use super::rules::{font_size, font_weight, layout, text_content, Role};

/// Longest text content used verbatim as a layer name.
const MAX_CONTENT_NAME_CHARS: usize = 40;
/// Longest heading folded into a named role (`Pricing card`).
const MAX_HEADING_WORDS: usize = 3;
const MAX_HEADING_CHARS: usize = 24;
/// Longest text that qualifies a repeated sibling name (`Row (HD export)`).
const MAX_LABEL_CHARS: usize = 32;
/// Longest excerpt of a long text used as its name (before the ellipsis).
const MAX_EXCERPT_CHARS: usize = 32;
/// A screen's first child at most this tall, holding only short texts, is
/// its header band (the preview's rule, `wire_web_nav_links`).
const HEADER_MAX_HEIGHT: f64 = 200.0;
const HEADER_TEXT_MAX_CHARS: usize = 40;

/// One text layer with what the roles read off it.
pub(crate) struct TextInfo<'a> {
    pub node: &'a PenNode,
    pub content: String,
    pub size: f64,
    pub weight: u32,
}

impl<'a> TextInfo<'a> {
    fn of(node: &'a PenNode) -> Option<Self> {
        let content = text_content(node)?;
        (!content.is_empty()).then(|| Self {
            node,
            content,
            size: font_size(node),
            weight: font_weight(node),
        })
    }

    /// Big or heavy enough to read as a heading.
    fn prominent(&self) -> bool {
        self.size >= 20.0 || (self.size >= 16.0 && self.weight >= 600)
    }

    /// Words, not a figure (`$9`, `12k`, `99 TL/ay`) or a unit suffix
    /// (`/mo`): the only texts that may name a container. A number going
    /// on in words (`2024 roadmap`, `5-visit pass`) is words.
    fn wordy(&self) -> bool {
        has_a_word(&self.content) && !is_figure(&self.content)
    }

    fn short(&self) -> bool {
        self.content.chars().count() <= MAX_CONTENT_NAME_CHARS
    }
}

/// Whether some whitespace-separated token of `text` opens with a letter
/// or digit and holds a letter (`Pro`, `价格`, `1Password`; not `/mo` or
/// `—`).
fn has_a_word(text: &str) -> bool {
    text.split_whitespace().any(|word| {
        word.chars().next().is_some_and(char::is_alphanumeric)
            && word.chars().any(char::is_alphabetic)
    })
}

/// Where a walk below a container stops, by what it is for.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Scope {
    /// Handing out text roles: a screen's sections, cards and buttons each
    /// hand out their own (a hero and a pricing section each get a Title).
    Roles,
    /// Finding a heading: only cards and buttons are skipped, so a screen
    /// may be named after the heading in its hero, while a card's heading
    /// never names what holds it.
    Heading,
}

/// Whether `node` (at `depth` below the page) holds texts the walk in
/// `scope` leaves to it.
pub(crate) fn is_boundary(node: &PenNode, depth: usize, scope: Scope, facts: &Facts) -> bool {
    is_container(node) && ((scope == Scope::Roles && depth <= 1) || facts.is_card_or_button(node))
}

/// Whether `node` at `depth` is a unit whose texts get roles together: a
/// screen, a section of one, a card or a button.
pub(crate) fn is_unit(node: &PenNode, depth: usize, facts: &Facts) -> bool {
    is_boundary(node, depth, Scope::Roles, facts)
}

/// A text with its box in the reading container's coordinates (when the
/// container is free-form; `None` in a laid-out one).
struct Placed<'a> {
    info: TextInfo<'a>,
    at: Option<Bounds>,
}

/// The visible texts below `node` in reading order, not descending into
/// the boundaries of `scope`. A free-form container reads its texts by
/// their boxes in its own coordinates (through free-form descendants, so
/// a title in one group and a price in another still read top to bottom);
/// a laid-out child is read as one block at its own box.
fn collect<'a>(
    node: &'a PenNode,
    depth: usize,
    scope: Scope,
    facts: &Facts,
    skip_first: bool,
    out: &mut Vec<Placed<'a>>,
) {
    let free = is_free_form(node);
    let mut items: Vec<Placed<'a>> = Vec::new();
    for (index, kid) in children(node).iter().enumerate() {
        if !is_node_visible(kid) || (skip_first && index == 0) {
            continue;
        }
        let at = free.then(|| bounds_in_parent(kid));
        if let Some(info) = TextInfo::of(kid) {
            items.push(Placed { info, at });
        } else if !is_boundary(kid, depth + 1, scope, facts) {
            let mut inner = Vec::new();
            collect(kid, depth + 1, scope, facts, false, &mut inner);
            let kid_free = is_free_form(kid);
            for mut placed in inner {
                placed.at = match (at, placed.at) {
                    (Some(_), Some(inner_box)) if kid_free => Some(to_parent(kid, inner_box)),
                    (kid_box, _) => kid_box,
                };
                items.push(placed);
            }
        }
    }
    if free {
        let mut keyed: Vec<(Bounds, Placed<'a>)> = items
            .into_iter()
            .map(|placed| (placed.at.expect("free-form boxes"), placed))
            .collect();
        sort_reading(&mut keyed);
        items = keyed.into_iter().map(|(_, placed)| placed).collect();
    }
    out.extend(items);
}

fn texts<'a>(node: &'a PenNode, depth: usize, scope: Scope, facts: &Facts) -> Vec<TextInfo<'a>> {
    let mut out = Vec::new();
    collect(node, depth, scope, facts, false, &mut out);
    out.into_iter().map(|placed| placed.info).collect()
}

fn short_heading(text: &str) -> Option<String> {
    let words = text.split_whitespace().count();
    (words <= MAX_HEADING_WORDS && text.chars().count() <= MAX_HEADING_CHARS)
        .then(|| text.to_string())
}

/// The top-most heading-like text among `texts` (words, not a figure): the
/// first prominent one, else the one text that is larger than every other.
fn pick_heading<'t, 'a>(texts: &'t [TextInfo<'a>]) -> Option<&'t TextInfo<'a>> {
    let words: Vec<&TextInfo> = texts.iter().filter(|t| t.wordy()).collect();
    if let Some(first) = words.iter().find(|t| t.prominent()) {
        return Some(first);
    }
    let largest = words.iter().map(|t| t.size).fold(0.0, f64::max);
    let mut at_largest = words.iter().filter(|t| t.size >= largest);
    match (at_largest.next(), at_largest.next()) {
        (Some(only), None) => Some(only),
        _ => None,
    }
}

/// The heading `node` is named after (`Starter` → `Starter card`), when
/// it is short enough. Prices and other figures never qualify, and the
/// texts of nested cards and buttons are not looked at. Remembered per
/// node: ask [`Facts::heading`].
pub(crate) fn heading_of(node: &PenNode, facts: &Facts) -> Option<String> {
    let texts = texts(node, 0, Scope::Heading, facts);
    short_heading(&pick_heading(&texts)?.content)
}

/// Whether `node` lays its content out side by side: a row, or a
/// free-form box whose children all sit on one reading row.
fn lies_in_a_row(node: &PenNode) -> bool {
    if layout(node) == Some(&LayoutMode::Horizontal) {
        return true;
    }
    if !is_free_form(node) {
        return false;
    }
    let boxes: Vec<Bounds> = children(node)
        .iter()
        .filter(|kid| is_node_visible(kid))
        .map(bounds_in_parent)
        .collect();
    boxes.len() >= 2 && single_row(&boxes)
}

/// `node`, or the container it wraps alone (through every such wrapper).
fn lone_content(mut node: &PenNode) -> &PenNode {
    while let [only] = children(node) {
        if !is_container(only) {
            break;
        }
        node = only;
    }
    node
}

/// The first child of a screen that reads as its header band (the
/// preview's rule, `wire_web_nav_links`, plus a navigation shape): at most
/// `HEADER_MAX_HEIGHT` tall (or, without an authored height, a row), laid
/// out in a row, holding two or more texts none longer than a link label,
/// at most one of them prominent (a logo among links). A wrapper around
/// the row alone (a full-width bar holding a padded row) counts as the row.
/// A heading block (`Pricing` over `Choose a plan`) is no band.
pub(crate) fn header_band(screen: &PenNode, facts: &Facts) -> bool {
    let Some(first) = children(screen).first() else {
        return false;
    };
    let row = lone_content(first);
    if !is_container(first) || !lies_in_a_row(row) {
        return false;
    }
    if numeric_height(first).is_some_and(|height| height > HEADER_MAX_HEIGHT) {
        return false;
    }
    // Read from the row itself: a floating bar is shaped like a card, and
    // a card's texts are its own.
    let labels = texts(row, 1, Scope::Heading, facts);
    labels.len() >= 2
        && labels
            .iter()
            .all(|t| t.content.chars().count() <= HEADER_TEXT_MAX_CHARS)
        && labels.iter().filter(|t| t.prominent()).count() <= 1
}

/// The heading a screen is named after: the first heading-like text of
/// its main content, past a header band (a logo there never names it).
pub(crate) fn screen_heading(screen: &PenNode, facts: &Facts) -> Option<String> {
    let mut placed = Vec::new();
    collect(
        screen,
        0,
        Scope::Heading,
        facts,
        header_band(screen, facts),
        &mut placed,
    );
    let texts: Vec<TextInfo> = placed.into_iter().map(|p| p.info).collect();
    short_heading(&pick_heading(&texts)?.content)
}

/// `node` itself as a heading: a prominent, short text of words.
pub(crate) fn standalone_heading(node: &PenNode) -> Option<String> {
    let info = TextInfo::of(node)?;
    (is_node_visible(node) && info.wordy() && info.prominent())
        .then(|| short_heading(&info.content))
        .flatten()
}

/// The short texts of words that may label `node`, in reading order:
/// qualifiers for a sibling that would otherwise repeat a name
/// (`Row (10 images a month)`).
pub(crate) fn labels(node: &PenNode, facts: &Facts) -> Vec<String> {
    texts(node, 0, Scope::Heading, facts)
        .into_iter()
        .filter(|t| t.wordy() && t.content.chars().count() <= MAX_LABEL_CHARS)
        .map(|t| t.content)
        .collect()
}

/// Whether any visible text below `node` is a price.
pub(crate) fn holds_price(node: &PenNode, facts: &Facts) -> bool {
    texts(node, 0, Scope::Heading, facts)
        .iter()
        .any(|t| is_price(&t.content))
}

/// The first words of a long text, cut at a word boundary, with an
/// ellipsis: how a text is named once its unit's roles are used up.
fn excerpt(content: &str) -> String {
    let mut out = String::new();
    for word in content.split_whitespace() {
        let len = out.chars().count() + usize::from(!out.is_empty()) + word.chars().count();
        if len > MAX_EXCERPT_CHARS && !out.is_empty() {
            break;
        }
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(word);
    }
    if out.chars().count() > MAX_EXCERPT_CHARS {
        out = out.chars().take(MAX_EXCERPT_CHARS).collect();
    }
    out.push('…');
    out
}

/// A text named by what it says: verbatim when short, else an excerpt.
fn by_content(info: &TextInfo<'_>) -> Role {
    if info.short() {
        Role::Content(info.content.clone())
    } else {
        Role::Content(excerpt(&info.content))
    }
}

/// The roles a unit hands out, each at most once.
#[derive(Default)]
struct Slots {
    title: bool,
    subtitle: bool,
    price: bool,
    description: bool,
    caption: bool,
}

fn take(slot: &mut bool) -> bool {
    !std::mem::replace(slot, true)
}

fn text_role_in(info: &TextInfo<'_>, slots: &mut Slots) -> Role {
    if is_price(&info.content) {
        return if take(&mut slots.price) {
            Role::Price
        } else {
            by_content(info)
        };
    }
    if info.wordy() && info.prominent() {
        if take(&mut slots.title) {
            return Role::Title;
        }
        if take(&mut slots.subtitle) {
            return Role::Subtitle;
        }
        return by_content(info);
    }
    if info.short() {
        return Role::Content(info.content.clone());
    }
    let slot = if info.size <= 12.0 {
        (&mut slots.caption, Role::Caption)
    } else {
        (&mut slots.description, Role::Description)
    };
    if take(slot.0) {
        slot.1
    } else {
        by_content(info)
    }
}

/// The role of a text layer on its own, outside any unit. A hidden text,
/// which no unit reads, is named by what it says: it never takes a role a
/// visible sibling holds.
pub(crate) fn text_role(node: &PenNode) -> Option<Role> {
    let info = TextInfo::of(node)?;
    if !is_node_visible(node) {
        return Some(by_content(&info));
    }
    Some(text_role_in(&info, &mut Slots::default()))
}

/// The roles of the texts of one unit, keyed by node address.
#[derive(Default)]
pub(crate) struct UnitRoles(HashMap<*const PenNode, Role>);

impl UnitRoles {
    /// The role `node` got in this unit, if it is one of its texts.
    pub(crate) fn role_of(&self, node: &PenNode) -> Option<Role> {
        self.0.get(&std::ptr::from_ref(node)).cloned()
    }

    /// Hand out the roles of the visible texts of `unit` (at `depth`), in
    /// reading order: the first heading-like text is the title, the next
    /// one the subtitle, the first price the price, the first long text the
    /// description (or caption when small); the rest are named by what they
    /// say. A button's texts are its labels, named by what they say. Texts
    /// of nested units get their roles there.
    pub(crate) fn of(unit: &PenNode, depth: usize, facts: &Facts) -> Self {
        let is_button = facts.is_button(unit);
        let mut slots = Slots::default();
        let roles = texts(unit, depth, Scope::Roles, facts)
            .iter()
            .map(|info| {
                let role = if is_button {
                    by_content(info)
                } else {
                    text_role_in(info, &mut slots)
                };
                (std::ptr::from_ref(info.node), role)
            })
            .collect();
        Self(roles)
    }
}
