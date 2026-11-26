//! Where open overlays sit and how they paint (#372).
//!
//! Overlays are fixed to the screen's viewport: the visible part of the
//! device screen in a device frame, the screen board on the Canvas. Each
//! one becomes scene nodes (its dim, then its content, translated into
//! place) composed on top of the screen, so every paint path — the
//! canvas, the device frame's last pass, `capture_preview` — shows the
//! same thing hit-testing uses. A tall overlay is clipped to the viewport
//! and scrolls inside itself.

use de_editor_core::overlay_action::{OverlayAnimation, OverlayEdge};
use de_editor_ui::layout_scene::{LayoutScene, NodeKind, SceneNode, ScenePage};
use de_editor_ui::{Color, Point2D, Rect};

use crate::proto_overlay::OverlayEntry;
use crate::PreviewSession;

/// How far an animated overlay has arrived, 0..=1, eased out.
pub(crate) fn progress(entry: &OverlayEntry, now_ms: u64) -> f32 {
    if entry.spec.animation == OverlayAnimation::Instant || entry.spec.duration_ms == 0 {
        return 1.0;
    }
    let t = now_ms.saturating_sub(entry.opened_at_ms) as f32 / entry.spec.duration_ms as f32;
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

impl PreviewSession {
    /// The rect overlays are placed in: the last painted viewport, else
    /// the screen board.
    pub(crate) fn overlay_viewport(&self) -> Option<Rect> {
        self.proto_overlays
            .viewport
            .get()
            .or_else(|| self.framed_root().map(|(_, rect)| rect))
    }

    /// The top-left of `entry` in scene space (before animation).
    pub(crate) fn overlay_origin(&self, entry: &OverlayEntry, viewport: Rect) -> (f32, f32) {
        let size = entry.node.bounds.size;
        let (x, y) = entry.spec.position.place(
            (
                viewport.origin.x,
                viewport.origin.y,
                viewport.size.x,
                viewport.size.y,
            ),
            (size.x, size.y),
            entry.trigger_bottom_left,
            entry.spec.offset,
        );
        // A tall overlay starts at the viewport's top and scrolls.
        if entry.spec.position.anchor().is_some() && size.y > viewport.size.y {
            return (x, viewport.origin.y);
        }
        (x, y)
    }

    /// The scene-space box `entry` shows in: its placed size, cut to the
    /// viewport's height when it is taller.
    pub(crate) fn overlay_box(&self, entry: &OverlayEntry, viewport: Rect) -> Rect {
        let (x, y) = self.overlay_origin(entry, viewport);
        let size = entry.node.bounds.size;
        Rect::xywh(
            x,
            y,
            size.x,
            size.y.min(self.overlay_visible_h(entry, viewport)),
        )
    }

    fn overlay_visible_h(&self, entry: &OverlayEntry, viewport: Rect) -> f32 {
        let size = entry.node.bounds.size;
        if entry.spec.position.anchor().is_some() {
            size.y.min(viewport.size.y)
        } else {
            size.y
        }
    }

    /// How far a tall overlay can scroll.
    pub(crate) fn overlay_scroll_max(&self, entry: &OverlayEntry, viewport: Rect) -> f32 {
        (entry.node.bounds.size.y - self.overlay_visible_h(entry, viewport)).max(0.0)
    }

    /// The overlays as scene nodes in paint order (the first child paints
    /// on top): the top overlay's content, its dim, then the ones below.
    pub(crate) fn overlay_scene_nodes(&self, viewport: Rect, now_ms: u64) -> Vec<SceneNode> {
        let mut out = Vec::new();
        for (index, entry) in self.proto_overlays.stack.iter().enumerate() {
            let p = progress(entry, now_ms);
            if let Some([r, g, b, a]) = entry.spec.background_rgba() {
                let mut dim = SceneNode::leaf(
                    format!("{}#overlay-dim-{index}", entry.spec.target),
                    NodeKind::Frame,
                );
                dim.bounds = viewport;
                dim.fill = Some(Color { r, g, b, a: a * p });
                out.push(dim);
            }
            out.push(self.overlay_content_node(entry, viewport, p));
        }
        out.reverse();
        out
    }

    /// The content of `entry`, placed, scrolled, clipped and animated.
    fn overlay_content_node(&self, entry: &OverlayEntry, viewport: Rect, p: f32) -> SceneNode {
        let boxed = self.overlay_box(entry, viewport);
        let (mut dx, mut dy) = (boxed.origin.x, boxed.origin.y);
        if let OverlayAnimation::MoveIn(edge) = entry.spec.animation {
            let rest = 1.0 - p;
            match edge {
                OverlayEdge::Bottom => {
                    dy += rest * (viewport.origin.y + viewport.size.y - boxed.origin.y)
                }
                OverlayEdge::Top => {
                    dy -= rest * (boxed.origin.y + boxed.size.y - viewport.origin.y)
                }
                OverlayEdge::Right => {
                    dx += rest * (viewport.origin.x + viewport.size.x - boxed.origin.x)
                }
                OverlayEdge::Left => {
                    dx -= rest * (boxed.origin.x + boxed.size.x - viewport.origin.x)
                }
            }
        }
        let mut node = translated(&entry.node, dx, dy - entry.scroll_y);
        if entry.spec.animation == OverlayAnimation::Dissolve {
            node.composite_opacity *= p;
        }
        if boxed.size.y + 0.5 >= entry.node.bounds.size.y {
            return node;
        }
        // Taller than the viewport: a clipping box the content scrolls in.
        let mut clip = SceneNode::leaf(
            format!("{}#overlay-clip", entry.spec.target),
            NodeKind::Frame,
        );
        clip.bounds = Rect::xywh(dx, dy, boxed.size.x, boxed.size.y);
        clip.clip_content = true;
        clip.children = vec![node];
        clip
    }

    /// Put the overlays on top of the screen board in `scene` (the Canvas
    /// paint and `capture_preview`), placed in the board's rect.
    pub(crate) fn compose_prototype_overlays(&self, scene: &mut LayoutScene, now_ms: u64) {
        if self.proto_overlays.stack.is_empty() {
            return;
        }
        let Some((root_id, root_rect)) = self.framed_root() else {
            return;
        };
        self.proto_overlays.viewport.set(Some(root_rect));
        let nodes = self.overlay_scene_nodes(root_rect, now_ms);
        let index = scene.active_page_index;
        let Some(root) = scene
            .pages
            .get_mut(index)
            .and_then(|page| page.children.iter_mut().find(|node| node.id == root_id))
        else {
            return;
        };
        root.aggregate_bounds_cache = Rect::ZERO;
        root.children.splice(0..0, nodes);
    }

    /// The device frame's last pass: overlays over the whole screen
    /// (pinned strips included), placed in the screen's visible rect.
    /// `frame` is that rect on screen; `content_origin` / `fit` map scene
    /// to screen like the content pass.
    pub(crate) fn framed_overlay_page(
        &self,
        frame: Rect,
        content_origin: Point2D,
        fit: f32,
        now_ms: u64,
    ) -> Option<(Rect, ScenePage)> {
        if self.proto_overlays.stack.is_empty() || fit <= 0.0 {
            return None;
        }
        let viewport = Rect::xywh(
            (frame.origin.x - content_origin.x) / fit,
            (frame.origin.y - content_origin.y) / fit,
            frame.size.x / fit,
            frame.size.y / fit,
        );
        self.proto_overlays.viewport.set(Some(viewport));
        Some((
            viewport,
            ScenePage {
                id: String::new(),
                name: String::new(),
                children: self.overlay_scene_nodes(viewport, now_ms),
            },
        ))
    }

    /// While an overlay animates in, the next frame it needs.
    pub(crate) fn overlay_deadline_ms(&self) -> Option<u64> {
        let now = self.last_now_ms;
        self.proto_overlays
            .stack
            .iter()
            .any(|entry| progress(entry, now) < 1.0)
            .then_some(now + 16)
    }
}

/// `node` moved by `(dx, dy)`, through the scene's own translate.
fn translated(node: &SceneNode, dx: f32, dy: f32) -> SceneNode {
    let mut scene = LayoutScene {
        pages: vec![ScenePage {
            id: String::new(),
            name: String::new(),
            children: vec![node.clone()],
        }],
        active_page_index: 0,
    };
    scene.translate_nodes(std::slice::from_ref(&node.id), dx, dy);
    scene
        .pages
        .pop()
        .and_then(|mut page| page.children.pop())
        .unwrap_or_else(|| node.clone())
}
