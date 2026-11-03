//! Per-child writes of the auto-layout snapshot pass: sizing modes, leaving
//! absolute placement for flow, and what each child's content decides.

use std::collections::HashMap;

use jian_ops_schema::node::text::TextGrowth;
use jian_ops_schema::node::PenNode;
use jian_ops_schema::sizing::SizeLimits;

use super::{Axis, Fit, ItemKind};
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

/// Hand the node to its parent's layout: no authored position, no
/// constraints (either would keep it absolute inside an auto-layout frame).
pub(in crate::snapshot) fn into_flow(node: &mut PenNode) {
    let base = node_base_mut(node);
    base.x = None;
    base.y = None;
    base.constraints = None;
}
