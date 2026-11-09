//! Per-child work of the auto-layout snapshot pass: each child's geometry
//! and role (`item`), sizing modes, leaving absolute placement for flow,
//! and what each child's content decides.

use std::collections::HashMap;

use jian_ops_schema::node::text::{TextGrowth, TextOverflow};
use jian_ops_schema::node::PenNode;
use jian_ops_schema::sizing::SizeLimits;
use serde_json::{Map, Value};

use super::super::snapshot_stack::PaintBucket;
use super::super::{Rect, SnapshotCtx};
use super::lines::line_box;
use super::{style, Axis, Fit, Item, ItemKind, EPS};
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

/// `node`, drawn at `at`, held in a fixed `layout: none` frame of the box
/// `rect` its line spaced, placed inside it where it was drawn.
///
/// Two boxes take this shape: a vector path whose artwork sits inset in its
/// element's box (the path cannot carry the inset: the renderer fits path
/// data to the node by its bounds), and a box a CSS transform moved off its
/// layout box (an avatar pulled up with `-translate-y-1/2`), whose siblings
/// were laid out around the layout box. `fallback` names the frame when the
/// node has no name.
pub(in crate::snapshot) fn held_at(
    id: String,
    fallback: &str,
    mut held: PenNode,
    rect: Rect,
    at: Rect,
) -> PenNode {
    let base = node_base_mut(&mut held);
    let name = base.name.clone();
    // The frame takes the node's place (an overlay, or a child of a frame
    // that falls back to `layout: none`, keeps it): the box, not where the
    // node was drawn.
    let placed = (
        base.x.map(|x| x - (at.x - rect.x)),
        base.y.map(|y| y - (at.y - rect.y)),
        base.constraints,
    );
    base.x = Some(at.x - rect.x);
    base.y = Some(at.y - rect.y);
    let mut node = PenNode::Frame(super::frame_node(
        id,
        name.as_deref().unwrap_or(fallback),
        super::Laid {
            layout: jian_ops_schema::node::container::LayoutMode::None,
            gap: None,
            padding: None,
            justify: None,
            align: None,
            children: vec![held],
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

/// The untransformed layout box (`flowRect`) of an element a CSS transform
/// moved off it, when it differs from the painted box `rect`.
pub(in crate::snapshot) fn moved_from(object: &Map<String, Value>, rect: Rect) -> Option<Rect> {
    let flow = Rect::from_field(object, "flowRect")?;
    let moved = (flow.x - rect.x).abs() > EPS
        || (flow.y - rect.y).abs() > EPS
        || (flow.w - rect.w).abs() > EPS
        || (flow.h - rect.h).abs() > EPS;
    moved.then_some(flow)
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

impl SnapshotCtx<'_> {
    /// The geometry and role of one mapped child.
    pub(super) fn item(
        &mut self,
        child: &Value,
        node: PenNode,
        bucket: PaintBucket,
    ) -> Option<Item> {
        let object = child.as_object()?;
        let rect = Rect::from_node(object)?;
        let overlay = matches!(
            style(object, "position").map(str::trim),
            Some("absolute" | "fixed")
        );
        let grow = object
            .get("layout")
            .and_then(|layout| layout.get("grow"))
            .and_then(Value::as_f64)
            .is_some_and(|grow| grow > 0.0);
        let mut cut = false;
        let (kind, rect) = match &node {
            PenNode::Frame(_) => (ItemKind::Frame, rect),
            PenNode::Text(_) => {
                let lines = object.get("lines").and_then(Value::as_u64).unwrap_or(1);
                let nowrap = style(object, "white-space") == Some("nowrap");
                let single_line = lines <= 1 || nowrap;
                // Whether it actually ran past the box is decided against
                // the content box (`cut_at`).
                cut = single_line && style(object, "text-overflow") == Some("ellipsis");
                let rect = line_box(object, rect, self.opts.base_font_size);
                (ItemKind::Text { single_line }, rect)
            }
            _ => (ItemKind::Media, rect),
        };
        // A vector path is sized to its artwork, which sits inset in the
        // element box (a 16 px icon drawing 10 px of art): the box is what
        // the line spaced, so the path rides in a frame of that box.
        let art = Rect::from_field(object, "vectorRect").filter(|r| r.w > 0.0 && r.h > 0.0);
        let node = match (node, art) {
            (PenNode::Path(path), Some(art)) => {
                let id = self.auxiliary_id("icon");
                held_at(id, "icon", PenNode::Path(path), rect, art)
            }
            (node, _) => node,
        };
        // A box a transform moved (`-translate-y-1/2`, a v4 `translate`)
        // was laid out by its untransformed box: the line is decided by
        // that, and the box keeps its painted place inside a slot of it.
        // A rotation or scale is still drawn as the import always drew it
        // (`apply_base_styles`), only spaced by its layout box. Overlays
        // are placed, not laid out.
        let moved = match kind {
            ItemKind::Text { .. } => None,
            _ if overlay => None,
            _ => moved_from(object, rect),
        };
        let (node, rect, kind) = match moved {
            Some(flow) => {
                let id = self.auxiliary_id("slot");
                (held_at(id, "slot", node, flow, rect), flow, ItemKind::Frame)
            }
            None => (node, rect, kind),
        };
        Some(Item {
            node,
            bucket,
            rect,
            overlay,
            grow,
            kind,
            cut,
        })
    }
}
