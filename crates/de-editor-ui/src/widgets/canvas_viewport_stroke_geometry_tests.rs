//! Canvas paint of precomputed stroke geometry (#389).

use super::{paint_line_node_stroke, paint_stroke_geometry};
use crate::layout_scene::{
    NodeKind, SceneNode, SceneStroke, SceneStrokeAlign, SceneStrokeGeometry, StrokeCap, StrokeDash,
    StrokeJoin, StrokeStyle,
};
use crate::widgets::PaintCx;
use crate::{Color, Point2D, Rect, RenderBackend, TextLayout};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq)]
enum Op {
    Style(StrokeStyle),
    StrokePath(String, Point2D, f32, f32),
    FillPath(String, Point2D, f32, f32),
    Line,
}

#[derive(Default)]
struct Recorder {
    ops: Vec<Op>,
}

impl RenderBackend for Recorder {
    fn begin_frame(&mut self) {}
    fn end_frame(&mut self) {}
    fn fill_rect(&mut self, _: Rect, _: Color) {}
    fn stroke_rect(&mut self, _: Rect, _: Color, _: f32) {}
    fn draw_text(&mut self, _: &TextLayout, _: Point2D) {}
    fn clip_rect(&mut self, _: Rect) {}
    fn save(&mut self) {}
    fn restore(&mut self) {}
    fn translate(&mut self, _: Point2D) {}
    fn stroke_line(&mut self, _: Point2D, _: Point2D, _: Color, _: f32) {
        self.ops.push(Op::Line);
    }
    fn fill_round_rect(&mut self, _: Rect, _: f32, _: Color) {}
    fn stroke_round_rect(&mut self, _: Rect, _: f32, _: Color, _: f32) {}
    fn stroke_svg_path(&mut self, d: &str, at: Point2D, size: f32, _: Color, width: f32) {
        self.ops.push(Op::StrokePath(d.into(), at, size, width));
    }
    fn fill_svg_path(&mut self, d: &str, at: Point2D, size: f32, viewbox: f32, _: Color) {
        self.ops.push(Op::FillPath(d.into(), at, size, viewbox));
    }
    fn set_stroke_style(&mut self, style: StrokeStyle) {
        self.ops.push(Op::Style(style));
    }
    fn resize(&mut self, _: u32, _: u32) {}
    fn dpi_scale(&self) -> f32 {
        1.0
    }
}

fn line(geometry: Option<SceneStrokeGeometry>) -> SceneNode {
    let mut node = SceneNode::leaf("l", NodeKind::Line);
    node.bounds = Rect::xywh(10.0, 20.0, 100.0, 0.0);
    node.stroke = Some(SceneStroke {
        color: Color::BLACK,
        width: 2.0,
        sides: None,
        align: SceneStrokeAlign::Center,
        style: StrokeStyle {
            join: Some(StrokeJoin::Round),
            dash: StrokeDash::new(&[4.0, 2.0], 0.0),
            ..StrokeStyle::default()
        },
    });
    node.stroke_geometry = geometry.map(Arc::new);
    node
}

fn paint(node: &SceneNode, zoom: f32) -> Vec<Op> {
    let mut backend = Recorder::default();
    {
        let mut cx = PaintCx {
            backend: &mut backend,
        };
        let world = Rect::xywh(
            node.bounds.origin.x * zoom,
            node.bounds.origin.y * zoom,
            node.bounds.size.x * zoom,
            node.bounds.size.y * zoom,
        );
        paint_line_node_stroke(&mut cx, node, world, zoom);
    }
    backend.ops
}

#[test]
fn geometry_replaces_the_plain_line_and_scales_with_zoom() {
    let geometry = SceneStrokeGeometry {
        stroke_d: "M 0 0 L 90 0".into(),
        fill_d: "M 100 0 L 90 5 L 90 -5 Z".into(),
        arms_d: "M 1 1 L 2 2 L 1 3".into(),
    };
    let ops = paint(&line(Some(geometry)), 2.0);
    assert!(!ops.contains(&Op::Line), "{ops:?}");
    let at = Point2D::new(20.0, 40.0);
    assert!(ops.contains(&Op::StrokePath("M 0 0 L 90 0".into(), at, 48.0, 4.0)));
    assert!(ops.contains(&Op::FillPath(
        "M 100 0 L 90 5 L 90 -5 Z".into(),
        at,
        2.0,
        1.0
    )));
    assert!(ops.contains(&Op::StrokePath("M 1 1 L 2 2 L 1 3".into(), at, 48.0, 4.0)));
    // The centre line keeps its join and (zoomed) dash with butt caps; the
    // arms are plain mitred strokes; the node's own style comes back last.
    let Op::Style(centre) = &ops[0] else {
        panic!("{ops:?}");
    };
    assert_eq!(centre.cap, Some(StrokeCap::Butt));
    assert_eq!(centre.join, Some(StrokeJoin::Round));
    assert_eq!(
        centre.dash.map(|d| d.lengths().to_vec()),
        Some(vec![8.0, 4.0])
    );
    let arms = ops
        .iter()
        .filter_map(|op| match op {
            Op::Style(s) if s.join == Some(StrokeJoin::Miter) => Some(s),
            _ => None,
        })
        .next()
        .expect("arm style");
    assert!(arms.dash.is_none());
    let Some(Op::Style(last)) = ops.last() else {
        panic!("{ops:?}");
    };
    assert_eq!(*last, line(None).stroke.unwrap().style.scaled(2.0));
}

#[test]
fn a_line_without_geometry_strokes_as_before() {
    assert_eq!(paint(&line(None), 1.0), vec![Op::Line]);
}

#[test]
fn empty_parts_are_skipped() {
    let node = line(Some(SceneStrokeGeometry {
        stroke_d: "M 0 0 L 100 0".into(),
        ..SceneStrokeGeometry::default()
    }));
    let mut backend = Recorder::default();
    {
        let mut cx = PaintCx {
            backend: &mut backend,
        };
        let geometry = node.stroke_geometry.clone().unwrap();
        paint_stroke_geometry(&mut cx, &node, node.bounds, 1.0, &geometry);
    }
    let paths = backend
        .ops
        .iter()
        .filter(|op| matches!(op, Op::StrokePath(..) | Op::FillPath(..)))
        .count();
    assert_eq!(paths, 1);
}
