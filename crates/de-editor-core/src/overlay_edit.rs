//! Overlay authoring (#372): the Interactions panel's overlay rows (target
//! picker, position grid, close-outside checkbox, dim, animation) and the
//! frames an overlay can show.
//!
//! The writes go through `interaction_edit`'s whole-`events` rewrite, so
//! sibling handlers survive and each edit is one undo step.

use jian_ops_schema::events::{Action, ActionList};
use jian_ops_schema::node::PenNode;
use jian_ops_schema::PenDocument;

use crate::interaction_edit::InteractionTrigger;
use crate::overlay_action::{
    OverlayAction, OverlayAnimation, OverlayEdge, OverlayPosition, OverlaySpec,
    DEFAULT_OVERLAY_DIM,
};
use crate::pen_node_ext::PenNodeExt;
use crate::state::EditorState;

/// One option of a trigger's single overlay action.
#[derive(Debug, Clone, PartialEq)]
pub enum OverlayOptionEdit {
    Position(OverlayPosition),
    CloseOnOutsideClick(bool),
    /// `true` adds the default dim (or keeps the current one), `false`
    /// removes it.
    Background(bool),
    /// A typed `#RRGGBBAA`; blank removes the dim, an unparseable value
    /// is refused.
    BackgroundHex(String),
    /// The animation kind; a move-in keeps its edge (bottom when new).
    Animation(OverlayAnimationKind),
    /// The edge of a move-in.
    Direction(OverlayEdge),
}

/// The animation kinds the panel's segments offer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OverlayAnimationKind {
    Instant,
    Dissolve,
    MoveIn,
}

impl OverlayAnimationKind {
    pub const ALL: [OverlayAnimationKind; 3] = [
        OverlayAnimationKind::Instant,
        OverlayAnimationKind::Dissolve,
        OverlayAnimationKind::MoveIn,
    ];

    pub fn of(animation: OverlayAnimation) -> Self {
        match animation {
            OverlayAnimation::Instant => OverlayAnimationKind::Instant,
            OverlayAnimation::Dissolve => OverlayAnimationKind::Dissolve,
            OverlayAnimation::MoveIn(_) => OverlayAnimationKind::MoveIn,
        }
    }
}

/// A frame an overlay can show, for the panel's picker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverlayTarget {
    pub id: String,
    pub name: String,
    /// The page it is on, when that is not the active page.
    pub page: Option<String>,
}

/// The single overlay action of a trigger, if that is what it holds.
pub fn single_overlay(actions: Option<&ActionList>) -> Option<OverlayAction> {
    match actions.map(Vec::as_slice) {
        Some([only]) => OverlayAction::parse(only),
        _ => None,
    }
}

/// An `open_overlay` of `target`, keeping the options of an overlay the
/// trigger already opens (or swaps to: its animation).
pub(crate) fn open_overlay_keeping(current: Option<&ActionList>, target: &str) -> Action {
    let spec = match single_overlay(current) {
        Some(OverlayAction::Open(mut spec)) => {
            spec.target = target.to_owned();
            spec
        }
        Some(OverlayAction::Swap {
            animation,
            duration_ms,
            ..
        }) => OverlaySpec {
            animation,
            duration_ms,
            ..OverlaySpec::new(target)
        },
        _ => OverlaySpec::new(target),
    };
    OverlayAction::Open(spec).to_action()
}

/// A `swap_overlay` to `target`, keeping the animation of an overlay the
/// trigger already opens or swaps to.
pub(crate) fn swap_overlay_keeping(current: Option<&ActionList>, target: &str) -> Action {
    let (animation, duration_ms) = match single_overlay(current) {
        Some(OverlayAction::Open(spec)) => (spec.animation, spec.duration_ms),
        Some(OverlayAction::Swap {
            animation,
            duration_ms,
            ..
        }) => (animation, duration_ms),
        _ => (
            OverlayAnimation::Instant,
            crate::overlay_action::DEFAULT_OVERLAY_DURATION_MS,
        ),
    };
    OverlayAction::Swap {
        target: target.to_owned(),
        animation,
        duration_ms,
    }
    .to_action()
}

/// Apply `edit` to an overlay action; `false` when it does not apply
/// (a swap has only an animation) or the value is refused.
fn apply_option(action: &mut OverlayAction, edit: &OverlayOptionEdit) -> bool {
    let animation = |current: OverlayAnimation, kind: OverlayAnimationKind| match kind {
        OverlayAnimationKind::Instant => OverlayAnimation::Instant,
        OverlayAnimationKind::Dissolve => OverlayAnimation::Dissolve,
        OverlayAnimationKind::MoveIn => match current {
            OverlayAnimation::MoveIn(edge) => OverlayAnimation::MoveIn(edge),
            _ => OverlayAnimation::MoveIn(OverlayEdge::default()),
        },
    };
    match (action, edit) {
        (OverlayAction::Open(spec), OverlayOptionEdit::Position(position)) => {
            spec.position = *position;
        }
        (OverlayAction::Open(spec), OverlayOptionEdit::CloseOnOutsideClick(on)) => {
            spec.close_on_outside_click = *on;
        }
        (OverlayAction::Open(spec), OverlayOptionEdit::Background(on)) => {
            if !on {
                spec.background = None;
            } else if spec.background.is_none() {
                spec.background = Some(DEFAULT_OVERLAY_DIM.to_owned());
            }
        }
        (OverlayAction::Open(spec), OverlayOptionEdit::BackgroundHex(hex)) => {
            let hex = hex.trim();
            if hex.is_empty() {
                spec.background = None;
            } else {
                let parsed = de_util::hex_color::parse_hex_rgba8(
                    hex,
                    de_util::hex_color::HexOptions::LENIENT,
                );
                let Some([r, g, b, a]) = parsed else {
                    return false;
                };
                spec.background = Some(format!("#{r:02X}{g:02X}{b:02X}{a:02X}"));
            }
        }
        (OverlayAction::Open(spec), OverlayOptionEdit::Animation(kind)) => {
            spec.animation = animation(spec.animation, *kind);
        }
        (OverlayAction::Swap { animation: a, .. }, OverlayOptionEdit::Animation(kind)) => {
            *a = animation(*a, *kind);
        }
        (OverlayAction::Open(spec), OverlayOptionEdit::Direction(edge)) => {
            spec.animation = OverlayAnimation::MoveIn(*edge);
        }
        (OverlayAction::Swap { animation: a, .. }, OverlayOptionEdit::Direction(edge)) => {
            *a = OverlayAnimation::MoveIn(*edge);
        }
        _ => return false,
    }
    true
}

impl EditorState {
    /// Change one option of `trigger`'s single overlay action on the
    /// selection. `false` when the trigger holds anything else or nothing
    /// changes.
    pub fn set_selected_overlay_option(
        &mut self,
        trigger: InteractionTrigger,
        edit: &OverlayOptionEdit,
    ) -> bool {
        self.edit_selected_events(|handlers| {
            let slot = trigger.slot(handlers);
            let Some(current) = single_overlay(slot.as_ref()) else {
                return false;
            };
            let mut next = current.clone();
            if !apply_option(&mut next, edit) || next == current {
                return false;
            }
            *slot = Some(vec![next.to_action()]);
            true
        })
    }
}

/// The frames an overlay can show: every page's top-level frames and
/// instances (component masters are left out: an instance stands for
/// them), except the board holding `exclude` on the active page.
pub fn overlay_targets(
    doc: &PenDocument,
    active_page_index: usize,
    exclude: &str,
) -> Vec<OverlayTarget> {
    let pages: Vec<(Option<&str>, &[PenNode])> = match doc.pages.as_deref() {
        Some(pages) if !pages.is_empty() => pages
            .iter()
            .map(|p| (Some(p.name.as_str()), p.children.as_slice()))
            .collect(),
        _ => vec![(None, doc.children.as_slice())],
    };
    let mut out = Vec::new();
    for (index, (page_name, roots)) in pages.iter().enumerate() {
        let active = index == active_page_index.min(pages.len().saturating_sub(1));
        for node in roots.iter() {
            let eligible = match node {
                PenNode::Frame(frame) => frame.reusable != Some(true),
                PenNode::Ref(_) => true,
                _ => false,
            };
            if !eligible || (active && !exclude.is_empty() && subtree_contains(node, exclude)) {
                continue;
            }
            let name = node
                .base()
                .name
                .as_deref()
                .map(str::trim)
                .filter(|n| !n.is_empty())
                .unwrap_or(node.id_str())
                .to_owned();
            out.push(OverlayTarget {
                id: node.id_str().to_owned(),
                name,
                page: (!active).then(|| page_name.unwrap_or_default().to_owned()),
            });
        }
    }
    out
}

/// The node an overlay names, on any page (nested nodes too): its page
/// index (0 for a document without pages) and the node.
pub fn find_overlay_target<'a>(doc: &'a PenDocument, id: &str) -> Option<(usize, &'a PenNode)> {
    let node_id = crate::NodeId::new_opt(id)?;
    match doc.pages.as_deref() {
        Some(pages) if !pages.is_empty() => pages.iter().enumerate().find_map(|(i, page)| {
            crate::walkers::find_node(&page.children, &node_id).map(|node| (i, node))
        }),
        _ => crate::walkers::find_node(&doc.children, &node_id).map(|node| (0, node)),
    }
}

fn subtree_contains(node: &PenNode, id: &str) -> bool {
    node.id_str() == id
        || node
            .children()
            .into_iter()
            .flatten()
            .any(|child| subtree_contains(child, id))
}

#[cfg(test)]
#[path = "overlay_edit_tests.rs"]
mod tests;
