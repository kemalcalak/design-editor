use super::*;

fn surface(clear: skia_safe::Color) -> skia_safe::Surface {
    let mut surface = skia_safe::surfaces::raster_n32_premul((20, 20)).unwrap();
    surface.canvas().clear(clear);
    surface
}

fn pixel(surface: &mut skia_safe::Surface, x: i32, y: i32) -> skia_safe::Color {
    surface
        .image_snapshot()
        .peek_pixels()
        .expect("raster pixels")
        .get_color((x, y))
}

fn fill(canvas: &skia_safe::Canvas, color: skia_safe::Color) {
    let paint = skia_safe::Paint::new(skia_safe::Color4f::from(color), None);
    canvas.draw_rect(skia_safe::Rect::from_wh(20.0, 20.0), &paint);
}

fn node_box() -> Rect {
    Rect::xywh(0.0, 0.0, 20.0, 20.0)
}

/// Paints its left document half green and passes the rest through.
const SPLIT: &str = "uniform shader object;
half4 main(float2 p) {
    half4 c = object.eval(p);
    return p.x < 5.0 ? half4(0.0, c.a, 0.0, c.a) : c;
}";

#[test]
fn object_pass_filters_the_layer_in_document_space() {
    let mut backend = NativeBackend::with_dpi(1.0);
    let mut surface = surface(skia_safe::Color::TRANSPARENT);
    let size = [10.0_f32, 10.0];
    let uniforms: [(&str, &[f32]); 1] = [("size", &size)];
    let base = surface.canvas().save_count();
    backend.push_shader_effect_layer(surface.canvas(), node_box(), SPLIT, &uniforms, false);
    fill(surface.canvas(), skia_safe::Color::RED);
    backend.restore(surface.canvas());
    assert_eq!(
        surface.canvas().save_count(),
        base,
        "one restore pops the pass"
    );

    // Document x < 5 is paint x < 10 (the box is 2x the `size`).
    let left = pixel(&mut surface, 5, 10);
    let right = pixel(&mut surface, 15, 10);
    assert!(left.g() > 240 && left.r() < 16, "left half green: {left:?}");
    assert!(
        right.r() > 240 && right.g() < 16,
        "right half red: {right:?}"
    );
}

#[test]
fn backdrop_pass_reads_what_is_behind_the_node() {
    let mut backend = NativeBackend::with_dpi(1.0);
    let mut surface = surface(skia_safe::Color::RED);
    let sksl = "uniform shader background;
half4 main(float2 p) { return background.eval(p).bgra; }";
    let base = surface.canvas().save_count();
    backend.push_shader_effect_layer(surface.canvas(), node_box(), sksl, &[], true);
    backend.restore(surface.canvas());
    assert_eq!(surface.canvas().save_count(), base);

    let center = pixel(&mut surface, 10, 10);
    assert!(
        center.b() > 240 && center.r() < 16,
        "red backdrop swizzled to blue: {center:?}"
    );
}

#[test]
fn a_program_without_its_child_degrades_to_a_plain_save() {
    let mut backend = NativeBackend::with_dpi(1.0);
    let mut surface = surface(skia_safe::Color::TRANSPARENT);
    let base = surface.canvas().save_count();
    for sksl in [
        "half4 main(float2 p) { return half4(1); }",
        "this is not sksl",
    ] {
        backend.push_shader_effect_layer(surface.canvas(), node_box(), sksl, &[], false);
        assert_eq!(surface.canvas().save_count(), base + 1);
        fill(surface.canvas(), skia_safe::Color::RED);
        backend.restore(surface.canvas());
        assert_eq!(surface.canvas().save_count(), base);
    }
    assert!(pixel(&mut surface, 10, 10).r() > 240, "node unaffected");
}

#[test]
fn object_pass_keeps_device_resolution_under_a_scaled_node() {
    let mut backend = NativeBackend::with_dpi(1.0);
    let mut surface = surface(skia_safe::Color::TRANSPARENT);
    // 5x5 document px over 20x20 paint px: a 4x node-local scale.
    let size = [5.0_f32, 5.0];
    let uniforms: [(&str, &[f32]); 1] = [("size", &size)];
    let identity = "uniform shader object; half4 main(float2 p) { return object.eval(p); }";
    backend.push_shader_effect_layer(surface.canvas(), node_box(), identity, &uniforms, false);
    let red = skia_safe::Paint::new(skia_safe::Color4f::from(skia_safe::Color::RED), None);
    // An edge at paint x = 9: between document pixels 2 and 3.
    surface
        .canvas()
        .draw_rect(skia_safe::Rect::from_wh(9.0, 20.0), &red);
    backend.restore(surface.canvas());
    let inside = pixel(&mut surface, 8, 10);
    let outside = pixel(&mut surface, 9, 10);
    assert!(inside.a() > 240, "inside the edge stays opaque: {inside:?}");
    assert!(
        outside.a() < 16,
        "the edge stays one device pixel sharp: {outside:?}"
    );
}

#[test]
fn object_pass_maps_document_space_under_a_hidpi_canvas_matrix() {
    let mut backend = NativeBackend::with_dpi(2.0);
    let mut surface = surface(skia_safe::Color::TRANSPARENT);
    surface.canvas().scale((2.0, 2.0));
    let rect = Rect::xywh(0.0, 0.0, 10.0, 10.0);
    let size = [10.0_f32, 10.0];
    let uniforms: [(&str, &[f32]); 1] = [("size", &size)];
    backend.push_shader_effect_layer(surface.canvas(), rect, SPLIT, &uniforms, false);
    fill(surface.canvas(), skia_safe::Color::RED);
    backend.restore(surface.canvas());
    assert_eq!(
        surface.canvas().local_to_device_as_3x3().scale_x(),
        2.0,
        "matrix restored"
    );
    // Document x < 5 is device x < 10 at 2x.
    assert!(pixel(&mut surface, 8, 10).g() > 240);
    assert!(pixel(&mut surface, 11, 10).r() > 240);
    // The crop keeps the output inside the node box (device 20x20 here).
    assert!(pixel(&mut surface, 19, 19).r() > 240);
}

#[test]
fn nested_passes_restore_in_order() {
    let mut backend = NativeBackend::with_dpi(1.0);
    let mut surface = surface(skia_safe::Color::TRANSPARENT);
    let base = surface.canvas().save_count();
    let identity = "uniform shader object; half4 main(float2 p) { return object.eval(p); }";
    backend.push_shader_effect_layer(surface.canvas(), node_box(), SPLIT, &[], false);
    backend.push_shader_effect_layer(surface.canvas(), node_box(), identity, &[], false);
    surface.canvas().save();
    fill(surface.canvas(), skia_safe::Color::RED);
    backend.restore(surface.canvas());
    backend.restore(surface.canvas());
    backend.restore(surface.canvas());
    assert_eq!(surface.canvas().save_count(), base);
    // No `size` uniform: document px = paint px, so x < 5 turns green.
    assert!(pixel(&mut surface, 2, 10).g() > 240);
    assert!(pixel(&mut surface, 15, 10).r() > 240);
}
