use super::*;
use de_editor_core::shader_images::encode_image_id;
use de_editor_ui::Color;

const PHOTO: &str = "uniform shader photo; half4 main(float2 p){ return photo.eval(p); }";

/// A 2×1 raster: red on the left, blue on the right.
fn red_blue() -> skia_safe::Image {
    let mut surface = skia_safe::surfaces::raster_n32_premul((2, 1)).expect("surface");
    let canvas = surface.canvas();
    let mut paint = skia_safe::Paint::default();
    paint.set_color(skia_safe::Color::RED);
    canvas.draw_rect(skia_safe::Rect::from_xywh(0.0, 0.0, 1.0, 1.0), &paint);
    paint.set_color(skia_safe::Color::BLUE);
    canvas.draw_rect(skia_safe::Rect::from_xywh(1.0, 0.0, 1.0, 1.0), &paint);
    surface.image_snapshot()
}

/// Paint `sksl` over a 40×20 rect and return the RGBA of two sample points.
fn paint(
    backend: &mut NativeBackend,
    sksl: &str,
    uniforms: &[(&str, &[f32])],
) -> ([u8; 4], [u8; 4]) {
    let info = skia_safe::ImageInfo::new(
        (40, 20),
        skia_safe::ColorType::RGBA8888,
        skia_safe::AlphaType::Premul,
        None,
    );
    let mut surface = skia_safe::surfaces::raster(&info, None, None).expect("surface");
    backend.fill_round_rect_shader(
        surface.canvas(),
        Rect::xywh(0.0, 0.0, 40.0, 20.0),
        0.0,
        sksl,
        uniforms,
        1.0,
        Color::GREEN,
    );
    let image = surface.image_snapshot();
    let pixmap = image.peek_pixels().expect("pixels");
    let bytes = pixmap.bytes().expect("bytes");
    let at = |x: usize, y: usize| -> [u8; 4] {
        let i = (y * 40 + x) * 4;
        [bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]]
    };
    (at(5, 10), at(35, 10))
}

#[test]
fn an_image_child_samples_the_raster_stretched_over_the_node() {
    let mut backend = NativeBackend::with_dpi(1.0);
    backend.install_raster_image(41, red_blue(), u32::MAX);
    let id = encode_image_id(41);
    let (left, right) = paint(&mut backend, PHOTO, &[("photo", &id)]);
    assert!(
        left[0] > 200 && left[2] < 60 && left[3] == 255,
        "left is red: {left:?}"
    );
    assert!(
        right[2] > 200 && right[0] < 60 && right[3] == 255,
        "right is blue: {right:?}"
    );
}

#[test]
fn the_size_uniform_sets_the_box_the_image_stretches_over() {
    let mut backend = NativeBackend::with_dpi(1.0);
    backend.install_raster_image(42, red_blue(), u32::MAX);
    let id = encode_image_id(42);
    let size = [20.0, 10.0];
    let sksl =
        "uniform shader photo; uniform float2 size; half4 main(float2 p){ return photo.eval(p); }";
    // The local matrix scales the 20×10 document box onto the 40×20 rect, so
    // the image still spans the whole node.
    let (left, right) = paint(&mut backend, sksl, &[("photo", &id), ("size", &size)]);
    assert!(left[0] > 200 && left[2] < 60, "left is red: {left:?}");
    assert!(right[2] > 200 && right[0] < 60, "right is blue: {right:?}");
}

#[test]
fn float_uniforms_still_bind_next_to_an_image_child() {
    let mut backend = NativeBackend::with_dpi(1.0);
    backend.install_raster_image(43, red_blue(), u32::MAX);
    let id = encode_image_id(43);
    let half = [0.5];
    let sksl = "uniform shader photo; uniform float gain; half4 main(float2 p){ return photo.eval(p) * gain; }";
    let (left, _) = paint(&mut backend, sksl, &[("gain", &half), ("photo", &id)]);
    assert!(
        (110..=145).contains(&left[0]),
        "gain halves the red: {left:?}"
    );
}

#[test]
fn a_missing_image_samples_transparent_instead_of_the_fallback() {
    let mut backend = NativeBackend::with_dpi(1.0);
    let id = encode_image_id(44);
    let (left, right) = paint(&mut backend, PHOTO, &[("photo", &id)]);
    assert_eq!(
        left[3], 0,
        "not resident: transparent, not the green fallback"
    );
    assert_eq!(right[3], 0);
    let (unbound, _) = paint(&mut backend, PHOTO, &[]);
    assert_eq!(unbound[3], 0, "no binding at all also runs the program");
}
