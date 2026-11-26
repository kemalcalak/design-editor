//! The offline, rule-based namer: a layer's role from its kind, geometry,
//! fills and text, rendered with the role words of a locale.

use de_i18n::Locale;
use jian_ops_schema::node::container::LayoutMode;
use jian_ops_schema::node::text::{FontWeight, TextContent};
use jian_ops_schema::node::PenNode;
use jian_ops_schema::style::PenFill;

use crate::color::{parse_hex_color, relative_luminance};
use crate::node_util::{
    children, corner_radius_numeric, first_fill_color, has_stroke, node_fills, node_kind,
    numeric_height, numeric_width, NodeKind,
};

use super::text_roles::{self, heading, holds_price, is_card};

/// How a container of repeated cards is named: after the heading right
/// before it (`Plans` → `Plans cards`), as pricing cards when every card
/// holds a price, else plainly.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum CardsName {
    Heading(String),
    Pricing,
    Plain,
}

/// What a layer is for, as far as its structure tells.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Role {
    Screen(Option<String>),
    Hero,
    Navigation,
    Footer,
    Section(Option<String>),
    Card(Option<String>),
    /// A container of repeated cards, named for what they hold.
    Cards(CardsName),
    Row,
    Column,
    Container,
    PrimaryButton,
    SecondaryButton,
    Title,
    /// A second heading under the title.
    Subtitle,
    /// An amount of money (`$9 / month`).
    Price,
    /// The body text of a unit.
    Description,
    /// Small print (a long text at 12 px or less).
    Caption,
    /// A short text named by what it says.
    Content(String),
    Icon(Option<String>),
    Picture,
    Avatar,
    Divider,
    Background,
    Dot,
    Illustration,
    Shape,
    InputField,
    Control,
    ProgressBar,
    TabBar,
}

impl Role {
    /// Whether descendants read this layer's name as their section
    /// (`Hero / Title`).
    pub(crate) fn is_section_like(&self) -> bool {
        matches!(
            self,
            Role::Hero | Role::Navigation | Role::Footer | Role::Section(_) | Role::Card(_)
        )
    }

    /// Generic roles that read better with their section in front.
    fn takes_section_prefix(&self) -> bool {
        matches!(
            self,
            Role::Title
                | Role::Subtitle
                | Role::Price
                | Role::Description
                | Role::Caption
                | Role::Picture
                | Role::Illustration
                | Role::Background
                | Role::Shape
        )
    }

    /// The name in `locale`'s words, prefixed with `section` when the role
    /// alone would not say which part of the screen it belongs to.
    pub(crate) fn render(&self, locale: Locale, section: Option<&str>) -> String {
        let word = |key: &'static str| de_i18n::translate(locale, key).to_string();
        let named =
            |key: &'static str, name: &str| de_i18n::translate_with(locale, key, &[("name", name)]);
        let name = match self {
            Role::Screen(Some(heading)) => named("renameLayers.role.screenNamed", heading),
            Role::Screen(None) => word("renameLayers.role.screen"),
            Role::Hero => word("renameLayers.role.hero"),
            Role::Navigation => word("renameLayers.role.navigation"),
            Role::Footer => word("renameLayers.role.footer"),
            Role::Section(Some(heading)) => named("renameLayers.role.sectionNamed", heading),
            Role::Section(None) => word("renameLayers.role.section"),
            Role::Card(Some(heading)) => named("renameLayers.role.cardNamed", heading),
            Role::Card(None) => word("renameLayers.role.card"),
            Role::Cards(CardsName::Heading(heading)) => {
                named("renameLayers.role.cardsNamed", heading)
            }
            Role::Cards(CardsName::Pricing) => word("renameLayers.role.pricingCards"),
            Role::Cards(CardsName::Plain) => word("renameLayers.role.cards"),
            Role::Row => word("renameLayers.role.row"),
            Role::Column => word("renameLayers.role.column"),
            Role::Container => word("renameLayers.role.container"),
            Role::PrimaryButton => word("renameLayers.role.primaryButton"),
            Role::SecondaryButton => word("renameLayers.role.secondaryButton"),
            Role::Title => word("renameLayers.role.title"),
            Role::Subtitle => word("renameLayers.role.subtitle"),
            Role::Price => word("renameLayers.role.price"),
            Role::Description => word("renameLayers.role.description"),
            Role::Caption => word("renameLayers.role.caption"),
            Role::Content(text) => text.clone(),
            Role::Icon(Some(icon)) => named("renameLayers.role.iconNamed", icon),
            Role::Icon(None) => word("renameLayers.role.icon"),
            Role::Picture => word("renameLayers.role.picture"),
            Role::Avatar => word("renameLayers.role.avatar"),
            Role::Divider => word("renameLayers.role.divider"),
            Role::Background => word("renameLayers.role.background"),
            Role::Dot => word("renameLayers.role.dot"),
            Role::Illustration => word("renameLayers.role.illustration"),
            Role::Shape => word("renameLayers.role.shape"),
            Role::InputField => word("renameLayers.role.inputField"),
            Role::Control => word("renameLayers.role.control"),
            Role::ProgressBar => word("renameLayers.role.progressBar"),
            Role::TabBar => word("renameLayers.role.tabBar"),
        };
        match section {
            Some(section) if self.takes_section_prefix() => de_i18n::translate_with(
                locale,
                "renameLayers.role.inSection",
                &[("section", section), ("name", name.as_str())],
            ),
            _ => name,
        }
    }
}

/// Where a layer sits: its parent, its index among its siblings and how
/// deep below the page it is (0 = a top-level layer of the page).
#[derive(Clone, Copy)]
pub(crate) struct Place<'a> {
    pub parent: Option<&'a PenNode>,
    pub index: usize,
    pub depth: usize,
}

/// Collapse every whitespace run to one space and trim the ends.
pub fn collapse_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The plain text of a text layer (styled runs joined), whitespace
/// collapsed; `None` for any other kind.
pub fn text_content(node: &PenNode) -> Option<String> {
    let PenNode::Text(text) = node else {
        return None;
    };
    let raw = match &text.content {
        TextContent::Plain(text) => text.clone(),
        TextContent::Styled(segments) => segments.iter().map(|s| s.text.as_str()).collect(),
    };
    Some(collapse_whitespace(&raw))
}

pub(crate) fn font_size(node: &PenNode) -> f64 {
    match node {
        PenNode::Text(text) => text.font_size.unwrap_or(16.0),
        _ => 0.0,
    }
}

pub(crate) fn font_weight(node: &PenNode) -> u32 {
    let PenNode::Text(text) = node else {
        return 400;
    };
    match &text.font_weight {
        Some(FontWeight::Number(weight)) => *weight,
        Some(FontWeight::Keyword(word)) => match word.to_ascii_lowercase().as_str() {
            "bold" | "bolder" => 700,
            "semibold" | "semi-bold" => 600,
            _ => 400,
        },
        None => 400,
    }
}

pub(crate) fn layout(node: &PenNode) -> Option<&LayoutMode> {
    match node {
        PenNode::Frame(n) => n.container.layout.as_ref(),
        PenNode::Group(n) => n.container.layout.as_ref(),
        PenNode::Rectangle(n) => n.container.layout.as_ref(),
        _ => None,
    }
}

fn has_visible_fill(node: &PenNode) -> bool {
    node_fills(node).is_some_and(|fills| !fills.is_empty())
}

/// Every text layer below `node` (depth-first), with its font size.
fn texts_below(node: &PenNode, out: &mut Vec<(f64, String)>) {
    for child in children(node) {
        if let Some(text) = text_content(child) {
            if !text.is_empty() {
                out.push((font_size(child), text));
            }
        }
        texts_below(child, out);
    }
}

fn is_primary_fill(node: &PenNode) -> bool {
    match node_fills(node).and_then(|fills| fills.first()) {
        Some(PenFill::Solid(_)) => match first_fill_color(node) {
            // A design-variable colour is a brand token, not a white panel.
            Some(color) if color.starts_with('$') => true,
            Some(color) => {
                parse_hex_color(color).is_some_and(|rgb| relative_luminance(rgb) < 0.85)
                    && !mostly_transparent(color)
            }
            None => false,
        },
        Some(_) => true,
        None => false,
    }
}

/// True for an `#RRGGBBAA` colour whose alpha is under one half. Only the
/// alpha pair counts: `#FF0000` or `#008000` end in `00` and are opaque.
fn mostly_transparent(color: &str) -> bool {
    let hex = color.trim_start_matches('#');
    hex.len() == 8 && u8::from_str_radix(&hex[6..8], 16).is_ok_and(|alpha| alpha < 0x80)
}

/// A small filled or outlined box around one short label.
pub(crate) fn button_role(node: &PenNode) -> Option<Role> {
    let kids = children(node);
    if kids.is_empty() || kids.len() > 3 {
        return None;
    }
    if numeric_height(node).is_some_and(|h| h > 72.0)
        || numeric_width(node).is_some_and(|w| w > 480.0)
    {
        return None;
    }
    let labels: Vec<String> = kids.iter().filter_map(text_content).collect();
    let others_are_icons = kids.iter().all(|kid| {
        matches!(
            node_kind(kid),
            NodeKind::Text | NodeKind::IconFont | NodeKind::Path
        )
    });
    let [label] = labels.as_slice() else {
        return None;
    };
    if !others_are_icons
        || label.is_empty()
        || label.split_whitespace().count() > 4
        || label.chars().count() > 32
    {
        return None;
    }
    if is_primary_fill(node) {
        Some(Role::PrimaryButton)
    } else if has_stroke(node) || has_visible_fill(node) {
        Some(Role::SecondaryButton)
    } else {
        None
    }
}

fn covers(node: &PenNode, parent: Option<&PenNode>, share: f64) -> bool {
    let (Some(w), Some(h)) = (numeric_width(node), numeric_height(node)) else {
        return false;
    };
    let Some(parent) = parent else {
        return false;
    };
    match (numeric_width(parent), numeric_height(parent)) {
        (Some(pw), Some(ph)) => w >= pw * share && h >= ph * share,
        _ => false,
    }
}

/// Section of a screen: a wide child of a top-level frame.
fn section_role(node: &PenNode, place: Place<'_>) -> Option<Role> {
    let parent = place.parent?;
    if place.depth != 1 || !matches!(node_kind(parent), NodeKind::Frame | NodeKind::Group) {
        return None;
    }
    let wide = match (numeric_width(node), numeric_width(parent)) {
        (Some(w), Some(pw)) => w >= pw * 0.9,
        _ => matches!(layout(parent), Some(LayoutMode::Vertical)),
    };
    if !wide {
        return None;
    }
    let siblings = children(parent).len();
    let short = numeric_height(node).is_none_or(|h| h <= 120.0);
    let mut texts = Vec::new();
    texts_below(node, &mut texts);
    let largest = texts.iter().map(|(size, _)| *size).fold(0.0, f64::max);
    if place.index == 0
        && short
        && (texts.len() >= 3 || layout(node) == Some(&LayoutMode::Horizontal))
    {
        return Some(Role::Navigation);
    }
    if place.index <= 1 && largest >= 32.0 {
        return Some(Role::Hero);
    }
    if siblings >= 3 && place.index + 1 == siblings && largest <= 16.0 {
        return Some(Role::Footer);
    }
    Some(Role::Section(heading(node)))
}

pub(crate) fn card_role(node: &PenNode) -> Option<Role> {
    let decorated = has_stroke(node) || corner_radius_numeric(node) > 0.0;
    if !(has_visible_fill(node) || has_stroke(node)) || !decorated || children(node).len() < 2 {
        return None;
    }
    let mut texts = Vec::new();
    texts_below(node, &mut texts);
    (!texts.is_empty()).then(|| Role::Card(heading(node)))
}

/// Two or more cards side by side, named for their content, never after
/// one of them: `Plans cards` under a `Plans` heading, `Pricing cards`
/// when every card holds a price, else `Cards`.
fn cards_role(node: &PenNode, place: Place<'_>) -> Option<Role> {
    let kids = children(node);
    if kids.len() < 2 || !kids.iter().all(is_card) {
        return None;
    }
    if let Some(heading) = preceding_heading(place) {
        return Some(Role::Cards(CardsName::Heading(heading)));
    }
    Some(Role::Cards(if kids.iter().all(holds_price) {
        CardsName::Pricing
    } else {
        CardsName::Plain
    }))
}

/// The heading text right before the layer at `place` among its siblings
/// (`Plans` above a row of plan cards).
fn preceding_heading(place: Place<'_>) -> Option<String> {
    let previous = children(place.parent?).get(place.index.checked_sub(1)?)?;
    text_roles::standalone_heading(previous)
}

fn container_role(node: &PenNode, place: Place<'_>) -> Role {
    if let Some(role) = button_role(node) {
        return role;
    }
    if let Some(role) = cards_role(node, place) {
        return role;
    }
    if let Some(role) = section_role(node, place) {
        return role;
    }
    if let Some(role) = card_role(node) {
        return role;
    }
    match layout(node) {
        Some(LayoutMode::Horizontal) => Role::Row,
        Some(LayoutMode::Vertical) => Role::Column,
        _ => Role::Container,
    }
}

fn leaf_shape_role(node: &PenNode, place: Place<'_>) -> Role {
    let (w, h) = (numeric_width(node), numeric_height(node));
    if node_kind(node) == NodeKind::Line {
        return Role::Divider;
    }
    if let (Some(w), Some(h)) = (w, h) {
        if w.min(h) <= 2.0 && w.max(h) >= 16.0 {
            return Role::Divider;
        }
    }
    if covers(node, place.parent, 0.95) {
        return Role::Background;
    }
    match node_kind(node) {
        NodeKind::Ellipse if w.is_some_and(|w| w <= 12.0) => Role::Dot,
        NodeKind::Path if w.into_iter().chain(h).all(|size| size <= 48.0) => Role::Icon(None),
        NodeKind::Path => Role::Illustration,
        _ => Role::Shape,
    }
}

/// The role the rules read off `node`, or `None` for layers they do not
/// name (component instances, notes, embedded browsers, empty text).
pub(crate) fn infer_role(node: &PenNode, place: Place<'_>) -> Option<Role> {
    let kind = node_kind(node);
    if place.depth == 0 && matches!(kind, NodeKind::Frame | NodeKind::Group) {
        return Some(Role::Screen(heading(node)));
    }
    Some(match kind {
        NodeKind::Text => return text_roles::text_role(node),
        NodeKind::Image => {
            let square_small = match (numeric_width(node), numeric_height(node)) {
                (Some(w), Some(h)) => w <= 96.0 && (w - h).abs() <= 4.0,
                _ => false,
            };
            if square_small {
                Role::Avatar
            } else {
                Role::Picture
            }
        }
        NodeKind::IconFont => {
            let PenNode::IconFont(icon) = node else {
                return None;
            };
            let name = icon.icon_font_name.replace(['-', '_'], " ");
            Role::Icon((!name.trim().is_empty()).then(|| name.trim().to_string()))
        }
        NodeKind::TextInput | NodeKind::TextArea | NodeKind::NumberInput | NodeKind::Select => {
            Role::InputField
        }
        NodeKind::Switch | NodeKind::Checkbox | NodeKind::RadioGroup | NodeKind::Slider => {
            Role::Control
        }
        NodeKind::Progress => Role::ProgressBar,
        NodeKind::Tabs => Role::TabBar,
        NodeKind::Frame | NodeKind::Group | NodeKind::Rectangle if !children(node).is_empty() => {
            container_role(node, place)
        }
        NodeKind::Frame
        | NodeKind::Group
        | NodeKind::Rectangle
        | NodeKind::Ellipse
        | NodeKind::Line
        | NodeKind::Polygon
        | NodeKind::Path => leaf_shape_role(node, place),
        NodeKind::Ref
        | NodeKind::Note
        | NodeKind::Context
        | NodeKind::Prompt
        | NodeKind::Browser => return None,
    })
}
