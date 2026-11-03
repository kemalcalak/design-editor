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
