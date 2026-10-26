//! Absolute children pinned by `bottom` / `right` inside a box whose size
//! only layout knows.
//!
//! `apply_position` writes an absolute child's `x` / `y` at map time, from
//! the containing block's size. When that box is sized by layout (a
//! `flex: 1` slide body, a `height: auto` card), the size is a guess and a
//! `bottom: 0` row lands wherever the guess put it, often outside the box.
//! Jian places absolute children from their parent's top-left corner only,
//! so the pin is kept by structure instead: the child moves into an
//! "Anchor" frame that covers its parent (absolute, `fill_container` both
//! ways, which resolves against the parent's box at layout time) and lays
//! it out against the pinned edge, the insets becoming the anchor's
//! padding.

use jian_ops_schema::node::base::PenNodeBase;
use jian_ops_schema::node::container::{
    AlignItems, ContainerProps, JustifyContent, LayoutMode, Padding,
};
use jian_ops_schema::node::{FrameNode, PenNode};
use jian_ops_schema::sizing::{SizingBehavior, SizingKeyword};

use super::layout_heuristics::{has_offset, position_value};
use super::node_access::{node_base_mut, node_height_mut};
use super::MapCtx;
use crate::css::cascade::ComputedStyle;
use crate::import_warning::ImportWarning;

/// How an absolute child is anchored to its parent's box.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Anchor {
    /// `[top, right, bottom, left]` from the parent's outer corner.
    padding: [f64; 4],
    /// Pinned to the right / bottom edge rather than the left / top one.
    end: (bool, bool),
    /// Pinned to top and bottom with an auto height (`inset: 0`): the node
    /// fills the anchor between the insets instead of keeping a guessed
    /// height. Widths are worked out at map time, so only height stretches.
    stretch: bool,
}

/// The anchor `base` (already positioned by `apply_position`) needs, if any:
/// pinned by `bottom` without `top` (or `right` without `left`) on an axis
/// where the parent's size or the node's own (`own_definite`) is not known
/// before layout, so `parent - inset - own` cannot be worked out; or pinned
/// by both `top` and `bottom` with an auto height in such a parent, where
/// the height itself is `parent - top - bottom`.
pub(super) fn plan(
    base: &PenNodeBase,
    style: &ComputedStyle,
    context: &MapCtx<'_>,
    own_definite: (bool, bool),
) -> Option<Anchor> {
    if style.get("position") != Some("absolute") {
        return None;
    }
    // A transform moves the box after layout from where its insets put it;
    // the map-time placement already folds that in, so it stays numeric.
    if style
        .get("transform")
        .is_some_and(|value| value.trim() != "none")
    {
        return None;
    }
    let (parent_x, parent_y) = context.positioned_definite;
    let end_x =
        has_offset(style, "right") && !has_offset(style, "left") && !(parent_x && own_definite.0);
    let end_y =
        has_offset(style, "bottom") && !has_offset(style, "top") && !(parent_y && own_definite.1);
    let stretch = !parent_y
        && has_offset(style, "top")
        && has_offset(style, "bottom")
        && style
            .get("height")
            .is_none_or(|value| value.trim() == "auto");
    if !end_x && !end_y && !stretch {
        return None;
    }
    let length = |name: &str, reference: f64| {
        position_value(style.get(name), style.font_size, reference, context)
            .0
            .unwrap_or(0.0)
    };
    // The parent's padding box starts inside its left / top border; its
    // right / bottom borders are taken to match (the import keeps no
    // per-side record for them).
    let (origin_x, origin_y) = context.positioned_origin;
    let (width, height) = (context.containing_width, context.containing_height);
    let end_inset = |inset: &str, margin: &str, reference: f64, origin: f64| {
        length(inset, reference) + length(margin, width) + origin
    };
    let top = if end_y { 0.0 } else { base.y.unwrap_or(0.0) };
    let left = if end_x { 0.0 } else { base.x.unwrap_or(0.0) };
    let right = if end_x {
        end_inset("right", "margin-right", width, origin_x)
    } else {
        0.0
    };
    let bottom = if end_y || stretch {
        end_inset("bottom", "margin-bottom", height, origin_y)
    } else {
        0.0
    };
    // Padding cannot pull a box past its parent's edge: a badge hung
    // outside a corner (`right: -6px`) keeps its numeric place.
    let padding = [top, right, bottom, left];
    if padding.iter().any(|edge| *edge < 0.0 || !edge.is_finite()) {
        return None;
    }
    Some(Anchor {
        padding,
        end: (end_x, end_y),
        stretch,
    })
}

/// Re-parent `node` into the frame that keeps its pin at layout time.
pub(super) fn wrap(context: &mut MapCtx<'_>, mut node: PenNode, anchor: Option<Anchor>) -> PenNode {
    let Some(Anchor {
        padding,
        end,
        stretch,
    }) = anchor
    else {
        return node;
    };
    if context.node_count.saturating_add(1) > crate::MAX_OUTPUT_NODES {
        context.warn_once(ImportWarning::NodeLimitMapping);
        return node;
    }
    context.node_count += 1;
    // The node is now in flow inside the anchor; the anchor takes its place
    // among the parent's children, so the private carriers the parent's
    // post-passes read (stacking, grid placement, wrap margins) move with it.
    let (explain, theme) = {
        let base = node_base_mut(&mut node);
        base.x = None;
        base.y = None;
        base.constraints = None;
        (base.explain.take(), base.theme.take())
    };
    if stretch {
        if let Some(height) = node_height_mut(&mut node) {
            *height = Some(SizingBehavior::Keyword(SizingKeyword::FillContainer));
        }
    }
    let (end_x, end_y) = end;
    let justify = if end_y {
        JustifyContent::End
    } else {
        JustifyContent::Start
    };
    let align = if end_x {
        AlignItems::End
    } else {
        AlignItems::Start
    };
    PenNode::Frame(FrameNode {
        base: PenNodeBase {
            id: context.generate_id(),
            name: Some("Anchor".to_string()),
            x: Some(0.0),
            y: Some(0.0),
            role: Some("overlay".to_string()),
            explain,
            theme,
            ..Default::default()
        },
        container: ContainerProps {
            width: Some(SizingBehavior::Keyword(SizingKeyword::FillContainer)),
            height: Some(SizingBehavior::Keyword(SizingKeyword::FillContainer)),
            layout: Some(LayoutMode::Vertical),
            justify_content: Some(justify),
            align_items: Some(align),
            padding: Some(Padding::LtrB(padding)),
            ..Default::default()
        },
        children: Some(vec![node]),
        image_search_query: None,
        reusable: None,
        slot: None,
        state: None,
        bindings: None,
        events: None,
        lifecycle: None,
        semantics: None,
        gestures: None,
        route: None,
        screen: None,
        breakpoint: None,
        placeholder: None,
    })
}
