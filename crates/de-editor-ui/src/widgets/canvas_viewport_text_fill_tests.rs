//! Paint-order tests for text with a fill stack (#179): the stack is
//! assembled in an isolated layer, cut to the glyphs by a mask source, and
//! own-fill runs paint solid on top.

use super::*;
use crate::layout_scene::{
    NodeKind, SceneFillLayer, SceneGradient, SceneGradientStop, SceneTextRun,
};
use crate::{Color, RenderBackend, TextLayout};

#[derive(Debug, PartialEq)]
enum Op {
    Composite(ImageBlendMode),
    MaskSource,
    Restore,
    /// A text draw: its content and whether it is the opaque mask ink.
    Text(String, bool),
    Gradient,
    FillRect,
}

struct Capture {
    ops: Vec<Op>,
    pixel_masks: bool,
}

impl Capture {
    fn new(pixel_masks: bool) -> Self {
        Self {
            ops: Vec::new(),
            pixel_masks,
        }
    }
}

impl RenderBackend for Capture {
    fn begin_frame(&mut self) {}
    fn end_frame(&mut self) {}
    fn fill_rect(&mut self, _: Rect, _: Color) {
        self.ops.push(Op::FillRect);
    }
    fn stroke_rect(&mut self, _: Rect, _: Color, _: f32) {}
    fn draw_text(&mut self, layout: &TextLayout, _: Point2D) {
        if let Some(run) = layout.runs().first() {
            let mask = run.color == Color::WHITE.to_jian();
            self.ops.push(Op::Text(run.content.clone(), mask));
        }
    }
    fn clip_rect(&mut self, _: Rect) {}
    fn save(&mut self) {}
    fn restore(&mut self) {
        self.ops.push(Op::Restore);
    }
    fn translate(&mut self, _: Point2D) {}
    fn stroke_line(&mut self, _: Point2D, _: Point2D, _: Color, _: f32) {}
    fn fill_round_rect(&mut self, _: Rect, _: f32, _: Color) {}
    fn stroke_round_rect(&mut self, _: Rect, _: f32, _: Color, _: f32) {}
    fn stroke_svg_path(&mut self, _: &str, _: Point2D, _: f32, _: Color, _: f32) {}
    fn resize(&mut self, _: u32, _: u32) {}
    fn dpi_scale(&self) -> f32 {
        1.0
    }
    fn measure_text_weighted(&mut self, text: &str, _: f32, _: u16) -> f32 {
        text.chars().count() as f32 * 10.0
    }
    fn supports_pixel_masks(&self) -> bool {
        self.pixel_masks
    }
    fn push_composite_layer(&mut self, _: Rect, _: f32, mode: ImageBlendMode) {
        self.ops.push(Op::Composite(mode));
    }
    fn push_mask_source_layer(&mut self, luminance: bool) {
        assert!(!luminance, "glyph coverage is an alpha mask");
        self.ops.push(Op::MaskSource);
    }
    fn fill_round_rect_linear_gradient(
        &mut self,
        _: Rect,
        _: f32,
        _: &[(f32, Color)],
        _: f32,
        _: f32,
    ) {
        self.ops.push(Op::Gradient);
    }
}

fn gradient_text(content: &str) -> SceneNode {
    let mut node = SceneNode::leaf("t", NodeKind::Text);
    node.bounds = Rect::xywh(0.0, 0.0, 200.0, 40.0);
    node.text = Some(content.to_string());
    node.font_size = 20.0;
    node.fill = Some(Color::BLACK);
    node.fill_layers = vec![SceneFillLayer::Gradient {
        gradient: SceneGradient::Linear {
            angle_deg: 90.0,
            opacity: 1.0,
            stops: vec![
                SceneGradientStop {
                    offset: 0.0,
                    color: Color::BLACK,
                },
                SceneGradientStop {
                    offset: 1.0,
                    color: Color::WHITE,
                },
            ],
        },
        blend_mode: ImageBlendMode::Normal,
    }];
    node
}

fn paint(backend: &mut Capture, node: &SceneNode) {
    let mut cx = PaintCx { backend };
    paint_text_in_viewport(&mut cx, node, node.bounds, Point2D::ZERO, 1.0, &None);
}

#[test]
fn a_gradient_fill_paints_under_a_glyph_mask() {
    let node = gradient_text("Hi");
    let mut backend = Capture::new(true);
    paint(&mut backend, &node);
    let ops = backend.ops;
    let composite = ops
        .iter()
        .position(|o| *o == Op::Composite(ImageBlendMode::Normal))
        .expect("stack is isolated");
    let gradient = ops
        .iter()
        .position(|o| *o == Op::Gradient)
        .expect("gradient");
    let mask = ops.iter().position(|o| *o == Op::MaskSource).expect("mask");
    let glyphs = ops
        .iter()
        .position(|o| *o == Op::Text("Hi".into(), true))
        .expect("glyphs paint as mask ink");
    assert!(
        composite < gradient && gradient < mask && mask < glyphs,
        "{ops:?}"
    );
    // Mask layer + stack layer both close after the glyphs.
    let closes = ops[glyphs..].iter().filter(|o| **o == Op::Restore).count();
    assert!(closes >= 2, "{ops:?}");
    assert!(
        !ops.contains(&Op::Text("Hi".into(), false)),
        "no solid ink copy: {ops:?}"
    );
}

#[test]
fn own_fill_runs_paint_solid_over_the_masked_stack() {
    let mut node = gradient_text("ab");
    node.text_runs = vec![
        SceneTextRun {
            start: 0,
            end: 1,
            font_size: 0.0,
            font_weight: 0,
            fill: None,
            italic: false,
            underline: false,
            strikethrough: false,
        },
        SceneTextRun {
            start: 1,
            end: 2,
            font_size: 0.0,
            font_weight: 0,
            fill: Some(Color {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            }),
            italic: false,
            underline: false,
            strikethrough: false,
        },
    ];
    let mut backend = Capture::new(true);
    paint(&mut backend, &node);
    let ops = backend.ops;
    let inherited = ops
        .iter()
        .position(|o| *o == Op::Text("a".into(), true))
        .expect("inherited run is mask ink");
    let own = ops
        .iter()
        .position(|o| *o == Op::Text("b".into(), false))
        .expect("own-fill run paints in its colour");
    assert!(inherited < own, "{ops:?}");
    assert!(!ops.contains(&Op::Text("b".into(), true)), "{ops:?}");
    assert!(!ops.contains(&Op::Text("a".into(), false)), "{ops:?}");
}

#[test]
fn plain_solid_text_and_maskless_backends_keep_the_ink_path() {
    let mut node = gradient_text("Hi");
    node.fill_layers = vec![SceneFillLayer::Solid {
        color: Color::BLACK,
        blend_mode: ImageBlendMode::Normal,
    }];
    let mut backend = Capture::new(true);
    paint(&mut backend, &node);
    assert_eq!(
        backend
            .ops
            .iter()
            .filter(|o| matches!(o, Op::Text(..)))
            .count(),
        1
    );
    assert!(!backend.ops.contains(&Op::MaskSource), "{:?}", backend.ops);

    let node = gradient_text("Hi");
    let mut backend = Capture::new(false);
    paint(&mut backend, &node);
    assert!(!backend.ops.contains(&Op::MaskSource), "{:?}", backend.ops);
    assert!(!backend.ops.contains(&Op::Gradient), "{:?}", backend.ops);
    assert!(backend.ops.contains(&Op::Text("Hi".into(), false)));
}

#[test]
fn greeked_text_skips_the_stack() {
    let node = gradient_text("Hi");
    let mut backend = Capture::new(true);
    let mut cx = PaintCx {
        backend: &mut backend,
    };
    paint_text_in_viewport(&mut cx, &node, node.bounds, Point2D::ZERO, 0.05, &None);
    assert!(!backend.ops.contains(&Op::MaskSource), "{:?}", backend.ops);
    assert!(backend.ops.contains(&Op::FillRect), "{:?}", backend.ops);
}
