//! Per-child writes of the auto-layout snapshot pass: sizing modes, leaving
//! absolute placement for flow, and what each child's content decides.

use std::collections::HashMap;

use jian_ops_schema::node::text::{TextGrowth, TextOverflow};
use jian_ops_schema::node::PenNode;
use jian_ops_schema::sizing::SizeLimits;

use super::super::Rect;
use super::{Axis, Fit, Item, ItemKind, EPS};
use crate::mapper::node_access::node_base_mut;

/// Whether `node`'s content decides its `(width, height)`. Frames report
/// what their own arrangement found (`auto_hug`); one-line text hugs both
/// axes, wrapped text only its height; media never does.
pub(in crate::snapshot) fn hug_of(
    auto_hug: &HashMap<String, (bool, bool)>,
    node: &PenNode,
    kind: ItemKind,
) -> (bool, bool) {
    match kind {
        ItemKind::Text { single_line } => (single_line, true),
        ItemKind::Media => (false, false),
        ItemKind::Frame => match node {
            PenNode::Frame(frame) => auto_hug
                .get(&frame.base.id)
                .copied()
                .unwrap_or((false, false)),
            _ => (false, false),
        },
    }
}

/// Write `fit` on `axis`. Text never takes a fixed or filling height (the
/// editor's line box decides it) and drops the absolute import's size
/// floors, so a shorter label shrinks its parent too.
pub(in crate::snapshot) fn set_size(node: &mut PenNode, axis: Axis, fit: Fit, size: f64) {
    let sizing = fit.sizing(size);
    match node {
        PenNode::Frame(frame) => match axis {
            Axis::Horizontal => frame.container.width = Some(sizing),
            Axis::Vertical => frame.container.height = Some(sizing),
        },
        PenNode::Text(text) => {
            text.limits = SizeLimits::default();
            match axis {
                Axis::Horizontal if fit == Fit::Hug => {
                    text.width = None;
                    text.text_growth = Some(TextGrowth::Auto);
                }
                Axis::Horizontal => {
                    text.width = Some(sizing);
                    text.text_growth = Some(TextGrowth::FixedWidth);
                }
                Axis::Vertical => text.height = None,
            }
        }
        // Images, vectors and icons keep the captured box.
        _ => {}
    }
}

/// A cut line: one line at whatever width its box gets, ending in "…"
/// where it runs past it.
pub(in crate::snapshot) fn truncate(node: &mut PenNode) {
    if let PenNode::Text(text) = node {
        text.text_growth = Some(TextGrowth::FixedWidthHeight);
        text.text_overflow = Some(TextOverflow::Ellipsis);
    }
}

/// Hand the node to its parent's layout: no authored position, no
/// constraints (either would keep it absolute inside an auto-layout frame).
pub(in crate::snapshot) fn into_flow(node: &mut PenNode) {
    let base = node_base_mut(node);
    base.x = None;
    base.y = None;
    base.constraints = None;
}

/// Move a placed node's position back by `(dx, dy)`: it was placed against
/// its element and now sits in a frame offset by that much inside it.
pub(in crate::snapshot) fn shift_back(node: &mut PenNode, dx: f64, dy: f64) {
    let base = node_base_mut(node);
    base.x = base.x.map(|x| x - dx);
    base.y = base.y.map(|y| y - dy);
}

/// A vector `path` whose artwork (`art`) sits inset in its element's box
/// (`rect`), wrapped in a fixed frame of that box with the path placed where
/// the art is. The line's spacing was measured between element boxes, and
/// the path cannot carry the inset itself: the renderer fits path data to
/// the node by its bounds.
pub(in crate::snapshot) fn artwork_box(
    id: String,
    mut path: PenNode,
    rect: Rect,
    art: Rect,
) -> PenNode {
    let base = node_base_mut(&mut path);
    let name = base.name.clone();
    // The frame takes the path's place (an overlay, or a child of a frame
    // that falls back to `layout: none`, keeps it): the box, not the art.
    let placed = (
        base.x.map(|x| x - (art.x - rect.x)),
        base.y.map(|y| y - (art.y - rect.y)),
        base.constraints,
    );
    base.x = Some(art.x - rect.x);
    base.y = Some(art.y - rect.y);
    let mut node = PenNode::Frame(super::frame_node(
        id,
        name.as_deref().unwrap_or("icon"),
        super::Laid {
            layout: jian_ops_schema::node::container::LayoutMode::None,
            gap: None,
            padding: None,
            justify: None,
            align: None,
            children: vec![path],
            hug: (false, false),
            natural: None,
        },
    ));
    let frame = node_base_mut(&mut node);
    (frame.x, frame.y, frame.constraints) = placed;
    set_size(&mut node, Axis::Horizontal, Fit::Fixed, rect.w);
    set_size(&mut node, Axis::Vertical, Fit::Fixed, rect.h);
    node
}

/// The element's padding and border, shrunk on any side its flow children
/// reach into. Content that overflows its padding box (a 20 px label in a
/// 32 px button with 8 px padding and a border) still sits inside the box
/// in the browser; keeping the full padding there makes the editor grow
/// the box around it, and that growth travels up every hugging ancestor.
pub(in crate::snapshot) fn fitted_inset(inset: [f64; 4], rect: Rect, flow: &[Item]) -> [f64; 4] {
    let union = union_of(flow);
    let reach = [
        union.y - rect.y,
        rect.x + rect.w - (union.x + union.w),
        rect.y + rect.h - (union.y + union.h),
        union.x - rect.x,
    ];
    let mut fitted = inset;
    for side in 0..4 {
        if reach[side] < inset[side] - EPS {
            fitted[side] = reach[side].max(0.0);
        }
    }
    fitted
}

/// The box the items cover together.
pub(in crate::snapshot) fn union_of(items: &[Item]) -> Rect {
    let low_x = items.iter().map(|i| i.rect.x).fold(f64::INFINITY, f64::min);
    let low_y = items.iter().map(|i| i.rect.y).fold(f64::INFINITY, f64::min);
    let high_x = items
        .iter()
        .map(|i| i.rect.x + i.rect.w)
        .fold(f64::NEG_INFINITY, f64::max);
    let high_y = items
        .iter()
        .map(|i| i.rect.y + i.rect.h)
        .fold(f64::NEG_INFINITY, f64::max);
    Rect {
        x: low_x,
        y: low_y,
        w: high_x - low_x,
        h: high_y - low_y,
    }
}

/// Settle which `truncate` runs the browser cut: those running past the
/// content box's right edge end at it, as the clip showed them. A run that
/// fit is not cut and keeps hugging, as the page lets it grow.
pub(in crate::snapshot) fn cut_at(flow: &mut [Item], content: Rect) {
    let end = content.x + content.w;
    for item in flow.iter_mut().filter(|item| item.cut) {
        item.cut = item.rect.x + item.rect.w > end + EPS;
        if item.cut {
            item.rect.w = (end - item.rect.x).max(0.0);
        }
    }
}
