use super::{paint_node_stroke, stroke_paint_rect, with_stroke_paint_or};
use crate::layout_scene::{
    NodeKind, SceneFillLayer, SceneGradient, SceneGradientStop, SceneNode, SceneStroke,
    SceneStrokeAlign,
};
use crate::widgets::PaintCx;
use crate::{Color, ImageBlendMode, Point2D, Rect, RenderBackend, TextLayout};

#[derive(Debug, Clone, PartialEq)]
enum Op {
    Composite(Rect, f32, ImageBlendMode),
    MaskSource,
    Fill(Rect, Color),
    Gradient(Rect),
    Stroke(Rect, Color, f32),
    Restore,
}

#[derive(Default)]
struct Recorder {
    masks: bool,
    ops: Vec<Op>,
}

impl RenderBackend for Recorder {
    fn begin_frame(&mut self) {}
    fn end_frame(&mut self) {}
    fn fill_rect(&mut self, rect: Rect, color: Color) {
        self.ops.push(Op::Fill(rect, color));
    }
    fn stroke_rect(&mut self, rect: Rect, color: Color, width: f32) {
        self.ops.push(Op::Stroke(rect, color, width));
    }
    fn draw_text(&mut self, _: &TextLayout, _: Point2D) {}
    fn clip_rect(&mut self, _: Rect) {}
    fn save(&mut self) {}
    fn restore(&mut self) {
        self.ops.push(Op::Restore);
    }
    fn translate(&mut self, _: Point2D) {}
    fn stroke_line(&mut self, _: Point2D, _: Point2D, _: Color, _: f32) {}
    fn fill_round_rect(&mut self, rect: Rect, _: f32, color: Color) {
        self.fill_rect(rect, color);
    }
    fn stroke_round_rect(&mut self, rect: Rect, _: f32, color: Color, width: f32) {
        self.stroke_rect(rect, color, width);
    }
    fn stroke_svg_path(&mut self, _: &str, _: Point2D, _: f32, _: Color, _: f32) {}
    fn fill_round_rect_linear_gradient(
        &mut self,
        rect: Rect,
        _: f32,
        _: &[(f32, Color)],
        _: f32,
        _: f32,
    ) {
        self.ops.push(Op::Gradient(rect));
    }
    fn push_composite_layer(&mut self, bounds: Rect, opacity: f32, mode: ImageBlendMode) {
        self.ops.push(Op::Composite(bounds, opacity, mode));
    }
    fn supports_pixel_masks(&self) -> bool {
        self.masks
    }
    fn push_mask_source_layer(&mut self, _: bool) {
        self.ops.push(Op::MaskSource);
    }
    fn resize(&mut self, _: u32, _: u32) {}
    fn dpi_scale(&self) -> f32 {
        1.0
    }
}

const GREY: Color = Color {
    r: 0.5,
    g: 0.5,
    b: 0.5,
    a: 1.0,
};

fn stroked_rect(align: SceneStrokeAlign) -> SceneNode {
    let mut node = SceneNode::leaf("n", NodeKind::Rect);
    node.bounds = Rect::xywh(10.0, 10.0, 100.0, 50.0);
    node.stroke = Some(SceneStroke {
        color: GREY,
        width: 4.0,
        sides: None,
        align,
        style: Default::default(),
    });
    node
}

fn gradient_layer() -> SceneFillLayer {
    SceneFillLayer::Gradient {
        gradient: SceneGradient::Linear {
            angle_deg: 0.0,
            opacity: 1.0,
            stops: vec![
                SceneGradientStop {
                    offset: 0.0,
                    color: Color::RED,
                },
                SceneGradientStop {
                    offset: 1.0,
                    color: Color::BLUE,
                },
            ],
        },
        blend_mode: ImageBlendMode::Normal,
    }
}

fn paint(node: &SceneNode, masks: bool) -> Vec<Op> {
    let mut backend = Recorder {
        masks,
        ..Default::default()
    };
    {
        let mut cx = PaintCx {
            backend: &mut backend,
        };
        paint_node_stroke(&mut cx, node, node.bounds, 1.0);
    }
    backend.ops
}

#[test]
fn a_solid_stroke_keeps_the_direct_path() {
    let node = stroked_rect(SceneStrokeAlign::Center);
    assert_eq!(paint(&node, true), vec![Op::Stroke(node.bounds, GREY, 4.0)]);
}

#[test]
fn a_rich_stroke_masks_its_paint_stack_with_the_stroke_geometry() {
    let mut node = stroked_rect(SceneStrokeAlign::Center);
    node.opacity = 0.5;
    node.stroke_fill_layers = vec![gradient_layer()];
    let paint_rect = Rect::xywh(8.0, 8.0, 104.0, 54.0);
    let layer_rect = Rect::xywh(-2.0, -2.0, 124.0, 74.0);
    assert_eq!(
        paint(&node, true),
        vec![
            Op::Composite(layer_rect, 0.5, ImageBlendMode::Normal),
            Op::Gradient(paint_rect),
            Op::MaskSource,
            Op::Stroke(node.bounds, Color::WHITE, 4.0),
            Op::Restore,
            Op::Restore,
        ]
    );
}

#[test]
fn stacked_stroke_paints_run_back_to_front_with_their_blend() {
    let mut node = stroked_rect(SceneStrokeAlign::Inside);
    node.stroke_fill_layers = vec![
        SceneFillLayer::Solid {
            color: Color::RED,
            blend_mode: ImageBlendMode::LinearDodge,
        },
        gradient_layer(),
    ];
    let ops = paint(&node, true);
    let paint_rect = node.bounds;
    assert_eq!(ops[1], Op::Gradient(paint_rect));
    assert!(matches!(
        ops[2],
        Op::Composite(_, 1.0, ImageBlendMode::LinearDodge)
    ));
    assert_eq!(ops[3], Op::Fill(paint_rect, Color::RED));
    assert_eq!(ops[4], Op::Restore);
    assert_eq!(ops[5], Op::MaskSource);
}

#[test]
fn backends_without_masks_keep_the_fallback_colour() {
    let mut node = stroked_rect(SceneStrokeAlign::Center);
    node.stroke_fill_layers = vec![gradient_layer()];
    assert_eq!(
        paint(&node, false),
        vec![Op::Stroke(node.bounds, GREY, 4.0)]
    );
}

#[test]
fn an_implicit_outline_draws_without_an_authored_stroke() {
    let node = SceneNode::leaf("bare", NodeKind::Line);
    let mut backend = Recorder::default();
    let mut drawn = None;
    {
        let mut cx = PaintCx {
            backend: &mut backend,
        };
        with_stroke_paint_or(
            &mut cx,
            &node,
            node.bounds,
            1.0,
            Color::BLACK,
            |_, color| {
                drawn = Some(color);
            },
        );
    }
    assert_eq!(drawn, Some(Color::BLACK));
}

#[test]
fn the_paint_box_covers_the_visible_band_for_every_alignment() {
    let rect = Rect::xywh(0.0, 0.0, 10.0, 10.0);
    let mut stroke = stroked_rect(SceneStrokeAlign::Inside).stroke.unwrap();
    assert_eq!(stroke_paint_rect(rect, &stroke, 2.0), rect);
    stroke.align = SceneStrokeAlign::Center;
    assert_eq!(
        stroke_paint_rect(rect, &stroke, 2.0),
        Rect::xywh(-4.0, -4.0, 18.0, 18.0)
    );
    stroke.align = SceneStrokeAlign::Outside;
    assert_eq!(
        stroke_paint_rect(rect, &stroke, 2.0),
        Rect::xywh(-8.0, -8.0, 26.0, 26.0)
    );
}
