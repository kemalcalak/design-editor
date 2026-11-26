//! Pointer and wheel input while overlays are open (#372).
//!
//! The screen under an open overlay takes no input: a press inside the
//! top overlay goes to the overlay's own nodes (their tap / hover
//! actions run through the runtime's task queue, so a navigate, Back,
//! `close_overlay` or another overlay works from inside one), a press
//! outside closes it when the overlay allows that and is blocked
//! otherwise. A wheel over a tall overlay scrolls it.

use jian_core::gesture::pointer::PointerPhase;
use jian_ops_schema::events::ActionList;

use crate::proto_overlay::{OverlayEntry, OverlayHit};
use crate::PreviewSession;

impl PreviewSession {
    /// Route one pointer phase at a scene point to the overlays. `None`
    /// when none is open (the screen gets it); `Some(repaint)` otherwise.
    pub(crate) fn overlay_pointer(
        &mut self,
        pointer_id: u32,
        x: f32,
        y: f32,
        phase: PointerPhase,
    ) -> Option<bool> {
        if self.proto_overlays.stack.is_empty() {
            return None;
        }
        let hit = self.overlay_hit(x, y);
        match phase {
            PointerPhase::Down => {
                self.proto_overlays.presses.insert(pointer_id, hit);
                Some(false)
            }
            PointerPhase::Up => {
                let pressed = self.proto_overlays.presses.remove(&pointer_id);
                if pressed.as_ref() != Some(&hit) {
                    return Some(false);
                }
                Some(match hit {
                    OverlayHit::Inside(Some(id)) => {
                        let list = self
                            .proto_overlays
                            .stack
                            .last()
                            .and_then(|top| top.taps.get(&id).cloned());
                        self.run_overlay_actions("onTap", list, &id)
                    }
                    OverlayHit::Inside(None) => false,
                    OverlayHit::Outside => {
                        let closes = self
                            .proto_overlays
                            .stack
                            .last()
                            .is_some_and(|top| top.spec.close_on_outside_click);
                        closes && self.close_top_overlay()
                    }
                })
            }
            PointerPhase::Hover => {
                let hovered = match &hit {
                    OverlayHit::Inside(id) => id.clone(),
                    OverlayHit::Outside => None,
                };
                if hovered == self.proto_overlays.hovered {
                    return Some(false);
                }
                self.proto_overlays.hovered = hovered.clone();
                let hover_list = hovered.as_ref().and_then(|id| {
                    self.proto_overlays
                        .stack
                        .last()
                        .and_then(|top| hover_handler(top, id))
                });
                if let Some((owner, list)) = hover_list {
                    self.run_overlay_actions("onHoverEnter", Some(list), &owner);
                }
                Some(true)
            }
            PointerPhase::Move => Some(false),
            PointerPhase::Cancel => {
                self.proto_overlays.presses.remove(&pointer_id);
                Some(false)
            }
        }
    }

    /// Where a scene point lands: inside the top overlay (on the nearest
    /// clickable ancestor-or-self of the node under it), or outside.
    pub(crate) fn overlay_hit(&self, x: f32, y: f32) -> OverlayHit {
        let (Some(top), Some(viewport)) =
            (self.proto_overlays.stack.last(), self.overlay_viewport())
        else {
            return OverlayHit::Outside;
        };
        let boxed = self.overlay_box(top, viewport);
        if !contains(boxed, x, y) {
            return OverlayHit::Outside;
        }
        let local = (
            x - boxed.origin.x + top.node.bounds.origin.x,
            y - boxed.origin.y + top.node.bounds.origin.y + top.scroll_y,
        );
        let mut id = deepest(&top.node, local).map(str::to_owned);
        while let Some(current) = id {
            if top.taps.contains_key(&current) {
                return OverlayHit::Inside(Some(current));
            }
            id = top.parents.get(&current).cloned();
        }
        OverlayHit::Inside(None)
    }

    /// Run an overlay node's actions as if the runtime had fired them.
    fn run_overlay_actions(&mut self, hook: &str, list: Option<ActionList>, node_id: &str) -> bool {
        let Some(list) = list else {
            return false;
        };
        let Ok(value) = serde_json::to_value(&list) else {
            return false;
        };
        let _ = self.runtime.spawn_lifecycle(
            hook,
            value,
            Some(node_id.to_owned()),
            serde_json::json!({ "nodeId": node_id }),
        );
        self.sync_prototype_overlays();
        true
    }

    /// A wheel while overlays are open: scroll a tall top overlay under
    /// the pointer. Always consumed (the page under a modal stays put).
    pub(crate) fn overlay_wheel(&mut self, x: f32, y: f32, dy: f32) -> Option<bool> {
        if self.proto_overlays.stack.is_empty() {
            return None;
        }
        let Some(viewport) = self.overlay_viewport() else {
            return Some(true);
        };
        let hit = self.overlay_hit(x, y);
        if hit == OverlayHit::Outside {
            return Some(true);
        }
        let Some(top) = self.proto_overlays.stack.last() else {
            return Some(true);
        };
        let max = self.overlay_scroll_max(top, viewport);
        let next = (top.scroll_y - dy).clamp(0.0, max);
        if let Some(top) = self.proto_overlays.stack.last_mut() {
            if (next - top.scroll_y).abs() > f32::EPSILON {
                top.scroll_y = next;
                self.proto_overlays.changed.set(true);
            }
        }
        Some(true)
    }
}

/// The nearest ancestor-or-self of `id` with a hover handler.
fn hover_handler(entry: &OverlayEntry, id: &str) -> Option<(String, ActionList)> {
    let mut current = Some(id.to_owned());
    while let Some(node) = current {
        if let Some(list) = entry.hovers.get(&node) {
            return Some((node, list.clone()));
        }
        current = entry.parents.get(&node).cloned();
    }
    None
}

fn contains(rect: de_editor_ui::Rect, x: f32, y: f32) -> bool {
    x >= rect.origin.x
        && y >= rect.origin.y
        && x <= rect.origin.x + rect.size.x
        && y <= rect.origin.y + rect.size.y
}

/// The deepest shown node under `point` (the first child is on top).
fn deepest(node: &de_editor_ui::layout_scene::SceneNode, point: (f32, f32)) -> Option<&str> {
    if node.hidden || !contains(node.bounds, point.0, point.1) {
        return None;
    }
    node.children
        .iter()
        .find_map(|child| deepest(child, point))
        .or(Some(node.id.as_str()))
}
