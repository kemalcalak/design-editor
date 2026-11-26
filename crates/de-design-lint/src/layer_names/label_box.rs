//! A small box around one short label is a button, a badge or an avatar.
//! A button is something to press: tall and roomy around its label, wired
//! to a tap, or showing an action icon (`+ Draft`, `Next →`). A badge (tag,
//! chip, pill, step marker) only labels: compact around its text and never
//! pressed. An avatar is a circle of initials.
//!
//! Measured over the scene templates and the shadcn kit: buttons stand
//! 2.5–3.4 font sizes tall with 1.1–1.7 font sizes of side padding (the
//! kit's `sm` button 32 px at 13 px, `md` 40 px at 14 px), badges 1.5–2.4
//! tall with 0.7–1.1 at the sides (the kit badge 24 px at 12 px, `Most
//! popular` 8 / 18 px around 22 px).

use jian_ops_schema::node::container::Padding;
use jian_ops_schema::node::PenNode;

use crate::node_util::{children, corner_radius_numeric, numeric_height, numeric_width, padding};

use super::rules::{font_size, text_content};

/// At most this many font sizes tall (the box's height, or a line plus
/// the vertical padding) reads as a badge.
const BADGE_MAX_HEIGHT_EM: f64 = 2.4;
/// Side padding of at most this many font sizes reads as a badge.
const BADGE_MAX_SIDE_PADDING_EM: f64 = 1.1;
/// Line height of a label without an authored box height, in font sizes.
const LABEL_LINE_EM: f64 = 1.2;

/// The first word of icon names that stand for an action (`plus`,
/// `arrow-right`, `chevron-down`): a box showing one is pressed.
const ACTION_ICONS: &[&str] = &[
    "add", "arrow", "caret", "chevron", "close", "copy", "delete", "download", "edit", "external",
    "filter", "menu", "pencil", "play", "plus", "refresh", "search", "send", "share", "sort",
    "trash", "upload", "x",
];

/// Whether a tap, double tap or long press is wired to `node`.
fn is_tappable(node: &PenNode) -> bool {
    let events = match node {
        PenNode::Frame(n) => n.events.as_ref(),
        PenNode::Group(n) => n.events.as_ref(),
        PenNode::Rectangle(n) => n.events.as_ref(),
        _ => None,
    };
    events.is_some_and(|events| {
        events.on_tap.is_some() || events.on_double_tap.is_some() || events.on_long_press.is_some()
    })
}

/// Whether `node` holds an icon named for an action (`lucide:plus`,
/// `arrow_forward`).
fn shows_action_icon(node: &PenNode) -> bool {
    children(node).iter().any(|kid| {
        let PenNode::IconFont(icon) = kid else {
            return false;
        };
        let name = icon.icon_font_name.rsplit(':').next().unwrap_or_default();
        let first = name
            .split(['-', '_', ' '])
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase();
        ACTION_ICONS.contains(&first.as_str())
    })
}

/// The vertical (top + bottom) and side (left, the larger of left and
/// right) padding of `node`, when it is authored in numbers.
fn padding_em(node: &PenNode) -> Option<(f64, f64)> {
    match padding(node)? {
        Padding::Uniform(all) => Some((all * 2.0, *all)),
        Padding::XY([vertical, side]) => Some((vertical * 2.0, *side)),
        Padding::LtrB([top, right, bottom, left]) => Some((top + bottom, right.max(*left))),
        Padding::Expression(_) => None,
    }
}

/// Whether the box `node` around `label` is a badge: compact around its
/// text, not wired to a tap and showing no action icon.
pub(crate) fn is_badge(node: &PenNode, label: &PenNode) -> bool {
    if is_tappable(node) || shows_action_icon(node) {
        return false;
    }
    let font = font_size(label);
    if font <= 0.0 {
        return false;
    }
    let pads = padding_em(node);
    let height =
        numeric_height(node).or_else(|| pads.map(|(vertical, _)| font * LABEL_LINE_EM + vertical));
    let Some(height) = height else {
        return false;
    };
    let narrow = pads.is_none_or(|(_, side)| side / font <= BADGE_MAX_SIDE_PADDING_EM);
    height / font <= BADGE_MAX_HEIGHT_EM && narrow
}

/// Whether `node` is a circle around one or two capital letters (`JD`),
/// the shape of an avatar without a picture.
pub(crate) fn is_initials_avatar(node: &PenNode, label: &PenNode) -> bool {
    let (Some(w), Some(h)) = (numeric_width(node), numeric_height(node)) else {
        return false;
    };
    let round = (w - h).abs() <= 1.0 && corner_radius_numeric(node) >= w.min(h) / 2.0 - 0.5;
    let initials = text_content(label).is_some_and(|text| {
        let count = text.chars().count();
        (1..=2).contains(&count) && text.chars().all(|c| c.is_alphabetic() && c.is_uppercase())
    });
    round && initials
}
