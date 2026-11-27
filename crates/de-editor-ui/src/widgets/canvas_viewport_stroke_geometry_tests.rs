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
    Style(Box<StrokeStyle>),
    StrokePath(String, Point2D, f32, f32),
    FillPath(String, Point2D, f32, f32),
    /// The fill rule of the last fill: `true` = even-odd.
    FillRule(bool),
    Line(Point2D, Point2D),
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
    fn stroke_line(&mut self, from: Point2D, to: Point2D, _: Color, _: f32) {
        self.ops.push(Op::Line(from, to));
    }
    fn fill_round_rect(&mut self, _: Rect, _: f32, _: Color) {}
    fn stroke_round_rect(&mut self, _: Rect, _: f32, _: Color, _: f32) {}
    fn stroke_svg_path(&mut self, d: &str, at: Point2D, size: f32, _: Color, width: f32) {
        self.ops.push(Op::StrokePath(d.into(), at, size, width));
    }
    fn fill_svg_path(&mut self, d: &str, at: Point2D, size: f32, viewbox: f32, _: Color) {
        self.ops.push(Op::FillPath(d.into(), at, size, viewbox));
    }
    fn fill_svg_path_with_fill_rule(
        &mut self,
        d: &str,
        at: Point2D,
        size: f32,
        viewbox: f32,
        color: Color,
        even_odd: bool,
    ) {
        self.fill_svg_path(d, at, size, viewbox, color);
        self.ops.push(Op::FillRule(even_odd));
    }
    fn set_stroke_style(&mut self, style: StrokeStyle) {
        self.ops.push(Op::Style(Box::new(style)));
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
        head_hits: Vec::new(),
    };
    let ops = paint(&line(Some(geometry)), 2.0);
    assert!(!ops.iter().any(|op| matches!(op, Op::Line(..))), "{ops:?}");
    let at = Point2D::new(20.0, 40.0);
    assert!(ops.contains(&Op::StrokePath("M 0 0 L 90 0".into(), at, 48.0, 4.0)));
    assert!(ops.contains(&Op::FillPath(
        "M 100 0 L 90 5 L 90 -5 Z".into(),
        at,
        2.0,
        1.0
    )));
    assert!(ops.contains(&Op::StrokePath("M 1 1 L 2 2 L 1 3".into(), at, 48.0, 4.0)));
    // The heads fill nonzero, so overlapping shapes never cancel.
    assert!(ops.contains(&Op::FillRule(false)), "{ops:?}");
    // The centre line keeps its join and (zoomed) dash, and its dashes the
    // cap (unset = round on an open stroke); the arms are plain mitred
    // strokes; the node's own style comes back last.
    let Op::Style(centre) = &ops[0] else {
        panic!("{ops:?}");
    };
    assert_eq!(centre.cap, Some(StrokeCap::Round));
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
    assert_eq!(**last, line(None).stroke.unwrap().style.scaled(2.0));
}

#[test]
fn a_line_without_geometry_strokes_as_before() {
    assert_eq!(
        paint(&line(None), 1.0),
        vec![Op::Line(
            Point2D::new(10.0, 20.0),
            Point2D::new(110.0, 20.0)
        )]
    );
}

/// The centre line's cap: butt on a solid line (its ends are fill
/// shapes), the authored cap on a dashed one, whose dashes need it.
#[test]
fn solid_centre_lines_are_butt_and_dashed_ones_keep_their_cap() {
    use crate::stroke_centre_line::centre_line_style;
    let solid = StrokeStyle {
        cap: Some(StrokeCap::Round),
        ..StrokeStyle::default()
    };
    assert_eq!(centre_line_style(solid).cap, Some(StrokeCap::Butt));
    let dashed = StrokeStyle {
        cap: Some(StrokeCap::Square),
        dash: StrokeDash::new(&[4.0, 2.0], 0.0),
        ..StrokeStyle::default()
    };
    assert_eq!(centre_line_style(dashed).cap, Some(StrokeCap::Square));
}

/// A dotted line (zero-length dashes, round cap) with any end shape keeps
/// drawing its dots: the centre line is stroked with the round cap.
#[test]
fn a_dotted_line_with_end_shapes_stays_visible() {
    use de_editor_core::stroke_geometry::{stroke_end_geometry, CentreLine, StrokeEnds};
    use de_editor_core::vector_edit::Subpath;
    use jian_ops_schema::style::{StrokeCap as SchemaCap, StrokeEndCap};
    let ends = [
        StrokeEndCap::None,
        StrokeEndCap::Round,
        StrokeEndCap::Square,
        StrokeEndCap::LineArrow,
        StrokeEndCap::TriangleArrow,
        StrokeEndCap::ReversedTriangle,
        StrokeEndCap::Circle,
        StrokeEndCap::Diamond,
    ];
    let subs = [Subpath::polyline(&[(0.0, 0.0), (100.0, 0.0)])];
    for end in ends {
        let e = StrokeEnds::from_parts(None, Some(end), Some(&SchemaCap::Round)).unwrap_or(
            StrokeEnds {
                start: StrokeEndCap::Round,
                end,
                cap: StrokeEndCap::Round,
            },
        );
        let g = stroke_end_geometry(&subs, 2.0, e, CentreLine::Dashed).expect("geometry");
        let mut node = line(Some(SceneStrokeGeometry {
            stroke_d: g.stroke_d,
            fill_d: g.fill_d,
            arms_d: g.arms_d,
            head_hits: Vec::new(),
        }));
        let stroke = node.stroke.as_mut().unwrap();
        stroke.style = StrokeStyle {
            cap: Some(StrokeCap::Round),
            dash: StrokeDash::new(&[0.0, 4.0], 0.0),
            ..StrokeStyle::default()
        };
        let ops = paint(&node, 1.0);
        let centre = ops
            .iter()
            .position(|op| matches!(op, Op::StrokePath(..)))
            .expect("centre line");
        let Op::Style(style) = &ops[centre - 1] else {
            panic!("{end:?}: {ops:?}");
        };
        assert_eq!(style.cap, Some(StrokeCap::Round), "{end:?}");
        assert!(style.dash.is_some(), "{end:?}");
    }
}

/// Without end shapes a line still runs from its start to its end: one
/// drawn up and to the right is the box's bottom-left → top-right
/// diagonal, not the other one.
#[test]
fn a_plain_line_follows_its_direction() {
    let mut node = line(None);
    node.bounds = Rect::xywh(10.0, 20.0, 100.0, 50.0);
    node.line_direction = crate::layout_scene::SceneLineDirection {
        right_to_left: false,
        bottom_to_top: true,
    };
    assert_eq!(
        paint(&node, 1.0),
        vec![Op::Line(
            Point2D::new(10.0, 70.0),
            Point2D::new(110.0, 20.0)
        )]
    );
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
