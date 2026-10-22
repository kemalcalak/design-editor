//! The Design Editor mark (issue #49), painted from the path data in
//! `assets/brand/design-editor-symbol.svg`: a solid "D" whose stem is the
//! person, whose curve and anchor dot are the agent, with the agent's
//! `>_` prompt inside. Brand colours are fixed; they read on both themes.

use crate::{Color, Point2D, RenderBackend};

/// Width of the symbol's artwork in source units (the curve's rightmost
/// point). The artwork is 560 tall and is centred vertically in the box.
const ART_W: f32 = 580.0;
const ART_H: f32 = 560.0;
/// Stroke width of the `>_` prompt in source units.
const PROMPT_STROKE: f32 = 38.0;

const PERSON: Color = Color::rgba_u8(0x3E, 0x6D, 0xF5, 1.0);
const AGENT: Color = Color::rgba_u8(0x8F, 0xA8, 0xFA, 1.0);

/// Filled shapes in source units, with their colour.
const FILLS: [(&str, Color); 5] = [
    (
        "M18,0 H302 V120 H120 V440 H120 V560 H18 Q0,560 0,542 V18 Q0,0 18,0 Z",
        PERSON,
    ),
    (
        "M300.0,0.0 A280,280 0 0 1 404.9,20.4 L359.9,131.7 A160,160 0 0 0 300.0,120.0 Z",
        PERSON,
    ),
    (
        "M514.5,100.0 A280,280 0 0 1 300.0,560.0 L300.0,440.0 A160,160 0 0 0 422.6,177.2 Z",
        AGENT,
    ),
    ("M116,440 H302 V560 H116 Z", AGENT),
    (
        "M394.5,138.6 A74,74 0 1 0 542.5,138.6 A74,74 0 1 0 394.5,138.6 Z",
        AGENT,
    ),
];

/// The `>_` prompt, stroked with round caps and joins.
const PROMPT: [&str; 2] = ["M195,215 L262,280 L195,345", "M300,345 H374"];

/// Paint the mark fitted into a `size` × `size` box at `top_left`.
pub fn paint_design_editor_mark(backend: &mut dyn RenderBackend, top_left: Point2D, size: f32) {
    let scale = size / ART_W;
    let origin = Point2D::new(top_left.x, top_left.y + (size - ART_H * scale) / 2.0);
    for (d, color) in FILLS {
        backend.fill_svg_path(d, origin, size, ART_W, color);
    }
    // `stroke_svg_path` maps its path through `size / 24` (the lucide grid).
    for d in PROMPT {
        backend.stroke_svg_path(d, origin, 24.0 * scale, AGENT, PROMPT_STROKE * scale);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Rect, TextLayout};

    #[derive(Default)]
    struct Capture {
        fills: Vec<(Point2D, f32, f32)>,
        strokes: Vec<(f32, f32)>,
    }

    impl RenderBackend for Capture {
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
        fn stroke_svg_path(&mut self, _: &str, _: Point2D, size: f32, _: Color, width: f32) {
            self.strokes.push((size, width));
        }
        fn fill_svg_path(&mut self, _: &str, at: Point2D, size: f32, viewbox: f32, _: Color) {
            self.fills.push((at, size, viewbox));
        }
        fn resize(&mut self, _: u32, _: u32) {}
        fn dpi_scale(&self) -> f32 {
            1.0
        }
    }

    #[test]
    fn mark_paints_five_shapes_and_the_prompt_inside_its_box() {
        let mut backend = Capture::default();
        paint_design_editor_mark(&mut backend, Point2D::new(22.0, 40.0), 20.0);
        assert_eq!(backend.fills.len(), 5);
        assert_eq!(backend.strokes.len(), 2);
        for (at, size, viewbox) in &backend.fills {
            // Every shape shares one origin and one scale, so the parts line up.
            assert_eq!(*at, backend.fills[0].0);
            assert!((size / viewbox - 20.0 / ART_W).abs() < 1e-6);
            assert!(at.x >= 22.0 && at.y >= 40.0);
            assert!(at.y + ART_H * size / viewbox <= 60.0 + 1e-3);
        }
        for (size, width) in &backend.strokes {
            assert!((size / 24.0 - 20.0 / ART_W).abs() < 1e-6);
            assert!(*width > 1.0 && *width < 2.0);
        }
    }
}
