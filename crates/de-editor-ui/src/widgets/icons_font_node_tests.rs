use super::*;
use crate::{Rect, TextLayout};

/// Records the stroke widths and fill viewBoxes the icon painter asks for.
#[derive(Default)]
struct RecordingBackend {
    strokes: Vec<f32>,
    fills: Vec<f32>,
}

impl RenderBackend for RecordingBackend {
    fn begin_frame(&mut self) {}
    fn end_frame(&mut self) {}
    fn fill_rect(&mut self, _: Rect, _: Color) {}
    fn stroke_rect(&mut self, _: Rect, _: Color, _: f32) {}
    fn draw_text(&mut self, _: &TextLayout, _: Point2D) {}
    fn clip_rect(&mut self, _: Rect) {}
    fn save(&mut self) {}
    fn restore(&mut self) {}
    fn translate(&mut self, _: Point2D) {}
    fn stroke_line(&mut self, _: Point2D, _: Point2D, _: Color, _: f32) {}
    fn fill_round_rect(&mut self, _: Rect, _: f32, _: Color) {}
    fn stroke_round_rect(&mut self, _: Rect, _: f32, _: Color, _: f32) {}
    fn stroke_svg_path(&mut self, _: &str, _: Point2D, _: f32, _: Color, width: f32) {
        self.strokes.push(width);
    }
    fn fill_svg_path(&mut self, _: &str, _: Point2D, _: f32, viewbox: f32, _: Color) {
        self.fills.push(viewbox);
    }
    fn resize(&mut self, _: u32, _: u32) {}
    fn dpi_scale(&self) -> f32 {
        1.0
    }
}

fn paint(family: &str, name: &str, weight: u16, size: f32) -> RecordingBackend {
    let mut backend = RecordingBackend::default();
    paint_icon_font_node_weighted(
        &mut backend,
        family,
        name,
        weight,
        Rect::xywh(0.0, 0.0, size, size),
        Some(Color::WHITE),
    );
    backend
}

#[test]
fn material_symbols_paint_as_a_filled_24_unit_glyph() {
    let b = paint("material-symbols-rounded", "home", 0, 24.0);
    assert_eq!(b.fills, [24.0]);
    assert!(b.strokes.is_empty());
}

#[test]
fn a_heavy_material_weight_adds_a_scaled_outline() {
    // 700 = one 24-unit outline; at 48 px that is 2 px.
    let b = paint("Material Symbols Outlined", "home", 700, 48.0);
    assert_eq!(b.fills.len(), 1);
    assert_eq!(b.strokes, [2.0]);
}

#[test]
fn a_light_material_weight_paints_the_master_only() {
    let b = paint("material-symbols-sharp", "home", 200, 24.0);
    assert_eq!(b.fills.len(), 1);
    assert!(b.strokes.is_empty());
}

#[test]
fn phosphor_paints_its_256_unit_glyph_at_any_weight() {
    for weight in [0, 100, 300, 700] {
        let b = paint("phosphor", "house", weight, 24.0);
        assert_eq!(b.fills, [256.0], "weight {weight}");
        assert!(b.strokes.is_empty());
    }
}

#[test]
fn feather_ignores_weight_and_keeps_its_2px_stroke() {
    let b = paint("feather", "home", 700, 24.0);
    assert!(b.fills.is_empty());
    assert!(!b.strokes.is_empty());
    assert!(b.strokes.iter().all(|w| *w == 2.0));
}

#[test]
fn the_unweighted_entry_point_matches_weight_zero() {
    let mut backend = RecordingBackend::default();
    paint_icon_font_node(
        &mut backend,
        "phosphor",
        "house",
        Rect::xywh(0.0, 0.0, 24.0, 24.0),
        None,
    );
    assert_eq!(backend.fills, paint("phosphor", "house", 0, 24.0).fills);
}
