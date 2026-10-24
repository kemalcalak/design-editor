//! Animated canvas content over the retained pan-cache layer (#208).
//!
//! A time- or pointer-driven shader used to make every frame repaint the
//! whole canvas, and kept the pan cache off: a document with 17 scene
//! shaders on screen spent ~55 ms per frame re-painting ~2,100 nodes to
//! update a few small boxes. When the canvas paint located all of its
//! animated content (`canvas_animation_regions`: boxes known, no backdrop
//! reads), the layer is kept even at rest, and each served frame repaints
//! only those boxes into it — every node intersecting a box, in order, so
//! content above a shader stays on top — before the blit.

use super::canvas_pan_cache::PAN_CACHE_MARGIN as M;
use super::WidgetHostNative;
use crate::backend::NativeFrameBackend;
use de_editor_ui::widgets::canvas_animation_regions::{
    merge_overlapping, snap_out, AnimationRegions,
};
use de_editor_ui::widgets::{CanvasViewport, PaintCx, Widget};
use de_editor_ui::{Point2D, Rect, RenderBackend};

impl WidgetHostNative {
    /// Whether the canvas animation (if any) lets a cached layer stand in
    /// for a full paint: nothing animates, or the last full paint located
    /// every animated box.
    pub(in crate::widget_host) fn animation_allows_cache(&self) -> bool {
        let animated = de_editor_ui::widgets::canvas_shader_clock::animated_shader_painted()
            || de_editor_ui::widgets::canvas_shader_pointer::pointer_shader_painted();
        !animated || self.canvas_animation.located
    }

    /// Everything besides animation that a rest-time layer needs to stay
    /// valid between frames: no preview, transition, agent overlay, drag,
    /// starter ghost or text edit (their pixels change without a recorded
    /// box or an editor-state mutation).
    pub(in crate::widget_host) fn rest_layer_conditions(&self) -> bool {
        self.preview.is_none()
            && self
                .layout_transition
                .as_ref()
                .is_none_or(|transition| !transition.is_active(self.now_ms))
            && de_editor_core::agent_indicators::snapshot_at_if_active(self.now_ms).is_none()
            && self.node_drag.is_none()
            && self.marquee_drag.is_none()
            && self.editor_state.editor_ui.starter_ghost.is_none()
            && self.editor_state.ui.text_editing.is_none()
    }

    /// Whether this rest frame should paint the canvas into a fresh layer:
    /// the last paint animated, and all of it was located.
    pub(in crate::widget_host) fn rest_animation_layer_wanted(&self) -> bool {
        let animated = de_editor_ui::widgets::canvas_shader_clock::animated_shader_painted()
            || de_editor_ui::widgets::canvas_shader_pointer::pointer_shader_painted();
        !self.fast_interaction_active()
            && animated
            && self.canvas_animation.located
            && !self.canvas_animation.rects.is_empty()
            && self.rest_layer_conditions()
    }

    /// Repaint the layer under its animated boxes. The layer's content is
    /// anchored at `cache.pan`; the live pan is `cache.pan + offset`, so the
    /// scene paints shifted back by `offset` to land where the layer has it.
    /// Runs after any strip / tile repaint of the frame, so the shader clock
    /// the host schedules the next frame from ends up marked.
    pub(in crate::widget_host) fn refresh_animated_regions(
        &mut self,
        frame: &mut NativeFrameBackend<'_>,
        canvas_rect: Rect,
        offset: Point2D,
    ) {
        let Some(mut cache) = self.pan_cache.take() else {
            return;
        };
        if cache.animated.is_empty() {
            self.pan_cache = Some(cache);
            return;
        }
        let dpi = frame.dpi_scale();
        let expanded = Rect {
            origin: Point2D::new(canvas_rect.origin.x - M, canvas_rect.origin.y - M),
            size: Point2D::new(canvas_rect.size.x + M * 2.0, canvas_rect.size.y + M * 2.0),
        };
        let regions = merge_overlapping(
            cache
                .animated
                .iter()
                .filter_map(|rect| intersect(snap_out(*rect, dpi), expanded))
                .collect(),
        );
        {
            let mut canvas = CanvasViewport::from_editor(&self.editor_state, &self.layout_scene);
            canvas.now_ms = self.now_ms;
            canvas.pointer = self.shader_pointer;
            // Same quality the layer holds: degraded mid-gesture.
            canvas.fast_interaction = !cache.sharp && self.fast_interaction_active();
            canvas.offset_paint_origin(M - offset.x, M - offset.y);
            for region in regions {
                canvas.cull_override = Some(region);
                frame.paint_offscreen_region(
                    &mut cache.surface,
                    canvas_rect,
                    M,
                    Some(region),
                    |off| {
                        let mut cx = PaintCx { backend: off };
                        canvas.paint(&mut cx, expanded);
                    },
                );
            }
        }
        self.pan_cache_animation_refreshes = self.pan_cache_animation_refreshes.wrapping_add(1);
        self.pan_cache = Some(cache);
    }

    /// Fold what a partial repaint into `cache` (an exposed pan strip, a
    /// restore tile) recorded: a shader scrolled into view joins the
    /// refreshed set, and a paint that could not locate its animation or
    /// read a backdrop (full-quality tiles paint blur layers a degraded
    /// gesture build skipped) stops the layer from serving the next frame.
    pub(in crate::widget_host) fn absorb_partial_paint(
        &mut self,
        cache: &mut super::canvas_pan_cache::CanvasPanCache,
        recorded: AnimationRegions,
    ) {
        if !recorded.located {
            self.canvas_animation.located = false;
        }
        for rect in recorded.rects {
            if !cache.animated.contains(&rect) {
                cache.animated.push(rect);
            }
        }
    }
}

/// Move the layer's animated boxes with its content (a scroll in place or
/// a rebase shifts every pixel by `shift`).
pub(in crate::widget_host) fn shift_animated(animated: &mut [Rect], shift: Point2D) {
    for rect in animated {
        rect.origin.x += shift.x;
        rect.origin.y += shift.y;
    }
}

fn intersect(a: Rect, b: Rect) -> Option<Rect> {
    let x0 = a.origin.x.max(b.origin.x);
    let y0 = a.origin.y.max(b.origin.y);
    let x1 = (a.origin.x + a.size.x).min(b.origin.x + b.size.x);
    let y1 = (a.origin.y + a.size.y).min(b.origin.y + b.size.y);
    (x1 > x0 && y1 > y0).then(|| Rect {
        origin: Point2D::new(x0, y0),
        size: Point2D::new(x1 - x0, y1 - y0),
    })
}
