//! Shader-effect layer order tests (#6) for `canvas_viewport_paint.rs`.

use crate::layout_scene::{
    Effect, NodeKind, SceneNode, SceneShaderUniform, ShaderEffect, ShaderEffectInput,
};
use crate::widgets::canvas_viewport_paint::paint_node;
use crate::widgets::PaintCx;
use crate::{Color, Point2D, Rect, RenderBackend, TextLayout};

#[derive(Default)]
struct EffectCaptureBackend {
    ops: Vec<String>,
}

impl RenderBackend for EffectCaptureBackend {
    fn begin_frame(&mut self) {}
    fn end_frame(&mut self) {}
    fn fill_rect(&mut self, _: Rect, _: Color) {
        self.ops.push("fill".into());
    }
    fn stroke_rect(&mut self, _: Rect, _: Color, _: f32) {}
    fn draw_text(&mut self, _: &TextLayout, _: Point2D) {}
    fn clip_rect(&mut self, _: Rect) {
        self.ops.push("clip".into());
    }
    fn clip_round_rect(&mut self, _: Rect, _: f32) {
        self.ops.push("clip_round".into());
    }
    fn save(&mut self) {
        self.ops.push("save".into());
    }
    fn restore(&mut self) {
        self.ops.push("restore".into());
    }
    fn push_blur_layer(&mut self, _: f32) {
        self.ops.push("blur".into());
    }
    fn push_shader_effect_layer(
        &mut self,
        _: Rect,
        sksl: &str,
        uniforms: &[(&str, &[f32])],
        backdrop: bool,
    ) {
        let size = uniforms
            .iter()
            .find(|(name, _)| *name == "size")
            .map(|(_, v)| format!("{v:?}"))
            .unwrap_or_default();
        let kind = if backdrop { "backdrop" } else { "object" };
        self.ops.push(format!("{kind}:{sksl}:{size}"));
    }
    fn translate(&mut self, _: Point2D) {}
    fn stroke_line(&mut self, _: Point2D, _: Point2D, _: Color, _: f32) {}
    fn fill_round_rect(&mut self, _: Rect, _: f32, _: Color) {
        self.ops.push("fill".into());
    }
    fn stroke_round_rect(&mut self, _: Rect, _: f32, _: Color, _: f32) {}
    fn stroke_svg_path(&mut self, _: &str, _: Point2D, _: f32, _: Color, _: f32) {}
    fn resize(&mut self, _: u32, _: u32) {}
    fn dpi_scale(&self) -> f32 {
        1.0
    }
}

fn pass(sksl: &str, input: ShaderEffectInput) -> Effect {
    Effect::Shader(ShaderEffect {
        sksl: sksl.to_string(),
        uniforms: vec![SceneShaderUniform {
            name: "size".to_string(),
            values: vec![100.0, 60.0],
        }],
        input,
    })
}

fn paint(node: &SceneNode) -> Vec<String> {
    let mut backend = EffectCaptureBackend::default();
    paint_node(
        &mut PaintCx {
            backend: &mut backend,
        },
        node,
        Point2D::ZERO,
        1.0,
        Rect::xywh(-100.0, -100.0, 1000.0, 1000.0),
    );
    backend.ops
}

#[test]
fn shader_passes_chain_in_list_order_around_the_node_paint() {
    let mut node = SceneNode::leaf("fx", NodeKind::Rect);
    node.bounds = Rect::xywh(0.0, 0.0, 100.0, 60.0);
    node.fill = Some(Color::BLACK);
    node.effects = vec![
        pass("o1", ShaderEffectInput::Object),
        pass("b1", ShaderEffectInput::Background),
        pass("o2", ShaderEffectInput::Object),
        pass("b2", ShaderEffectInput::Background),
    ];
    let size = "[100.0, 60.0]";
    assert_eq!(
        paint(&node),
        vec![
            // Backdrop passes: silhouette clip, then list order.
            "save".to_string(),
            "clip".to_string(),
            format!("backdrop:b1:{size}"),
            format!("backdrop:b2:{size}"),
            // Object passes: last listed outermost, so `o1` runs first.
            format!("object:o2:{size}"),
            format!("object:o1:{size}"),
            "fill".to_string(),
            "restore".to_string(),
            "restore".to_string(),
            "restore".to_string(),
            "restore".to_string(),
            "restore".to_string(),
        ]
    );
}

#[test]
fn object_passes_wrap_the_layer_blur() {
    let mut node = SceneNode::leaf("fx", NodeKind::Rect);
    node.bounds = Rect::xywh(0.0, 0.0, 100.0, 60.0);
    node.fill = Some(Color::BLACK);
    node.effects = vec![
        Effect::Blur(crate::layout_scene::BlurEffect { radius: 8.0 }),
        pass("o1", ShaderEffectInput::Object),
    ];
    let ops = paint(&node);
    let object = ops.iter().position(|op| op.starts_with("object:")).unwrap();
    let blur = ops.iter().position(|op| op == "blur").unwrap();
    assert!(
        object < blur,
        "the shader samples the blurred output: {ops:?}"
    );
}

#[test]
fn a_zero_size_node_opens_no_shader_layer() {
    let mut node = SceneNode::leaf("fx", NodeKind::Rect);
    node.bounds = Rect::xywh(0.0, 0.0, 0.0, 60.0);
    node.effects = vec![pass("o1", ShaderEffectInput::Object)];
    assert!(!paint(&node).iter().any(|op| op.starts_with("object:")));
}
