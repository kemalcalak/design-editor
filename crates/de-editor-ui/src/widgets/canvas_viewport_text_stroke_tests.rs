//! Paint-order tests for outlined text (#282): the outline's alignment
//! decides whether it paints under the fill, over it, or over it cut to
//! the glyphs.

use crate::layout_scene::{
    NodeKind, SceneNode, SceneStroke, SceneStrokeAlign, SceneWidget, StrokeDash, StrokeStyle,
};
use crate::widgets::canvas_viewport::EditCaret;
use crate::widgets::canvas_viewport_text_fill::paint_text_in_viewport;
use crate::widgets::PaintCx;
use crate::{Color, ImageBlendMode, Point2D, Rect, RenderBackend, TextLayout};

#[derive(Debug, PartialEq)]
enum Op {
    Fill,
    /// A glyph fill in the opaque mask ink.
    Coverage,
    Stroke(f32),
    Composite,
    MaskSource,
    Restore,
    /// The first dash length a stroke style set.
    Dash(Option<f32>),
    /// A solid rect: the caret (or the greeking bar).
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

    /// The ops without the stroke-style bookkeeping.
    fn paint_ops(&self) -> Vec<&Op> {
        self.ops
            .iter()
            .filter(|op| !matches!(op, Op::Dash(_)))
            .collect()
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
        let mask = layout
            .runs()
            .first()
            .is_some_and(|run| run.color == Color::WHITE.to_jian());
        self.ops.push(if mask { Op::Coverage } else { Op::Fill });
    }
    fn stroke_text(&mut self, _: &TextLayout, _: Point2D, _: Color, width: f32) {
        self.ops.push(Op::Stroke(width));
    }
    fn set_stroke_style(&mut self, style: StrokeStyle) {
        self.ops
            .push(Op::Dash(style.dash.map(|dash| dash.lengths()[0])));
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
    fn push_composite_layer(&mut self, _: Rect, _: f32, _: ImageBlendMode) {
        self.ops.push(Op::Composite);
    }
    fn push_mask_source_layer(&mut self, _: bool) {
        self.ops.push(Op::MaskSource);
    }
}

fn outlined(align: SceneStrokeAlign) -> SceneNode {
    let mut node = SceneNode::leaf("t", NodeKind::Text);
    node.bounds = Rect::xywh(0.0, 0.0, 200.0, 40.0);
    node.text = Some("A".to_string());
    node.font_size = 20.0;
    node.fill = Some(Color::BLACK);
    node.stroke = Some(SceneStroke {
        color: Color {
            r: 1.0,
            g: 0.0,
            b: 1.0,
            a: 1.0,
        },
        width: 2.0,
        sides: None,
        align,
        style: StrokeStyle::default(),
    });
    node
}

fn paint(backend: &mut Capture, node: &SceneNode, zoom: f32) {
    let mut cx = PaintCx { backend };
    paint_text_in_viewport(&mut cx, node, node.bounds, Point2D::ZERO, zoom, &None);
}

fn painted(node: &SceneNode, pixel_masks: bool) -> Vec<Op> {
    let mut backend = Capture::new(pixel_masks);
    paint(&mut backend, node, 1.0);
    backend
        .ops
        .into_iter()
        .filter(|op| !matches!(op, Op::Dash(_) | Op::Restore))
        .collect()
}

#[test]
fn a_centred_outline_strokes_over_the_fill_at_its_width() {
    let ops = painted(&outlined(SceneStrokeAlign::Center), true);
    assert_eq!(ops, [Op::Fill, Op::Stroke(2.0)]);
}

#[test]
fn an_outside_outline_strokes_twice_the_width_under_the_fill() {
    let ops = painted(&outlined(SceneStrokeAlign::Outside), true);
    assert_eq!(ops, [Op::Stroke(4.0), Op::Fill]);
}

#[test]
fn an_inside_outline_is_cut_to_the_glyph_coverage() {
    let node = outlined(SceneStrokeAlign::Inside);
    let ops = painted(&node, true);
    assert_eq!(
        ops,
        [
            Op::Fill,
            Op::Composite,
            Op::Stroke(4.0),
            Op::MaskSource,
            Op::Coverage
        ]
    );
    let mut backend = Capture::new(true);
    paint(&mut backend, &node, 1.0);
    let restores = backend.paint_ops().len() - ops.len();
    // Every scope closes: the fill's, the outline's and the coverage's
    // doc-space scopes, then the mask and the layer.
    assert_eq!(restores, 5);
}

#[test]
fn an_inside_outline_strokes_centred_without_pixel_masks() {
    let ops = painted(&outlined(SceneStrokeAlign::Inside), false);
    assert_eq!(ops, [Op::Fill, Op::Stroke(2.0)]);
}

#[test]
fn widget_text_keeps_its_stroke_as_the_field_border() {
    let mut node = outlined(SceneStrokeAlign::Center);
    node.widget = Some(SceneWidget {
        kind: "text_input".into(),
        ..Default::default()
    });
    assert_eq!(painted(&node, true), [Op::Fill]);
}

#[test]
fn greeked_text_gets_no_outline() {
    let node = outlined(SceneStrokeAlign::Center);
    let mut backend = Capture::new(true);
    paint(&mut backend, &node, 0.01);
    assert!(!backend.ops.iter().any(|op| matches!(op, Op::Stroke(_))));
}

#[test]
fn the_dash_is_unscaled_on_the_glyphs_and_restored_for_the_screen() {
    let mut node = outlined(SceneStrokeAlign::Center);
    let stroke = node.stroke.as_mut().unwrap();
    stroke.style.dash = StrokeDash::new(&[3.0, 1.0], 0.0);
    let mut backend = Capture::new(true);
    paint(&mut backend, &node, 2.0);
    let stroke_at = backend
        .ops
        .iter()
        .position(|op| matches!(op, Op::Stroke(_)))
        .unwrap();
    assert_eq!(backend.ops[stroke_at - 1], Op::Dash(Some(3.0)));
    assert_eq!(backend.ops[stroke_at + 1], Op::Dash(Some(6.0)));
}

#[test]
fn the_caret_is_painted_again_over_a_centred_outline_while_editing() {
    let node = outlined(SceneStrokeAlign::Center);
    let edit = Some(EditCaret {
        editing: "t".to_string(),
        input: jian_core::text_input::TextInputState::with_text("A"),
        now_ms: 0,
        selection_color: Color::BLACK,
    });
    let mut backend = Capture::new(true);
    let mut cx = PaintCx {
        backend: &mut backend,
    };
    paint_text_in_viewport(&mut cx, &node, node.bounds, Point2D::ZERO, 1.0, &edit);
    let ops = backend.paint_ops();
    let stroke = ops
        .iter()
        .position(|op| matches!(op, Op::Stroke(_)))
        .expect("the outline paints");
    assert!(
        ops[..stroke].contains(&&Op::FillRect),
        "the fill pass paints the caret"
    );
    assert!(
        ops[stroke..].contains(&&Op::FillRect),
        "and it is painted again over the outline: {ops:?}"
    );
}

#[test]
fn an_edited_node_is_outlined_from_its_live_buffer() {
    let mut node = outlined(SceneStrokeAlign::Center);
    node.text = Some(String::new());
    let edit = Some(EditCaret {
        editing: "t".to_string(),
        input: jian_core::text_input::TextInputState::with_text("typed"),
        now_ms: 0,
        selection_color: Color::BLACK,
    });
    let mut backend = Capture::new(true);
    let mut cx = PaintCx {
        backend: &mut backend,
    };
    paint_text_in_viewport(&mut cx, &node, node.bounds, Point2D::ZERO, 1.0, &edit);
    assert!(backend.ops.iter().any(|op| matches!(op, Op::Stroke(_))));
}
