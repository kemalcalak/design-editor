//! Greedy line wrap for canvas text nodes with an explicit width, measured
//! in the node's own face so the breaks match the glyphs the painter draws
//! (`canvas_viewport_text`), the inline editor and the caret geometry
//! (`canvas_text_edit`).

/// The face a text node's lines are measured in: the painter's family
/// (with its `system-ui` default), weight and slant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TextFace<'a> {
    pub(crate) family: &'a str,
    pub(crate) weight: u16,
    pub(crate) italic: bool,
}

impl TextFace<'_> {
    /// Advance of `text` at `font_size` in this face.
    pub(crate) fn measure(
        &self,
        backend: &mut dyn crate::RenderBackend,
        text: &str,
        font_size: f32,
    ) -> f32 {
        backend.measure_text_family_styled(text, font_size, self.family, self.weight, self.italic)
    }
}

/// Width a text node's lines break at: its box width exactly, or `None`
/// for auto-width text, which never wraps. The one wrap decision for
/// painting, inline editing and caret geometry (all through
/// `canvas_text_edit::text_edit_layout`). Layout sizes `fit_content` heights
/// at the same width with the same wrap (`de-pen-loader`'s layout measure),
/// and pen.dev, Figma and browsers break at the exact width too. The old
/// `min(5%, fontSize / 2)` slack from the retired TS renderer kept words on
/// lines they did not fit (#186).
pub(crate) fn wrap_width(node: &crate::layout_scene::SceneNode) -> Option<f32> {
    node.text_wrap.then_some(node.bounds.size.x)
}

/// Greedy line-wrap for canvas text nodes with explicit width, measured
/// in `face` with `letter_spacing` between glyphs. The algorithm is the
/// shared `de_editor_core::text_wrap::wrap_text_measured`, which the
/// layout measure (`de-pen-loader`) runs too, so the box height layout
/// computes holds exactly the lines painted here (#186).
pub(crate) fn wrap_text(
    backend: &mut dyn crate::RenderBackend,
    text: &str,
    font_size: f32,
    max_w: f32,
    face: TextFace<'_>,
    letter_spacing: f32,
) -> Vec<String> {
    de_editor_core::text_wrap::wrap_text_measured(text, max_w, &mut |line: &str| {
        measure_text_with_letter_spacing(backend, line, font_size, face, letter_spacing)
    })
}

fn measure_text_with_letter_spacing(
    backend: &mut dyn crate::RenderBackend,
    text: &str,
    font_size: f32,
    face: TextFace<'_>,
    letter_spacing: f32,
) -> f32 {
    let base = face.measure(backend, text, font_size);
    let gaps = text.chars().count().saturating_sub(1) as f32;
    (base + gaps * letter_spacing).max(0.0)
}

#[cfg(test)]
mod wrap_tests {
    use super::*;
    use crate::{Color, Point2D, Rect, RenderBackend, TextLayout};

    fn face(weight: u16) -> TextFace<'static> {
        TextFace {
            family: "system-ui",
            weight,
            italic: false,
        }
    }

    /// Backend that measures every char as 10 px wide regardless of
    /// glyph — keeps the test arithmetic exact.
    struct UniformBackend;
    impl RenderBackend for UniformBackend {
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
        fn stroke_svg_path(&mut self, _: &str, _: Point2D, _: f32, _: Color, _: f32) {}
        fn resize(&mut self, _: u32, _: u32) {}
        fn fill_oval(&mut self, _: Rect, _: Color) {}
        fn stroke_oval(&mut self, _: Rect, _: Color, _: f32) {}
        fn fill_polygon(&mut self, _: &[Point2D], _: Color) {}
        fn stroke_polygon(&mut self, _: &[Point2D], _: Color, _: f32) {}
        fn rotate(&mut self, _: f32, _: Point2D) {}
        fn fill_svg_path(&mut self, _: &str, _: Point2D, _: f32, _: f32, _: Color) {}
        fn measure_text(&mut self, text: &str, _: f32) -> f32 {
            text.chars().count() as f32 * 10.0
        }
        fn measure_text_weighted(&mut self, text: &str, _: f32, _: u16) -> f32 {
            text.chars().count() as f32 * 10.0
        }
        fn dpi_scale(&self) -> f32 {
            1.0
        }
    }

    #[test]
    fn ascii_word_breaks_at_word_boundary() {
        let mut b = UniformBackend;
        // "hello world foo" = 15 chars × 10 = 150 px. Budget 100.
        // Greedy walk: "hello " (60) fits, "hello world" (110)
        // exceeds — flush "hello ", start with "world". Then
        // "world " (60), "world foo" (90) fits.
        let lines = wrap_text(&mut b, "hello world foo", 13.0, 100.0, face(400), 0.0);
        assert_eq!(lines, vec!["hello ", "world foo"]);
    }

    #[test]
    fn cjk_breaks_per_character() {
        let mut b = UniformBackend;
        // 8 CJK chars × 10 px = 80. Budget 50 forces a break every
        // 5 chars. Pure split_whitespace would treat the whole
        // string as one word and return a single 80-px line.
        let lines = wrap_text(&mut b, "中文测试段落很长哦", 13.0, 50.0, face(400), 0.0);
        assert!(lines.len() >= 2, "got {:?}", lines);
        for line in &lines {
            assert!(line.chars().count() <= 5, "line too long: {:?}", line);
        }
    }

    #[test]
    fn explicit_newlines_become_line_breaks() {
        let mut b = UniformBackend;
        // "ab\ncd\nef" — each segment fits the 100 px budget, but
        // we still split on `\n`. Pure wrap (no newline handling)
        // would join everything as "abcdef" and return one line.
        let lines = wrap_text(&mut b, "ab\ncd\nef", 13.0, 100.0, face(400), 0.0);
        assert_eq!(lines, vec!["ab", "cd", "ef"]);
    }

    #[test]
    fn blank_lines_survive_in_output() {
        let mut b = UniformBackend;
        // "para1\n\npara2" — the empty segment between the two
        // paragraphs must paint as a blank line so authored gaps
        // survive.
        let lines = wrap_text(&mut b, "para1\n\npara2", 13.0, 200.0, face(400), 0.0);
        assert_eq!(lines, vec!["para1", "", "para2"]);
    }

    #[test]
    fn newlines_split_before_wrap_runs() {
        let mut b = UniformBackend;
        // "hello world\nfoo bar baz" with budget 100. First segment
        // "hello world" = 110 → wraps to ["hello ", "world"].
        // Second segment "foo bar baz" = 110 → ["foo bar ", "baz"].
        let lines = wrap_text(
            &mut b,
            "hello world\nfoo bar baz",
            13.0,
            100.0,
            face(400),
            0.0,
        );
        assert_eq!(lines, vec!["hello ", "world", "foo bar ", "baz"]);
    }

    #[test]
    fn cjk_latin_mix_breaks_within_cjk_run() {
        let mut b = UniformBackend;
        let lines = wrap_text(&mut b, "hello 中文段落", 13.0, 60.0, face(400), 0.0);
        assert!(lines.len() >= 2, "got {:?}", lines);
        assert!(lines[0].starts_with("hello"));
    }

    /// Stub backend whose advance widths depend on the weight —
    /// 10 px per char at weight 400, 14 px per char at weight 700.
    /// Mirrors the real native backend behaviour where bold glyphs
    /// have wider advances; if wrap_text ignored weight, the same
    /// string would break at a different line index.
    struct WeightedBackend;
    impl RenderBackend for WeightedBackend {
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
        fn stroke_svg_path(&mut self, _: &str, _: Point2D, _: f32, _: Color, _: f32) {}
        fn resize(&mut self, _: u32, _: u32) {}
        fn fill_oval(&mut self, _: Rect, _: Color) {}
        fn stroke_oval(&mut self, _: Rect, _: Color, _: f32) {}
        fn fill_polygon(&mut self, _: &[Point2D], _: Color) {}
        fn stroke_polygon(&mut self, _: &[Point2D], _: Color, _: f32) {}
        fn rotate(&mut self, _: f32, _: Point2D) {}
        fn fill_svg_path(&mut self, _: &str, _: Point2D, _: f32, _: f32, _: Color) {}
        fn measure_text(&mut self, text: &str, _: f32) -> f32 {
            text.chars().count() as f32 * 10.0
        }
        fn measure_text_weighted(&mut self, text: &str, _: f32, weight: u16) -> f32 {
            let per_char = if weight >= 600 { 14.0 } else { 10.0 };
            text.chars().count() as f32 * per_char
        }
        fn dpi_scale(&self) -> f32 {
            1.0
        }
    }

    #[test]
    fn wrap_uses_weighted_advances() {
        // "hello world" at weight 700 = 11 chars × 14 = 154 px.
        // Budget 120 forces a break ("hello " = 6×14=84 fits, +5×14=70 doesn't).
        // At weight 400 (110 px) it'd fit on one line — codex's
        // BLOCK: wrap was measuring at 400 while paint used 700.
        let mut b = WeightedBackend;
        let bold = wrap_text(&mut b, "hello world", 13.0, 120.0, face(700), 0.0);
        let regular = wrap_text(&mut b, "hello world", 13.0, 120.0, face(400), 0.0);
        assert_eq!(regular, vec!["hello world"], "weight 400 fits");
        assert!(bold.len() >= 2, "weight 700 must wrap: {:?}", bold);
    }

    #[test]
    fn wrap_includes_negative_letter_spacing() {
        let mut b = UniformBackend;
        // Base width is 110 px; ten -1 px gaps bring it to exactly 100 px.
        let lines = wrap_text(&mut b, "hello world", 13.0, 100.0, face(400), -1.0);
        assert_eq!(lines, vec!["hello world"]);
    }
}
