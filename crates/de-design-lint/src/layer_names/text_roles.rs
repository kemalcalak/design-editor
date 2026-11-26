//! What the texts of a unit (a screen, a section, a card, a button) are
//! for: which one is the heading the unit is named after, and the distinct
//! role each text gets (`Title`, `Subtitle`, `Price`, `Description`,
//! `Caption`, or its own content), so two texts of one card never share a
//! name.

use std::collections::HashMap;

use jian_ops_schema::node::container::LayoutMode;
use jian_ops_schema::node::PenNode;

use crate::node_util::{children, node_kind, node_x, node_y};

use super::rules::{button_role, card_role, font_size, font_weight, layout, text_content, Role};

/// Longest text content used verbatim as a layer name.
const MAX_CONTENT_NAME_CHARS: usize = 40;
/// Longest heading folded into a named role (`Pricing card`).
const MAX_HEADING_WORDS: usize = 3;
const MAX_HEADING_CHARS: usize = 24;
/// Longest text that qualifies a repeated sibling name (`Row (HD export)`).
const MAX_LABEL_CHARS: usize = 32;
/// Longest excerpt of a long text used as its name (before the ellipsis).
const MAX_EXCERPT_CHARS: usize = 32;
/// Letters a figure may carry besides its digits (`12k`, `24h`, `4.9`).
const MAX_FIGURE_LETTERS: usize = 2;
/// Letters a price may carry (`$9 / month`, `€12 per user`).
const MAX_PRICE_LETTERS: usize = 12;

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

    /// Words, not a figure or a unit suffix (`/mo`): the only texts that
    /// may name a container.
    fn wordy(&self) -> bool {
        starts_a_word(&self.content) && !is_figure(&self.content)
    }

    fn short(&self) -> bool {
        self.content.chars().count() <= MAX_CONTENT_NAME_CHARS
    }
}

fn is_currency(c: char) -> bool {
    // `$`, `¢`–`¥`, the Currency Symbols block, the Rial sign.
    matches!(c, '$' | '\u{A2}'..='\u{A5}' | '\u{20A0}'..='\u{20CF}' | '\u{FDFC}')
}

/// Whether some whitespace-separated token of `text` begins with a letter
/// (`Pro`, `价格`; not `/mo` or `—`).
fn starts_a_word(text: &str) -> bool {
    text.split_whitespace()
        .any(|word| word.chars().next().is_some_and(char::is_alphabetic))
}

fn letter_count(text: &str) -> usize {
    text.chars().filter(|c| c.is_alphabetic()).count()
}

/// A number, an amount or a stat (`$9`, `12k`, `99%`, `4.9`): never a
/// heading, never the name of a container.
pub(crate) fn is_figure(text: &str) -> bool {
    text.chars().any(|c| c.is_ascii_digit())
        && (text.chars().any(is_currency) || letter_count(text) <= MAX_FIGURE_LETTERS)
}

/// An amount of money (`$9`, `€12/mo`, `₺99 per month`).
pub(crate) fn is_price(text: &str) -> bool {
    text.chars().any(|c| c.is_ascii_digit())
        && text.chars().any(is_currency)
        && letter_count(text) <= MAX_PRICE_LETTERS
}

/// `node`'s children in reading order: tree order, except that the
/// children of a free-form (unlaid-out) container that all have a `y` are
/// read top to bottom, then left to right.
pub(crate) fn ordered_children(node: &PenNode) -> Vec<&PenNode> {
    let mut kids: Vec<&PenNode> = children(node).iter().collect();
    let free_form = matches!(layout(node), None | Some(LayoutMode::None));
    if free_form && kids.iter().all(|kid| node_y(kid).is_some()) {
        kids.sort_by(|a, b| {
            let key = |n: &PenNode| (node_y(n).unwrap_or(0.0), node_x(n).unwrap_or(0.0));
            key(a)
                .partial_cmp(&key(b))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }
    kids
}

fn is_container(node: &PenNode) -> bool {
    node_kind(node).is_container() && !children(node).is_empty()
}

/// A card or a button: a unit of its own inside whatever holds it.
pub(crate) fn is_card_or_button(node: &PenNode) -> bool {
    is_container(node) && (button_role(node).is_some() || card_role(node).is_some())
}

/// A card that is not a button (a row of filled buttons is not a card list).
pub(crate) fn is_card(node: &PenNode) -> bool {
    is_container(node) && button_role(node).is_none() && card_role(node).is_some()
}

/// Whether `node` at `depth` is a unit whose texts get roles together: a
/// screen, a section of one, a card or a button.
pub(crate) fn is_unit(node: &PenNode, depth: usize) -> bool {
    is_container(node) && (depth <= 1 || is_card_or_button(node))
}

/// Texts below `node` in reading order, not descending into the subtrees
/// `skip` picks (`depth` is the child's depth below the page).
fn collect<'a>(
    node: &'a PenNode,
    depth: usize,
    skip: &dyn Fn(&PenNode, usize) -> bool,
    out: &mut Vec<TextInfo<'a>>,
) {
    for kid in ordered_children(node) {
        if let Some(info) = TextInfo::of(kid) {
            out.push(info);
        } else if !skip(kid, depth + 1) {
            collect(kid, depth + 1, skip, out);
        }
    }
}

/// Texts a container may be named after: everything below it except the
/// insides of cards and buttons, which have names of their own.
fn heading_texts(node: &PenNode) -> Vec<TextInfo<'_>> {
    let mut out = Vec::new();
    collect(node, 0, &|kid, _| is_card_or_button(kid), &mut out);
    out
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
/// texts of nested cards and buttons are not looked at.
pub(crate) fn heading(node: &PenNode) -> Option<String> {
    let texts = heading_texts(node);
    short_heading(&pick_heading(&texts)?.content)
}

/// `node` itself as a heading: a prominent, short text of words.
pub(crate) fn standalone_heading(node: &PenNode) -> Option<String> {
    let info = TextInfo::of(node)?;
    (info.wordy() && info.prominent())
        .then(|| short_heading(&info.content))
        .flatten()
}

/// The short texts of words that may label `node`, in reading order:
/// qualifiers for a sibling that would otherwise repeat a name
/// (`Row (10 images a month)`).
pub(crate) fn labels(node: &PenNode) -> Vec<String> {
    heading_texts(node)
        .into_iter()
        .filter(|t| t.wordy() && t.content.chars().count() <= MAX_LABEL_CHARS)
        .map(|t| t.content)
        .collect()
}

/// Whether any text below `node` is a price.
pub(crate) fn holds_price(node: &PenNode) -> bool {
    let mut out = Vec::new();
    collect(node, 0, &|_, _| false, &mut out);
    out.iter().any(|t| is_price(&t.content))
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

/// The role of a text layer on its own, outside any unit.
pub(crate) fn text_role(node: &PenNode) -> Option<Role> {
    let info = TextInfo::of(node)?;
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

    /// Hand out the roles of the texts of `unit` (at `depth`), top to
    /// bottom: the first heading-like text is the title, the next one the
    /// subtitle, the first price the price, the first long text the
    /// description (or caption when small); the rest are named by what they
    /// say. A button's texts are its labels, named by what they say. Texts
    /// of nested units get their roles there.
    pub(crate) fn of(unit: &PenNode, depth: usize) -> Self {
        let mut texts = Vec::new();
        collect(unit, depth, &|kid, d| is_unit(kid, d), &mut texts);
        let is_button = button_role(unit).is_some();
        let mut slots = Slots::default();
        let roles = texts
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
