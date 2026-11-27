//! Canvas paint of precomputed corner outlines (#389).

use crate::layout_scene::{NodeKind, SceneCornerPath, SceneNode, SceneStroke, SceneStrokeAlign};
use crate::widgets::canvas_viewport_paint::paint_node_with_options;
use crate::widgets::PaintCx;
use crate::{Color, Point2D, Rect, RenderBackend, TextLayout};
use std::sync::Arc;

#[derive(Default)]
struct Recorder {
    ops: Vec<String>,
}

fn r(rect: Rect) -> String {
    format!(
        "{},{},{},{}",
        rect.origin.x, rect.origin.y, rect.size.x, rect.size.y
    )
}

impl RenderBackend for Recorder {
    fn begin_frame(&mut self) {}
    fn end_frame(&mut self) {}
    fn fill_rect(&mut self, rect: Rect, _: Color) {
        self.ops.push(format!("fill_rect({})", r(rect)));
    }
    fn stroke_rect(&mut self, _: Rect, _: Color, _: f32) {
        self.ops.push("stroke_rect".into());
    }
    fn draw_text(&mut self, _: &TextLayout, _: Point2D) {}
    fn clip_rect(&mut self, rect: Rect) {
        self.ops.push(format!("clip_rect({})", r(rect)));
    }
    fn clip_round_rect(&mut self, _: Rect, _: f32) {
        self.ops.push("clip_round_rect".into());
    }
    fn clip_svg_path_in_rect(&mut self, d: &str, rect: Rect, _: bool) {
        self.ops.push(format!("clip_path({d}|{})", r(rect)));
    }
    fn save(&mut self) {
        self.ops.push("save".into());
    }
    fn restore(&mut self) {
        self.ops.push("restore".into());
    }
    fn translate(&mut self, _: Point2D) {}
    fn stroke_line(&mut self, _: Point2D, _: Point2D, _: Color, _: f32) {}
    fn fill_round_rect(&mut self, _: Rect, _: f32, _: Color) {
        self.ops.push("fill_round_rect".into());
    }
    fn stroke_round_rect(&mut self, _: Rect, _: f32, _: Color, _: f32) {
        self.ops.push("stroke_round_rect".into());
    }
    fn fill_polygon(&mut self, _: &[Point2D], _: Color) {
        self.ops.push("fill_polygon".into());
    }
    fn stroke_polygon(&mut self, _: &[Point2D], _: Color, _: f32) {
        self.ops.push("stroke_polygon".into());
    }
    fn stroke_svg_path(&mut self, _: &str, _: Point2D, _: f32, _: Color, _: f32) {}
    fn stroke_svg_path_in_rect(&mut self, d: &str, rect: Rect, _: Color, width: f32) {
        self.ops
            .push(format!("stroke_path({d}|{}|w={width})", r(rect)));
    }
    fn resize(&mut self, _: u32, _: u32) {}
    fn dpi_scale(&self) -> f32 {
        1.0
    }
}

fn paint(node: &SceneNode, zoom: f32) -> Vec<String> {
    let mut backend = Recorder::default();
    {
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
    }
    backend.ops
}

fn outline(d: &str, bounds: Rect) -> Arc<SceneCornerPath> {
    Arc::new(SceneCornerPath {
        d: d.into(),
        bounds,
        ..SceneCornerPath::default()
    })
}

fn stroke(align: SceneStrokeAlign) -> Option<SceneStroke> {
    Some(SceneStroke {
        color: Color::BLACK,
        width: 2.0,
        sides: None,
        align,
        style: Default::default(),
    })
}

#[test]
fn a_smoothed_rect_fills_square_inside_its_outline_and_strokes_it() {
    let mut node = SceneNode::leaf("r", NodeKind::Rect);
    node.bounds = Rect::xywh(10.0, 20.0, 100.0, 60.0);
    node.corner_radius = 12.0;
    node.fill = Some(Color::RED);
    node.stroke = stroke(SceneStrokeAlign::Center);
    node.corner_path = Some(outline("M 0 0 Z", Rect::xywh(0.0, 0.0, 100.0, 60.0)));
    let ops = paint(&node, 2.0);
    let clip = ops
        .iter()
        .position(|op| op == "clip_path(M 0 0 Z|20,40,200,120)")
        .expect("outline clip");
    assert_eq!(ops[clip - 1], "save");
    assert_eq!(ops[clip + 1], "fill_rect(20,40,200,120)", "{ops:?}");
    assert_eq!(ops[clip + 2], "restore");
    assert!(ops.contains(&"stroke_path(M 0 0 Z|20,40,200,120|w=4)".to_string()));
    assert!(!ops.iter().any(|op| op.contains("round_rect")), "{ops:?}");
}

#[test]
fn an_aligned_stroke_follows_its_own_centre_line() {
    let mut node = SceneNode::leaf("r", NodeKind::Frame);
    node.bounds = Rect::xywh(0.0, 0.0, 100.0, 60.0);
    node.corner_radius = 12.0;
    node.stroke = stroke(SceneStrokeAlign::Inside);
    node.corner_path = Some(Arc::new(SceneCornerPath {
        d: "M 0 0 Z".into(),
        bounds: Rect::xywh(0.0, 0.0, 100.0, 60.0),
        stroke_d: "M 1 1 Z".into(),
        stroke_bounds: Rect::xywh(1.0, 1.0, 98.0, 58.0),
        hit_ring: Vec::new(),
    }));
    let ops = paint(&node, 1.0);
    assert!(
        ops.contains(&"stroke_path(M 1 1 Z|1,1,98,58|w=2)".to_string()),
        "{ops:?}"
    );
}

#[test]
fn clip_content_uses_the_outline() {
    let mut child = SceneNode::leaf("c", NodeKind::Rect);
    child.bounds = Rect::xywh(0.0, 0.0, 500.0, 20.0);
    child.fill = Some(Color::BLUE);
    let mut frame = SceneNode::leaf("f", NodeKind::Frame);
    frame.bounds = Rect::xywh(0.0, 0.0, 100.0, 100.0);
    frame.corner_radius = 20.0;
    frame.clip_content = true;
    frame.children = vec![child];
    frame.corner_path = Some(outline("M 5 5 Z", Rect::xywh(0.0, 0.0, 100.0, 100.0)));
    let ops = paint(&frame, 1.0);
    let clip = ops
        .iter()
        .position(|op| op == "clip_path(M 5 5 Z|0,0,100,100)")
        .expect("child clip");
    assert_eq!(ops[clip + 1], "fill_rect(0,0,500,20)", "{ops:?}");
    assert!(!ops.contains(&"clip_round_rect".to_string()));
}

#[test]
fn a_rounded_polygon_paints_its_outline_not_its_vertices() {
    let mut node = SceneNode::leaf("p", NodeKind::Polygon);
    node.bounds = Rect::xywh(0.0, 0.0, 80.0, 80.0);
    node.polygon_sides = 5;
    node.corner_radius = 6.0;
    node.fill = Some(Color::RED);
    node.stroke = stroke(SceneStrokeAlign::Center);
    node.corner_path = Some(outline("M 9 9 Z", Rect::xywh(2.0, 3.0, 76.0, 70.0)));
    let ops = paint(&node, 1.0);
    assert!(!ops.contains(&"fill_polygon".to_string()), "{ops:?}");
    assert!(!ops.contains(&"stroke_polygon".to_string()), "{ops:?}");
    // The fill spans the node box (gradients and images lay out over it)
    // inside the outline, which sits where its bounds say.
    assert!(ops.contains(&"clip_path(M 9 9 Z|2,3,76,70)".to_string()));
    assert!(ops.contains(&"fill_rect(0,0,80,80)".to_string()));
    assert!(ops.contains(&"stroke_path(M 9 9 Z|2,3,76,70|w=2)".to_string()));
}

#[test]
fn an_open_rounded_path_strokes_without_filling() {
    let mut node = SceneNode::leaf("p", NodeKind::Path);
    node.bounds = Rect::xywh(0.0, 0.0, 100.0, 50.0);
    node.fill = Some(Color::RED);
    node.stroke = stroke(SceneStrokeAlign::Center);
    node.corner_path = Some(outline("M 0 0 L 90 0", Rect::xywh(0.0, 0.0, 100.0, 50.0)));
    let ops = paint(&node, 1.0);
    assert!(!ops.iter().any(|op| op.starts_with("fill_rect")), "{ops:?}");
    assert!(ops.contains(&"stroke_path(M 0 0 L 90 0|0,0,100,50|w=2)".to_string()));
}

#[test]
fn nodes_without_an_outline_paint_as_before() {
    let mut node = SceneNode::leaf("r", NodeKind::Rect);
    node.bounds = Rect::xywh(0.0, 0.0, 100.0, 60.0);
    node.corner_radius = 12.0;
    node.fill = Some(Color::RED);
    node.stroke = stroke(SceneStrokeAlign::Center);
    let ops = paint(&node, 1.0);
    assert_eq!(ops, vec!["fill_round_rect", "stroke_round_rect"]);
}
