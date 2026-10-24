//! Shader textures and offscreen effect passes (#165) on a raster surface.

use crate::backend::{NativeBackend, NativeFrameBackend};
use de_editor_core::shader_images::encode_image_id;
use de_editor_core::shader_textures::{texture_id, TextureKind, OBJECT_SDF_TEXTURE};
use de_editor_ui::{Color, Rect, RenderBackend};

const SIDE: i32 = 40;

fn pixel(surface: &mut skia_safe::Surface, x: i32, y: i32) -> skia_safe::Color {
    surface
        .image_snapshot()
        .peek_pixels()
        .expect("raster pixels")
        .get_color((x, y))
}

fn node_box() -> Rect {
    Rect::xywh(0.0, 0.0, SIDE as f32, SIDE as f32)
}

fn color(r: f32, g: f32, b: f32) -> Color {
    Color { r, g, b, a: 1.0 }
}

/// Run `paint` on a fresh frame backend over a cleared raster surface.
fn paint(paint: impl FnOnce(&mut NativeFrameBackend<'_>)) -> skia_safe::Surface {
    let mut surface = skia_safe::surfaces::raster_n32_premul((SIDE, SIDE)).unwrap();
    surface.canvas().clear(skia_safe::Color::TRANSPARENT);
    let mut backend = NativeBackend::with_dpi(1.0);
    {
        let mut frame = NativeFrameBackend::new(&mut backend, surface.canvas());
        paint(&mut frame);
    }
    surface
}

fn shader_fill(frame: &mut NativeFrameBackend<'_>, sksl: &str, uniforms: &[(&str, &[f32])]) {
    frame.fill_round_rect_shader(node_box(), 0.0, sksl, uniforms, 1.0, color(1.0, 1.0, 1.0));
}

#[test]
fn a_fill_samples_the_texture_of_the_fills_under_it() {
    let id = texture_id("n", 0, TextureKind::Object);
    let bound = encode_image_id(id);
    let mut surface = paint(|frame| {
        frame.begin_shader_texture(id, node_box());
        frame.fill_rect(Rect::xywh(0.0, 0.0, 20.0, 40.0), color(1.0, 0.0, 0.0));
        frame.end_shader_texture(false, 1.0);
        // Swap red and green: the texture's left half turns green.
        shader_fill(
            frame,
            "uniform shader object; half4 main(float2 p) { half4 c = object.eval(p); return half4(c.g, c.r, c.b, 1.0); }",
            &[("object", &bound)],
        );
        frame.release_shader_texture(id);
    });
    let left = pixel(&mut surface, 10, 20);
    let right = pixel(&mut surface, 30, 20);
    assert!(left.g() > 240 && left.r() < 16, "{left:?}");
    assert!(
        right.r() < 16 && right.g() < 16,
        "nothing drawn there: {right:?}"
    );
}

#[test]
fn a_fill_samples_the_captured_backdrop() {
    let id = texture_id("n", 0, TextureKind::Background);
    let bound = encode_image_id(id);
    let mut surface = paint(|frame| {
        frame.fill_rect(node_box(), color(0.0, 0.0, 1.0));
        frame.capture_shader_backdrop(id, node_box());
        shader_fill(
            frame,
            "uniform shader background; half4 main(float2 p) { return half4(background.eval(p).b, 0.0, 0.0, 1.0); }",
            &[("background", &bound)],
        );
    });
    let middle = pixel(&mut surface, 20, 20);
    assert!(middle.r() > 240 && middle.b() < 16, "{middle:?}");
}

#[test]
fn an_sdf_child_reads_the_signed_distance_in_node_pixels() {
    let id = texture_id("n", 0, TextureKind::Sdf);
    let bound = encode_image_id(id);
    let mut surface = paint(|frame| {
        frame.begin_shader_texture(id, node_box());
        frame.fill_oval(node_box(), color(1.0, 1.0, 1.0));
        frame.end_shader_texture(true, 1.0);
        // Red: distance / 40 inside; green outside.
        shader_fill(
            frame,
            "/** @sdf */ uniform shader shape; half4 main(float2 p) { float d = shape.eval(p).r; \
             return d > 0.0 ? half4(d / 40.0, 0.0, 0.0, 1.0) : half4(0.0, 1.0, 0.0, 1.0); }",
            &[("shape", &bound)],
        );
    });
    let centre = pixel(&mut surface, 20, 20);
    // About 20 px inside: 0.5 of the red range.
    assert!(
        (centre.r() as i32 - 128).abs() < 20 && centre.g() < 16,
        "{centre:?}"
    );
    let corner = pixel(&mut surface, 1, 1);
    assert!(corner.g() > 240, "{corner:?}");
}

#[test]
fn an_effect_pass_samples_the_node_and_its_backdrop_together() {
    let id = texture_id("n", 0, TextureKind::Background);
    let bound = encode_image_id(id);
    let mut surface = paint(|frame| {
        frame.fill_rect(node_box(), color(0.0, 0.0, 1.0));
        frame.capture_shader_backdrop(id, node_box());
        frame.push_shader_effect_layer(
            node_box(),
            "uniform shader object; uniform shader background; \
             half4 main(float2 p) { return object.eval(p) + background.eval(p); }",
            &[("background", &bound)],
            false,
        );
        frame.fill_rect(Rect::xywh(0.0, 0.0, 20.0, 40.0), color(1.0, 0.0, 0.0));
        frame.restore();
    });
    // Node red + backdrop blue on the left; the right shows the backdrop
    // the program drew over the (unchanged) blue.
    let left = pixel(&mut surface, 10, 20);
    assert!(left.r() > 240 && left.b() > 240, "magenta: {left:?}");
    let right = pixel(&mut surface, 30, 20);
    assert!(right.b() > 240 && right.r() < 16, "{right:?}");
}

#[test]
fn an_effect_sdf_child_follows_the_rendered_content() {
    let sdf = encode_image_id(OBJECT_SDF_TEXTURE);
    let mut surface = paint(|frame| {
        frame.push_shader_effect_layer(
            node_box(),
            "uniform shader object; /** @sdf */ uniform shader edge; \
             half4 main(float2 p) { float d = edge.eval(p).r; \
             return abs(d) < 2.0 ? half4(0.0, 1.0, 0.0, 1.0) : object.eval(p); }",
            &[("edge", &sdf)],
            false,
        );
        frame.fill_rect(Rect::xywh(10.0, 10.0, 20.0, 20.0), color(1.0, 0.0, 0.0));
        frame.restore();
    });
    // The square's edge turns into a green outline; its middle stays red.
    let edge = pixel(&mut surface, 10, 20);
    assert!(edge.g() > 240, "{edge:?}");
    let middle = pixel(&mut surface, 20, 20);
    assert!(middle.r() > 240 && middle.g() < 16, "{middle:?}");
}

#[test]
fn an_image_child_binds_in_an_effect_pass_and_a_redirect_stays_balanced() {
    let mut surface = paint(|frame| {
        let base = frame.save_count_for_tests();
        // An unknown image reads transparent; the program still runs.
        let missing = encode_image_id(42);
        frame.push_shader_effect_layer(
            node_box(),
            "uniform shader object; uniform shader photo; \
             half4 main(float2 p) { return object.eval(p) + photo.eval(p); }",
            &[("photo", &missing)],
            false,
        );
        frame.fill_rect(node_box(), color(0.0, 1.0, 0.0));
        frame.restore();
        assert_eq!(frame.save_count_for_tests(), base);
    });
    let middle = pixel(&mut surface, 20, 20);
    assert!(middle.g() > 240, "{middle:?}");
}

/// Chained passes: an inner pass opened inside an outer offscreen pass
/// still reads the frame's backdrop, not the outer pass's empty capture.
#[test]
fn a_backdrop_captured_inside_an_open_pass_reads_the_frame() {
    let outer = texture_id("n", 1, TextureKind::Background);
    let inner = texture_id("n", 0, TextureKind::Background);
    let program = "uniform shader object; uniform shader background; \
                   half4 main(float2 p) { half4 n = object.eval(p); return n + background.eval(p) * (1.0 - n.a); }";
    let mut surface = paint(|frame| {
        frame.fill_rect(node_box(), color(0.0, 0.0, 1.0));
        frame.capture_shader_backdrop(outer, node_box());
        frame.push_shader_effect_layer(
            node_box(),
            program,
            &[("background", &encode_image_id(outer))],
            false,
        );
        frame.capture_shader_backdrop(inner, node_box());
        frame.push_shader_effect_layer(
            node_box(),
            program,
            &[("background", &encode_image_id(inner))],
            false,
        );
        frame.fill_rect(Rect::xywh(0.0, 0.0, 20.0, 40.0), color(1.0, 0.0, 0.0));
        frame.restore();
        frame.restore();
    });
    // Right half: no node, both passes show the blue backdrop.
    let right = pixel(&mut surface, 30, 20);
    assert!(right.b() > 240, "{right:?}");
    let left = pixel(&mut surface, 10, 20);
    assert!(left.r() > 240 && left.b() < 16, "{left:?}");
}

/// A texture of a box partly off the canvas keeps its place: the fill
/// samples the node where it is, not squeezed into the visible part.
#[test]
fn a_texture_partly_off_the_canvas_keeps_its_mapping() {
    let id = texture_id("n", 0, TextureKind::Object);
    let bound = encode_image_id(id);
    let wide = Rect::xywh(0.0, 0.0, 80.0, 40.0);
    let mut surface = paint(|frame| {
        frame.begin_shader_texture(id, wide);
        frame.fill_rect(Rect::xywh(0.0, 0.0, 20.0, 40.0), color(1.0, 0.0, 0.0));
        frame.end_shader_texture(false, 1.0);
        frame.fill_round_rect_shader(
            wide,
            0.0,
            "uniform shader object; half4 main(float2 p) { half4 c = object.eval(p); return half4(c.g, c.r, c.b, 1.0); }",
            &[("object", &bound)],
            1.0,
            color(1.0, 1.0, 1.0),
        );
    });
    let left = pixel(&mut surface, 10, 20);
    let right = pixel(&mut surface, 30, 20);
    assert!(left.g() > 240, "{left:?}");
    assert!(right.g() < 16, "{right:?}");
}
