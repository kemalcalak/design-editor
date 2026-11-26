//! Test-only rasterizer: fills a path into an offscreen raster and
//! counts covered pixels, so the path operations are checked by the
//! pixels they paint, not only by their geometry.

use skia_safe::{surfaces, Color, Paint, Path};

/// Pixels (alpha ≥ 50%) `path` covers in a `w × h` raster at 1×.
pub fn coverage(path: &Path, w: i32, h: i32) -> usize {
    let mut surface = surfaces::raster_n32_premul((w, h)).expect("raster surface");
    let canvas = surface.canvas();
    canvas.clear(Color::TRANSPARENT);
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_color(Color::BLACK);
    canvas.draw_path(path, &paint);
    let info = surface.image_info();
    let row = info.min_row_bytes();
    let mut pixels = vec![0u8; row * h as usize];
    assert!(surface.read_pixels(&info, &mut pixels, row, (0, 0)));
    pixels.chunks(4).filter(|px| px[3] >= 128).count()
}

/// Whether the pixel at `(x, y)` is covered.
pub fn covered(path: &Path, x: f32, y: f32) -> bool {
    path.contains((x, y))
}
