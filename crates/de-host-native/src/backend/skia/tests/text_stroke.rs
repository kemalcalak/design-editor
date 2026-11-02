//! Outlined text (#282): `stroke_text` strokes the glyph contours that
//! `draw_text` fills, in the given colour, honouring the set dash.

use super::*;
use de_editor_ui::layout_scene::{StrokeDash, StrokeStyle};
use de_editor_ui::TextLayout;

const SIZE: f32 = 96.0;

/// Paint "O" on white through `paint`, then count the ink: pixels that are
/// clearly not white, and how many of those are dark (the run's black ink
/// rather than a red outline or its anti-aliased pink edge).
fn ink(paint: impl FnOnce(&mut NativeBackend, &skia_safe::Canvas, &TextLayout)) -> (usize, usize) {
    let _guard = crate::font_registry_test_support::lock();
    let mut be = NativeBackend::with_dpi(1.0);
    let (w, h) = (160, 140);
    let mut surface = skia_safe::surfaces::raster_n32_premul((w, h)).unwrap();
    surface.canvas().clear(skia_safe::Color::WHITE);
    let layout = TextLayout::single_run("O", "", SIZE, Color::BLACK.to_jian(), Point2D::ZERO);
    paint(&mut be, surface.canvas(), &layout);
    let image = surface.image_snapshot();
    let pixels = image.peek_pixels().expect("peek raster pixels");
    let (mut inked, mut dark) = (0, 0);
    for y in 0..h {
        for x in 0..w {
            let c = pixels.get_color((x, y));
            if c.r().min(c.g()).min(c.b()) < 200 {
                inked += 1;
                if c.r() < 120 {
                    dark += 1;
                }
            }
        }
    }
    (inked, dark)
}

const RED: Color = Color {
    r: 1.0,
    g: 0.0,
    b: 0.0,
    a: 1.0,
};

#[test]
fn stroke_text_outlines_the_glyphs_in_its_colour() {
    let (filled, filled_dark) = ink(|be, canvas, layout| {
        be.draw_text(canvas, layout, Point2D::new(20.0, 110.0));
    });
    let (outlined, dark) = ink(|be, canvas, layout| {
        be.stroke_text(canvas, layout, Point2D::new(20.0, 110.0), RED, 2.0);
    });
    assert!(filled_dark > 0, "the fill paints in the run's black");
    assert!(outlined > 0, "the outline paints");
    assert_eq!(dark, 0, "the outline is red, never the run's black");
    assert!(
        outlined < filled,
        "a thin outline covers less than the fill ({outlined} vs {filled})"
    );
}

#[test]
fn stroke_text_follows_the_set_dash() {
    let (solid, _) = ink(|be, canvas, layout| {
        be.stroke_text(canvas, layout, Point2D::new(20.0, 110.0), RED, 4.0);
    });
    let (dashed, _) = ink(|be, canvas, layout| {
        be.set_stroke_style(StrokeStyle {
            dash: StrokeDash::new(&[4.0, 8.0], 0.0),
            ..StrokeStyle::default()
        });
        be.stroke_text(canvas, layout, Point2D::new(20.0, 110.0), RED, 4.0);
    });
    assert!(
        dashed > 0 && dashed < solid,
        "dashes leave gaps ({dashed} vs {solid})"
    );
}

#[test]
fn a_zero_width_stroke_draws_nothing() {
    let (outlined, _) = ink(|be, canvas, layout| {
        be.stroke_text(canvas, layout, Point2D::new(20.0, 110.0), RED, 0.0);
    });
    assert_eq!(outlined, 0);
}
