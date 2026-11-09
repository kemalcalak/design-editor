//! Fixed / absolute boxes that escape an `overflow` clip (#322).
//!
//! A positioned box is clipped by the `overflow` of the boxes between it and
//! its containing block only. Radix renders an open submenu's popper inside
//! the main menu's `overflow-hidden rounded-md` panel, with its containing
//! block above the panel, so the browser draws it beside the panel, uncut.
//! The import keeps a rounded box's clip (`map_element`), which hid the
//! submenu. The extractor marks such a box `escapesClip`; it is taken out of
//! its parent and placed by the first unclipped element above the clipper
//! (or the root), in front of that element's other children.

use jian_ops_schema::node::PenNode;
use serde_json::{Map, Value};

use super::snapshot_stack::{to_front_to_back, PaintBucket};
use super::{Rect, SnapshotCtx};
use crate::mapper::node_access::node_base_mut;

/// One hoisted box waiting for the element that will hold it.
pub(super) struct Escaped {
    node: PenNode,
    bucket: PaintBucket,
    /// Page-absolute box, to place it against its new holder.
    rect: Rect,
    /// A clipping element lies between it and its captured parent: the
    /// next unclipped element up holds it.
    crossed: bool,
}

impl SnapshotCtx<'_> {
    /// `node` mapped from `child`, or `None` when the child escapes its
    /// parent's clip and waits for a holder further up.
    pub(super) fn unless_escaping(
        &mut self,
        child: &Value,
        node: PenNode,
        bucket: PaintBucket,
    ) -> Option<PenNode> {
        let object = child.as_object()?;
        let escapes = object.get("escapesClip").and_then(Value::as_bool) == Some(true);
        match Rect::from_node(object) {
            Some(rect) if escapes => {
                self.escaped.push(Escaped {
                    node,
                    bucket,
                    rect,
                    crossed: false,
                });
                None
            }
            _ => Some(node),
        }
    }

    /// The boxes that escaped below this element (`escaped[mark..]`) and
    /// that it holds, front-to-back and placed against `rect`. A clipping
    /// element passes them up; the root takes whatever is left.
    pub(super) fn hoisted(
        &mut self,
        mark: usize,
        object: &Map<String, Value>,
        rect: Rect,
        root: bool,
    ) -> Vec<PenNode> {
        if self.escaped.len() <= mark {
            return Vec::new();
        }
        if super::page_root::clips(object) && !root {
            for entry in &mut self.escaped[mark..] {
                entry.crossed = true;
            }
            return Vec::new();
        }
        let pending = self.escaped.split_off(mark);
        let (held, waiting): (Vec<Escaped>, Vec<Escaped>) =
            pending.into_iter().partition(|entry| entry.crossed || root);
        self.escaped.extend(waiting);
        let placed = held
            .into_iter()
            .map(|mut entry| {
                let base = node_base_mut(&mut entry.node);
                base.x = Some(entry.rect.x - rect.x);
                base.y = Some(entry.rect.y - rect.y);
                (entry.bucket, entry.node)
            })
            .collect();
        to_front_to_back(placed)
    }
}

#[cfg(test)]
#[path = "snapshot_escape_tests.rs"]
mod tests;
