//! Single-line input geometry from the widths paint measured (#451).
//!
//! `text_input::single_line_byte_offset_at` resolves a press before a host
//! holds a [`SingleLinePaintGeometry`] snapshot, and measures with a
//! backend that only estimates. The gallery panels (Prompt Center, Asset
//! Center) keep no snapshot of their own, so their press-to-caret and IME
//! caret went through that estimate on every press. Here the same geometry
//! runs over [`LabelWidths`]: paint measures the field's text through it
//! ([`measure_single_line`], in the face jian's `TextInputView` draws it in),
//! and the backend-less press and caret paths read the kept widths, so the
//! caret lands on the glyph that was painted.
//!
//! What is kept is what the geometry measures: each character of the field
//! and the prefix up to the caret, one width each, keyed like any other run.

use jian_core::text_input::TextInputState;

use crate::widgets::label_widths::LabelWidths;
use crate::widgets::text_input::{SingleLinePaintGeometry, TEXT_INPUT_FAMILY};
use crate::{Color, Point2D, Rect, RenderBackend, TextLayout};

/// Measure and keep every width [`SingleLinePaintGeometry::measure`] asks for
/// `state` — the prefix to the caret, the composition's prefix and each
/// character — through `backend`, so [`measured_byte_offset_at`] and
/// [`measured_caret_rect`] read what paint drew. `paint_text_input_view`
/// calls it for every field it draws; a repaint is lookups only, and it
/// builds no geometry, so it allocates nothing.
pub(crate) fn measure_single_line(
    backend: &mut dyn RenderBackend,
    state: &TextInputState,
    font_size: f32,
) {
    let mut widths = LabelWidths::measuring(backend);
    let mut keep = |text: &str| {
        widths.styled(text, font_size, TEXT_INPUT_FAMILY, 400);
    };
    let text = state.text();
    keep(&text[..jian_core::text_input::prev_char_boundary(text, state.caret())]);
    if let Some(composition) = state.composition().filter(|value| !value.text.is_empty()) {
        let cursor =
            jian_core::text_input::prev_char_boundary(&composition.text, composition.cursor);
        keep(&composition.text[..cursor]);
    }
    let mut buf = [0; 4];
    for ch in text.chars() {
        keep(ch.encode_utf8(&mut buf));
    }
}

/// The byte offset a press at `point` puts the caret at, from the kept
/// widths (the estimate only for text no paint has measured yet).
pub(crate) fn measured_byte_offset_at(
    state: &TextInputState,
    rect: Rect,
    point: Point2D,
    font_size: f32,
    pad_x: f32,
) -> Option<usize> {
    if !rect.contains(point) {
        return None;
    }
    geometry(state, rect, font_size, pad_x).byte_offset_at(state, point, false)
}

/// The caret rect the IME candidate window anchors to, from the kept widths.
pub(crate) fn measured_caret_rect(
    state: &TextInputState,
    rect: Rect,
    font_size: f32,
    pad_x: f32,
) -> Rect {
    let geometry = geometry(state, rect, font_size, pad_x);
    geometry
        .caret_rect(state)
        .unwrap_or_else(|| Rect::xywh(rect.origin.x + pad_x, rect.origin.y, 1.5, rect.size.y))
}

fn geometry(
    state: &TextInputState,
    rect: Rect,
    font_size: f32,
    pad_x: f32,
) -> SingleLinePaintGeometry {
    let mut widths = LabelWidths::cached();
    let mut adapter = WidthsBackend {
        widths: &mut widths,
    };
    SingleLinePaintGeometry::measure(state, rect, font_size, pad_x, &mut adapter)
}

/// A measure-only backend whose family-aware measurements go through
/// [`LabelWidths`]: measuring when it wraps a measuring pass, cached
/// otherwise. It draws nothing.
struct WidthsBackend<'w, 'b> {
    widths: &'w mut LabelWidths<'b>,
}

impl RenderBackend for WidthsBackend<'_, '_> {
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
        1.0
    }

    fn measure_text_family(&mut self, text: &str, font_size: f32, family: &str) -> f32 {
        self.widths.styled(text, font_size, family, 400)
    }

    fn measure_text_family_styled(
        &mut self,
        text: &str,
        font_size: f32,
        family: &str,
        weight: u16,
        _italic: bool,
    ) -> f32 {
        self.widths.styled(text, font_size, family, weight)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::label_fit_test_support::fresh_backend;

    /// After paint measured the field, a press resolves against the painted
    /// glyphs: the offset under each glyph's right half is the next one.
    #[test]
    fn a_press_reads_the_widths_paint_measured() {
        let mut backend = fresh_backend();
        let mut state = TextInputState::default();
        state.set_text("Wide Wonder");
        let rect = Rect::xywh(0.0, 0.0, 400.0, 30.0);
        measure_single_line(&mut backend, &state, 13.0);
        let painted =
            SingleLinePaintGeometry::measure(&state, rect, 13.0, 8.0, &mut fresh_backend_keep());
        for x in [9.0, 40.0, 90.0, 140.0] {
            let point = Point2D::new(x, 15.0);
            assert_eq!(
                measured_byte_offset_at(&state, rect, point, 13.0, 8.0),
                painted.byte_offset_at(&state, point, false),
                "at {x}"
            );
        }
        assert_eq!(
            Some(measured_caret_rect(&state, rect, 13.0, 8.0)),
            painted.caret_rect(&state)
        );
    }

    /// The same metrics as `fresh_backend` without resetting the cache.
    fn fresh_backend_keep() -> crate::widgets::test_family_gap_backend::FamilyGapBackend {
        crate::widgets::test_family_gap_backend::FamilyGapBackend::default()
    }
}
