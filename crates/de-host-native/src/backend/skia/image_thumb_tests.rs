use super::*;

fn test_jpeg(width: i32, height: i32) -> Vec<u8> {
    colored_jpeg(width, height, skia_safe::Color::RED)
}

fn colored_jpeg(width: i32, height: i32, color: skia_safe::Color) -> Vec<u8> {
    let mut surface =
        skia_safe::surfaces::raster_n32_premul((width, height)).expect("raster surface");
    surface.canvas().clear(color);
    surface
        .image_snapshot()
        .encode(None, skia_safe::EncodedImageFormat::JPEG, 60)
        .expect("JPEG encode")
        .as_bytes()
        .to_vec()
}

#[test]
fn thumbnail_draw_decodes_once_into_a_dedicated_cache() {
    let mut backend = NativeBackend::with_dpi(1.0);
    let mut surface = skia_safe::surfaces::raster_n32_premul((80, 40)).unwrap();
    let jpeg = test_jpeg(32, 16);

    backend.draw_image_thumb(surface.canvas(), Rect::xywh(0.0, 0.0, 80.0, 40.0), 7, &jpeg);

    assert_eq!(
        backend.image_cache_len(),
        0,
        "full raster cache stays untouched"
    );
    assert_eq!(backend.thumb_cache_len(), 1);
    assert_eq!(backend.thumb_raster_cache_len(), 1);

    backend.draw_image_thumb(surface.canvas(), Rect::xywh(0.0, 0.0, 80.0, 40.0), 7, &jpeg);
    assert_eq!(backend.thumb_cache_len(), 1);
    assert_eq!(backend.thumb_raster_cache_len(), 1);
}

/// Two documents may give one image source (one paint id) different
/// thumbnails (#469): a cached decode is reused only for the same bytes.
#[test]
fn the_same_id_with_other_bytes_decodes_those_bytes() {
    let mut backend = NativeBackend::with_dpi(1.0);
    let mut surface = skia_safe::surfaces::raster_n32_premul((20, 20)).unwrap();
    let rect = Rect::xywh(0.0, 0.0, 20.0, 20.0);
    let centre = |surface: &mut skia_safe::Surface| {
        let mut pixel = [0u8; 4];
        let info = skia_safe::ImageInfo::new(
            (1, 1),
            skia_safe::ColorType::RGBA8888,
            skia_safe::AlphaType::Unpremul,
            None,
        );
        assert!(surface.read_pixels(&info, &mut pixel, 4, (10, 10)));
        pixel
    };

    backend.draw_image_thumb(
        surface.canvas(),
        rect,
        7,
        &colored_jpeg(8, 8, skia_safe::Color::RED),
    );
    let red = centre(&mut surface);
    assert!(red[0] > 180 && red[2] < 80, "{red:?}");
    backend.draw_image_thumb(
        surface.canvas(),
        rect,
        7,
        &colored_jpeg(8, 8, skia_safe::Color::BLUE),
    );
    let blue = centre(&mut surface);
    assert!(blue[2] > 180 && blue[0] < 80, "{blue:?}");
    assert_eq!(
        backend.thumb_cache_len(),
        1,
        "the entry is replaced, not added"
    );
}

#[test]
fn corrupt_thumbnail_is_negatively_cached_and_never_panics() {
    let mut backend = NativeBackend::with_dpi(1.0);
    let mut surface = skia_safe::surfaces::raster_n32_premul((20, 20)).unwrap();

    backend.draw_image_thumb(
        surface.canvas(),
        Rect::xywh(0.0, 0.0, 20.0, 20.0),
        9,
        b"not an image",
    );
    backend.draw_image_thumb(
        surface.canvas(),
        Rect::xywh(0.0, 0.0, 20.0, 20.0),
        9,
        b"not an image",
    );

    assert_eq!(backend.thumb_cache_len(), 1);
    assert_eq!(backend.thumb_raster_cache_len(), 0);
}

#[test]
fn oversized_thumbnail_dimensions_are_rejected_and_negatively_cached() {
    let mut backend = NativeBackend::with_dpi(1.0);
    let mut surface = skia_safe::surfaces::raster_n32_premul((20, 20)).unwrap();
    let oversized = test_jpeg(64, 64);
    assert!(
        oversized.len() <= 4 * 1024,
        "the regression payload must pass the encoded-byte guard"
    );

    backend.draw_image_thumb(
        surface.canvas(),
        Rect::xywh(0.0, 0.0, 20.0, 20.0),
        10,
        &oversized,
    );
    backend.draw_image_thumb(
        surface.canvas(),
        Rect::xywh(0.0, 0.0, 20.0, 20.0),
        10,
        &oversized,
    );

    assert_eq!(backend.thumb_cache_len(), 1);
    assert_eq!(
        backend.thumb_raster_cache_len(),
        0,
        "a tiny encoded payload must not expand beyond the 32 px paint bound"
    );
}

#[test]
fn paint_diagnostics_distinguish_blur_up_from_sharp_rasters() {
    let mut backend = NativeBackend::with_dpi(1.0);
    let mut surface = skia_safe::surfaces::raster_n32_premul((80, 40)).unwrap();
    let jpeg = test_jpeg(32, 16);

    begin_image_paint_diagnostics();
    backend.draw_image_thumb(
        surface.canvas(),
        Rect::xywh(0.0, 0.0, 80.0, 40.0),
        17,
        &jpeg,
    );
    let blur = image_paint_diagnostics_snapshot();
    assert_eq!(blur.successful_thumbnail_draws, 1);
    assert_eq!(blur.sharp_raster_hits, 0);
    assert_eq!(blur.paint_thread_full_decodes, 0);

    let sharp = decode_raster(&jpeg).expect("valid JPEG rasterizes");
    backend.install_raster_image(17, sharp, u32::MAX);
    backend.draw_image(
        surface.canvas(),
        Rect::xywh(0.0, 0.0, 80.0, 40.0),
        17,
        &jpeg,
    );
    let sharp = end_image_paint_diagnostics();
    assert_eq!(sharp.successful_thumbnail_draws, 1);
    assert_eq!(sharp.sharp_raster_hits, 1);
    assert_eq!(sharp.paint_thread_full_decodes, 1);
}
