use super::*;
use crate::{Color, Point2D, RenderBackend, TextLayout};
use de_editor_core::shader_images::{encode_image_id, register_image_source};

/// Records the decode checks paint asks the backend for.
#[derive(Default)]
struct DecodeRecorder {
    decoded: Vec<(u64, Vec<u8>, u32)>,
}

impl RenderBackend for DecodeRecorder {
    fn begin_frame(&mut self) {}
    fn end_frame(&mut self) {}
    fn fill_rect(&mut self, _: Rect, _: Color) {}
    fn stroke_rect(&mut self, _: Rect, _: Color, _: f32) {}
    fn draw_text(&mut self, _: &TextLayout, _: Point2D) {}
    fn clip_rect(&mut self, _: Rect) {}
    fn stroke_line(&mut self, _: Point2D, _: Point2D, _: Color, _: f32) {}
    fn fill_round_rect(&mut self, _: Rect, _: f32, _: Color) {}
    fn stroke_round_rect(&mut self, _: Rect, _: f32, _: Color, _: f32) {}
    fn stroke_svg_path(&mut self, _: &str, _: Point2D, _: f32, _: Color, _: f32) {}
    fn save(&mut self) {}
    fn restore(&mut self) {}
    fn translate(&mut self, _: Point2D) {}
    fn resize(&mut self, _: u32, _: u32) {}
    fn dpi_scale(&self) -> f32 {
        2.0
    }
    fn image_decoded(&mut self, id: u64, encoded: &[u8], max_edge_px: u32) -> bool {
        self.decoded.push((id, encoded.to_vec(), max_edge_px));
        true
    }
}

const SKSL: &str = "uniform shader photo; uniform half4 tint; uniform shader mask;\n\
                    half4 main(float2 p){ return photo.eval(p) * mask.eval(p).a * tint; }";

#[test]
fn image_ids_follow_child_declaration_order_and_skip_unbound_children() {
    let photo = encode_image_id(7);
    let tint = [1.0, 0.0, 0.0, 1.0];
    let uniforms: Vec<(&str, &[f32])> = vec![("tint", &tint), ("photo", &photo)];
    // `tint` is a vec4 that happens to decode, but it is not a child; `mask`
    // is a child without a binding.
    assert_eq!(shader_image_ids(SKSL, &uniforms), vec![7]);
    assert!(shader_image_ids("half4 main(float2 p){ return half4(1); }", &uniforms).is_empty());
}

#[test]
fn a_bound_image_is_made_resident_at_the_on_screen_size() {
    // "shader-texture" as base64; any bytes do, the recorder never decodes.
    let src = "data:image/png;base64,c2hhZGVyLXRleHR1cmU=";
    let id = register_image_source(src);
    let encoded = encode_image_id(id);
    let uniforms: Vec<(&str, &[f32])> = vec![("photo", &encoded)];
    let mut backend = DecodeRecorder::default();
    {
        let mut cx = PaintCx {
            backend: &mut backend,
        };
        prepare_shader_images(&mut cx, SKSL, &uniforms, Rect::xywh(0.0, 0.0, 100.0, 60.0));
    }
    assert_eq!(backend.decoded.len(), 1);
    let (decoded_id, bytes, max_edge) = &backend.decoded[0];
    assert_eq!(*decoded_id, id);
    assert_eq!(bytes.as_slice(), b"shader-texture");
    assert_eq!(
        *max_edge,
        required_raster_edge(Rect::xywh(0.0, 0.0, 100.0, 60.0), 2.0)
    );
}

#[test]
fn an_unknown_image_id_asks_the_backend_for_nothing() {
    let encoded = encode_image_id(0xdead_beef_0000_0001);
    let uniforms: Vec<(&str, &[f32])> = vec![("photo", &encoded)];
    let mut backend = DecodeRecorder::default();
    {
        let mut cx = PaintCx {
            backend: &mut backend,
        };
        prepare_shader_images(&mut cx, SKSL, &uniforms, Rect::xywh(0.0, 0.0, 10.0, 10.0));
    }
    assert!(backend.decoded.is_empty());
}
