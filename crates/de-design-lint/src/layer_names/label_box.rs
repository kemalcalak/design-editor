//! A small box around one short label is a button, a badge or an avatar.
//! A button is something to press: tall and roomy around its label, wired
//! to a tap, or showing an action icon (`+ Draft`, `Next →`). A badge (tag,
//! chip, pill, step marker) only labels: compact around its text and never
//! pressed. An avatar is a circle of initials. An authored `role` (the
//! HTML importer writes `button` for `<button>` and `role="button"`)
//! decides first.
//!
//! Measured over the scene templates and the shadcn kit: buttons stand
//! 2.3–3.4 font sizes tall with 0.85–1.7 font sizes of side padding (the
//! Tailwind / shadcn `sm` button, `h-8 px-3 text-sm`, is 32 px at 14 px;
//! the kit's `md` 40 px at 14 px), badges 1.5–2.2 tall with 0.7–1.1 at the
//! sides (the kit badge 24 px at 12 px, `Most popular` 8 / 18 px around
//! 22 px). A square marker around a short label (`01` in a 52 px circle)
//! may stand up to 2.5 font sizes; a square page button (`1` in 36 px at
//! 13 px) stands taller.

use jian_ops_schema::node::container::Padding;
use jian_ops_schema::node::PenNode;

use crate::node_util::{
    base, children, corner_radius_numeric, numeric_height, numeric_width, padding,
};

use super::reading::text_size;
use super::rules::{font_size, text_content};

/// At most this many font sizes tall (the box's height, or a line plus
/// the vertical padding) reads as a badge.
const BADGE_MAX_HEIGHT_EM: f64 = 2.2;
/// A square box around a label of at most `MARKER_MAX_CHARS` may stand
/// this tall and still read as a marker.
const MARKER_MAX_HEIGHT_EM: f64 = 2.5;
const MARKER_MAX_CHARS: usize = 3;
/// Side padding of at most this many font sizes reads as a badge.
const BADGE_MAX_SIDE_PADDING_EM: f64 = 1.1;
/// Line height of a label without an authored box height, in font sizes.
const LABEL_LINE_EM: f64 = 1.2;
/// Room for rounding in the ratios above.
const EPSILON: f64 = 1e-9;

/// The first word of icon names that stand for an action (`plus`,
/// `chevron-down`, `send`): a box showing one is pressed. An `arrow` is an
/// action only when it points along the text (`arrow-right`, `arrow_back`);
/// one pointing up or down marks a trend, and an `x` / `close` mark on a
/// chip removes the chip rather than presses it.
const ACTION_ICONS: &[&str] = &[
    "add", "caret", "chevron", "copy", "delete", "download", "edit", "external", "filter", "menu",
    "pencil", "play", "plus", "refresh", "search", "send", "share", "sort", "trash", "upload",
];
/// Directions that make an `arrow` icon an action.
const ACTION_ARROW_WAYS: &[&str] = &["right", "left", "forward", "back", "next", "prev"];

/// Roles an author (or the HTML importer) gives a pressed box or a label.
const BUTTON_ROLES: &[&str] = &["button", "icon-button", "link"];
const BADGE_ROLES: &[&str] = &["badge", "tag", "chip", "pill"];

/// What a label box is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum LabelBox {
    Button,
    Badge,
    Avatar,
}

/// Whether a tap, double tap or long press is wired to `node`.
pub(super) fn is_tappable(node: &PenNode) -> bool {
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
pub(super) fn shows_action_icon(node: &PenNode) -> bool {
    children(node).iter().any(|kid| {
        let PenNode::IconFont(icon) = kid else {
            return false;
        };
        let name = icon
            .icon_font_name
            .rsplit(':')
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase();
        let mut words = name.split(['-', '_', ' ']);
        match words.next().unwrap_or_default() {
            "arrow" => words.any(|word| ACTION_ARROW_WAYS.contains(&word)),
            first => ACTION_ICONS.contains(&first),
        }
    })
}

/// The vertical (top + bottom) and side (the larger of left and right)
/// padding of `node` in px, when it is authored in numbers.
fn padding_px(node: &PenNode) -> Option<(f64, f64)> {
    match padding(node)? {
        Padding::Uniform(all) => Some((all * 2.0, *all)),
        Padding::XY([vertical, side]) => Some((vertical * 2.0, *side)),
        Padding::LtrB([top, right, bottom, left]) => Some((top + bottom, right.max(*left))),
        Padding::Expression(_) => None,
    }
}

/// The kind an authored `role` gives `node`, if any.
pub(super) fn authored_kind(node: &PenNode) -> Option<LabelBox> {
    let role = base(node).role.as_deref()?.trim().to_ascii_lowercase();
    if BUTTON_ROLES.contains(&role.as_str()) {
        Some(LabelBox::Button)
    } else if BADGE_ROLES.contains(&role.as_str()) {
        Some(LabelBox::Badge)
    } else {
        None
    }
}

/// Whether the box `node` around `label` is a badge: compact around its
/// text, not wired to a tap and showing no action icon. Without authored
/// padding, the side room is what the box's width leaves around the label.
fn is_badge(node: &PenNode, label: &PenNode) -> bool {
    if is_tappable(node) || shows_action_icon(node) {
        return false;
    }
    let font = font_size(label);
    if font <= 0.0 {
        return false;
    }
    let pads = padding_px(node);
    let height =
        numeric_height(node).or_else(|| pads.map(|(vertical, _)| font * LABEL_LINE_EM + vertical));
    let Some(height) = height else {
        return false;
    };
    let side = pads.map(|(_, side)| side).or_else(|| {
        let width = numeric_width(node)?;
        let (label_width, _) = text_size(label)?;
        Some(((width - label_width) / 2.0).max(0.0))
    });
    let narrow = side.is_none_or(|side| side / font <= BADGE_MAX_SIDE_PADDING_EM + EPSILON);
    let chars = text_content(label).map_or(0, |text| text.chars().count());
    let square = numeric_width(node).is_some_and(|width| (width - height).abs() <= 1.0);
    let max_height = if square && chars <= MARKER_MAX_CHARS {
        MARKER_MAX_HEIGHT_EM
    } else {
        BADGE_MAX_HEIGHT_EM
    };
    height / font <= max_height + EPSILON && narrow
}

/// Whether `node` is a circle around one or two letters (`JD`, `王`):
/// capitals, or letters of a script without case. The shape of an avatar
/// without a picture.
fn is_initials_avatar(node: &PenNode, label: &PenNode) -> bool {
    let (Some(w), Some(h)) = (numeric_width(node), numeric_height(node)) else {
        return false;
    };
    let round = (w - h).abs() <= 1.0 && corner_radius_numeric(node) >= w.min(h) / 2.0 - 0.5;
    let initials = text_content(label).is_some_and(|text| {
        let count = text.chars().count();
        (1..=2).contains(&count) && text.chars().all(|c| c.is_alphabetic() && !c.is_lowercase())
    });
    round && initials
}

/// What the box `node` around its one `label` (with `kids` children in
/// all) is: the authored role first, then a circle of initials, a compact
/// badge, else a button.
pub(crate) fn label_box_kind(node: &PenNode, label: &PenNode, kids: usize) -> LabelBox {
    if let Some(kind) = authored_kind(node) {
        return kind;
    }
    if kids == 1 && is_initials_avatar(node, label) {
        LabelBox::Avatar
    } else if is_badge(node, label) {
        LabelBox::Badge
    } else {
        LabelBox::Button
    }
}
