use super::*;
use crate::layout_scene::{NodeKind, SceneShader};
use crate::{Point2D, RenderBackend, TextLayout};
use de_editor_core::shader_images::decode_image_id;
use de_editor_core::shader_textures::texture_kind;

#[derive(Debug, PartialEq)]
enum Op {
    Backdrop(u64),
    Begin(u64),
    End { sdf: bool },
    Fill(u8),
    Shader(Vec<(String, Option<TextureKind>)>),
    Release(u64),
}

#[derive(Default)]
struct Recorder {
    ops: Vec<Op>,
}

impl RenderBackend for Recorder {
    fn begin_frame(&mut self) {}
    fn end_frame(&mut self) {}
    fn fill_rect(&mut self, _: Rect, color: Color) {
        self.ops.push(Op::Fill((color.r * 255.0).round() as u8));
    }
    fn stroke_rect(&mut self, _: Rect, _: Color, _: f32) {}
    fn draw_text(&mut self, _: &TextLayout, _: Point2D) {}
    fn clip_rect(&mut self, _: Rect) {}
    fn stroke_line(&mut self, _: Point2D, _: Point2D, _: Color, _: f32) {}
    fn fill_round_rect(&mut self, _: Rect, _: f32, _: Color) {}
    fn stroke_round_rect(&mut self, _: Rect, _: f32, _: Color, _: f32) {}
    fn stroke_svg_path(&mut self, _: &str, _: Point2D, _: f32, _: Color, _: f32) {}
    fn fill_round_rect_shader(
        &mut self,
        _: Rect,
        _: f32,
        _: &str,
        uniforms: &[(&str, &[f32])],
        _: f32,
        _: Color,
    ) {
        let bound = uniforms
            .iter()
            .filter_map(|(name, values)| {
                decode_image_id(values).map(|id| (name.to_string(), texture_kind(id)))
            })
            .collect();
        self.ops.push(Op::Shader(bound));
    }
    fn save(&mut self) {}
    fn restore(&mut self) {}
    fn translate(&mut self, _: Point2D) {}
    fn resize(&mut self, _: u32, _: u32) {}
    fn dpi_scale(&self) -> f32 {
        1.0
    }
    fn supports_shader_textures(&self) -> bool {
        true
    }
    fn capture_shader_backdrop(&mut self, id: u64, _: Rect) {
        self.ops.push(Op::Backdrop(id));
    }
    fn begin_shader_texture(&mut self, id: u64, _: Rect) {
        self.ops.push(Op::Begin(id));
    }
    fn end_shader_texture(&mut self, sdf: bool, _: f32) {
        self.ops.push(Op::End { sdf });
    }
    fn release_shader_texture(&mut self, id: u64) {
        self.ops.push(Op::Release(id));
    }
    fn shader_texture_ready(&self, _: u64) -> bool {
        false
    }
}

fn solid(red: u8) -> SceneFillLayer {
    SceneFillLayer::Solid {
        color: Color {
            r: f32::from(red) / 255.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
        blend_mode: ImageBlendMode::Normal,
    }
}

#[test]
fn a_fill_sampling_the_node_backdrop_and_sdf_gets_its_textures() {
    let sksl = "uniform shader object; uniform shader background; /** @sdf */ uniform shader shape;\n\
                half4 main(float2 p) { return object.eval(p) + background.eval(p) + shape.eval(p); }";
    let mut node = SceneNode::leaf("n", NodeKind::Rect);
    node.bounds = Rect::xywh(0.0, 0.0, 10.0, 10.0);
    node.fill_layers = vec![
        SceneFillLayer::Shader {
            shader: SceneShader {
                sksl: sksl.into(),
                uniforms: Vec::new(),
                opacity: 1.0,
                fallback: Color::BLACK,
            },
            blend_mode: ImageBlendMode::Normal,
        },
        solid(7),
    ];
    let mut backend = Recorder::default();
    let rect = Rect::xywh(0.0, 0.0, 20.0, 20.0);
    assert!(
        crate::widgets::canvas_viewport_fill_layers::paint_fill_layers(
            &mut PaintCx {
                backend: &mut backend,
            },
            &node,
            rect,
            2.0,
        )
    );
    let background = texture_id("n", usize::MAX, TextureKind::Background);
    let object = texture_id("n", 0, TextureKind::Object);
    let sdf = texture_id(&silhouette_key(&node, rect), 0, TextureKind::Sdf);
    assert_eq!(
        backend.ops,
        [
            Op::Backdrop(background),
            // The lower solid paints normally first.
            Op::Fill(7),
            // Then again into the object texture, then the silhouette.
            Op::Begin(object),
            Op::Fill(7),
            Op::End { sdf: false },
            Op::Begin(sdf),
            Op::Fill(255),
            Op::End { sdf: true },
            Op::Shader(vec![
                ("object".into(), Some(TextureKind::Object)),
                ("background".into(), Some(TextureKind::Background)),
                ("shape".into(), Some(TextureKind::Sdf)),
            ]),
            // The SDF stays for the next frame.
            Op::Release(object),
            Op::Release(background),
        ]
    );
}

#[test]
fn image_only_fills_need_no_textures() {
    let mut node = SceneNode::leaf("n", NodeKind::Rect);
    node.fill_layers = vec![SceneFillLayer::Shader {
        shader: SceneShader {
            sksl: "uniform shader photo; half4 main(float2 p) { return photo.eval(p); }".into(),
            uniforms: Vec::new(),
            opacity: 1.0,
            fallback: Color::BLACK,
        },
        blend_mode: ImageBlendMode::Normal,
    }];
    let mut backend = Recorder::default();
    crate::widgets::canvas_viewport_fill_layers::paint_fill_layers(
        &mut PaintCx {
            backend: &mut backend,
        },
        &node,
        Rect::xywh(0.0, 0.0, 10.0, 10.0),
        1.0,
    );
    assert_eq!(backend.ops, [Op::Shader(Vec::new())]);
}
