//! Present's slide painter (#158), shared by both hosts.
//!
//! The slideshow frames one board at a time; a slide transition paints
//! two — the slide being left and the one arriving — each fitted to the
//! canvas like the slideshow's own camera, offset sideways and faded as
//! `de_editor_core::slide_transition::slide_layers` says.

use crate::session::PreviewSession;
use de_editor_core::slide_transition::{slide_layers, SlideLayerBoard, SlideTransitionRun};
use de_editor_core::Viewport;
use de_editor_ui::{ImageBlendMode, Point2D, Rect, RenderBackend};

/// `t` (`0..1`) through `easing`, the curve node motion uses.
pub fn eased_progress(easing: &jian_ops_schema::motion::Easing, t: f32) -> f32 {
    crate::animation::ease(crate::motion::easing_from_schema(easing), t.clamp(0.0, 1.0))
}

impl PreviewSession {
    /// Paint top-level board `board` fitted to `canvas_rect` (zero
    /// padding, centred — the slideshow camera), clipped to its own rect,
    /// shifted right by `offset_x` slide widths and at `opacity`.
    /// Returns whether the board exists.
    pub fn paint_slide_board(
        &self,
        backend: &mut dyn RenderBackend,
        canvas_rect: Rect,
        board: &str,
        offset_x: f32,
        opacity: f32,
        now_ms: u64,
    ) -> bool {
        let Some(rect) = self.root_scene_rect(board) else {
            return false;
        };
        let mut viewport = Viewport::IDENTITY;
        viewport.fit_to(rect, canvas_rect.size.x, canvas_rect.size.y, 0.0);
        let screen = Rect {
            origin: Point2D::new(
                canvas_rect.origin.x + viewport.pan_x + rect.origin.x * viewport.zoom,
                canvas_rect.origin.y + viewport.pan_y + rect.origin.y * viewport.zoom,
            ),
            size: Point2D::new(rect.size.x * viewport.zoom, rect.size.y * viewport.zoom),
        };
        let dx = offset_x * screen.size.x;
        backend.save();
        backend.clip_rect(canvas_rect);
        backend.translate(Point2D::new(dx, 0.0));
        backend.clip_rect(screen);
        let faded = opacity < 1.0;
        if faded {
            backend.push_composite_layer(screen, opacity.max(0.0), ImageBlendMode::Normal);
        }
        self.paint_scene(
            backend,
            canvas_rect,
            (viewport.pan_x, viewport.pan_y),
            viewport.zoom,
            now_ms,
        );
        if faded {
            backend.restore();
        }
        backend.restore();
        true
    }

    /// Paint one frame of `run` at linear progress `progress`: both slides
    /// where the transition puts them. `to_board` is the slide arriving.
    pub fn paint_slide_transition(
        &self,
        backend: &mut dyn RenderBackend,
        canvas_rect: Rect,
        run: &SlideTransitionRun,
        to_board: &str,
        progress: f32,
        now_ms: u64,
    ) {
        let easing = run
            .easing
            .map(|easing| easing.to_schema())
            .unwrap_or_default();
        let t = eased_progress(&easing, progress);
        for layer in slide_layers(run.kind, t, run.forward) {
            let board = match layer.board {
                SlideLayerBoard::From => run.from_board.as_str(),
                SlideLayerBoard::To => to_board,
            };
            let _ = self.paint_slide_board(
                backend,
                canvas_rect,
                board,
                layer.offset_x,
                layer.opacity,
                now_ms,
            );
        }
    }
}
