//! Sibling test file for `canvas_viewport_paint.rs` — each node's stroke
//! cap / join / dash (#89) is the active style while that node strokes,
//! scaled by zoom, and never leaks into a child.

use crate::layout_scene::{NodeKind, SceneNode, SceneStroke, SceneStrokeAlign};
use crate::widgets::canvas_viewport_paint::paint_node_with_options;
use crate::widgets::PaintCx;
use crate::{Color, Point2D, Rect, RenderBackend, TextLayout};
use de_editor_core::stroke_render_style::{self, LineCap, StrokeRenderStyle};

/// Records the dash active at each stroke call, by stroke width.
#[derive(Default)]
struct StyleCaptureBackend {
    strokes: Vec<(f32, Option<Vec<f32>>)>,
}

impl StyleCaptureBackend {
    fn record(&mut self, width: f32) {
        let dash = stroke_render_style::active().and_then(|style| style.dash.clone());
        self.strokes.push((width, dash));
    }
}

impl RenderBackend for StyleCaptureBackend {
    fn begin_frame(&mut self) {}
    fn end_frame(&mut self) {}
    fn fill_rect(&mut self, _: Rect, _: Color) {}
    fn stroke_rect(&mut self, _: Rect, _: Color, w: f32) {
        self.record(w);
    }
    fn draw_text(&mut self, _: &TextLayout, _: Point2D) {}
    fn clip_rect(&mut self, _: Rect) {}
    fn save(&mut self) {}
    fn restore(&mut self) {}
    fn translate(&mut self, _: Point2D) {}
    fn stroke_line(&mut self, _: Point2D, _: Point2D, _: Color, w: f32) {
        self.record(w);
    }
    fn fill_round_rect(&mut self, _: Rect, _: f32, _: Color) {}
    fn stroke_round_rect(&mut self, _: Rect, _: f32, _: Color, w: f32) {
        self.record(w);
    }
    fn stroke_svg_path(&mut self, _: &str, _: Point2D, _: f32, _: Color, _: f32) {}
    fn resize(&mut self, _: u32, _: u32) {}
    fn dpi_scale(&self) -> f32 {
        1.0
    }
}

fn stroked(id: &str, kind: NodeKind, width: f32) -> SceneNode {
    let mut n = SceneNode::leaf(id, kind);
    n.bounds = Rect::xywh(0.0, 0.0, 100.0, 50.0);
    n.stroke = Some(SceneStroke {
        color: Color::BLACK,
        width,
        sides: None,
        align: SceneStrokeAlign::Center,
    });
    n
}

fn paint(node: &SceneNode, zoom: f32) -> Vec<(f32, Option<Vec<f32>>)> {
    let mut backend = StyleCaptureBackend::default();
    let mut cx = PaintCx {
        backend: &mut backend,
    };
    let _ = paint_node_with_options(
        &mut cx,
        node,
        Point2D::ZERO,
        zoom,
        None,
        Rect::xywh(-4000.0, -4000.0, 8000.0, 8000.0),
        None,
        None,
        None,
        None,
    );
    backend.strokes
}

fn dashed(pattern: &[f32]) -> Option<StrokeRenderStyle> {
    Some(StrokeRenderStyle {
        cap: Some(LineCap::Butt),
        join: None,
        dash: Some(pattern.to_vec()),
        dash_offset: 0.0,
    })
}

#[test]
fn a_dashed_node_strokes_with_its_dash_scaled_by_zoom() {
    let id = "stroke-style-paint-line";
    stroke_render_style::register_node(id, dashed(&[4.0, 2.0]));
    let strokes = paint(&stroked(id, NodeKind::Line, 1.0), 2.0);
    stroke_render_style::register_node(id, None);
    assert_eq!(strokes, vec![(2.0, Some(vec![8.0, 4.0]))]);
    assert!(
        stroke_render_style::active().is_none(),
        "scope ends with the node"
    );
}

#[test]
fn a_child_does_not_inherit_and_the_container_stroke_keeps_its_own() {
    let parent_id = "stroke-style-paint-frame";
    let child_id = "stroke-style-paint-child";
    stroke_render_style::register_node(parent_id, dashed(&[6.0, 6.0]));
    let mut frame = stroked(parent_id, NodeKind::Frame, 3.0);
    frame.children.push(stroked(child_id, NodeKind::Line, 1.0));
    let strokes = paint(&frame, 1.0);
    stroke_render_style::register_node(parent_id, None);
    // Child first (a container strokes after its children), undashed;
    // then the frame's own deferred stroke with its dash.
    assert_eq!(
        strokes,
        vec![(1.0, None), (3.0, Some(vec![6.0, 6.0]))],
        "{strokes:?}"
    );
}
