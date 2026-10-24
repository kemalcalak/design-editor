//! Layout measure that wraps text the way the canvas painter does (#186).
//!
//! jian's `SkiaMeasure` breaks lines with Skia's paragraph shaper, but the
//! canvas paints (and the inline editor places carets) with the greedy wrap
//! in `de_editor_core::text_wrap` over jian-skia's `FontResolver` advances:
//! shaped word by word (#197), plus the synthetic-bold width allowance for
//! weights the face does not carry. The paragraph shaper breaks and measures
//! a little differently, so a `fit_content` box could be sized for one line
//! while the canvas painted two. For text in one style and a simple script
//! this backend counts the lines with the painter's wrap and measure;
//! everything else (mixed styles, scripts that need shaping, the natural
//! extent) keeps `SkiaMeasure`'s answer. Both go through one resolver, the
//! one `SkiaMeasure` measures natural widths with.

use de_editor_core::text_defaults;
use jian_core::layout::measure::{FontStyleKind, MeasureBackend, MeasureRequest, MeasureResult};

/// Bundled last-resort face of the native painter (`NativeBackend`), so a
/// family nothing resolves measures in the face the canvas paints.
const ROBOTO_TTF: &[u8] = include_bytes!("../../de-host-native/assets/Roboto-Regular.ttf");

pub(crate) struct PainterWrapMeasure {
    paragraph: jian_skia::SkiaMeasure,
}

impl PainterWrapMeasure {
    pub(crate) fn new() -> Self {
        let paragraph = jian_skia::with_font_lock(|| {
            let font_mgr = skia_safe::FontMgr::new();
            let default_typeface = font_mgr.new_from_data(ROBOTO_TTF, None);
            jian_skia::SkiaMeasure::with_font_resolver(
                jian_skia::FontResolver::with_default_typeface(font_mgr, default_typeface),
            )
        });
        Self { paragraph }
    }
}

/// The one style a request's runs share, with their joined text.
struct UniformRun<'a> {
    text: std::borrow::Cow<'a, str>,
    family: &'a str,
    size: f32,
    weight: u16,
    italic: bool,
    letter_spacing: f32,
}

/// `None` when the runs differ in anything that changes advances.
fn uniform_run<'a>(req: &MeasureRequest<'a>) -> Option<UniformRun<'a>> {
    let first = req.runs.first()?;
    let same = req.runs.iter().all(|run| {
        run.font_family == first.font_family
            && run.font_size == first.font_size
            && run.font_weight == first.font_weight
            && run.font_style == first.font_style
            && run.letter_spacing == first.letter_spacing
    });
    if !same {
        return None;
    }
    let text = if req.runs.len() == 1 {
        std::borrow::Cow::Borrowed(first.text)
    } else {
        std::borrow::Cow::Owned(req.runs.iter().map(|run| run.text).collect())
    };
    Some(UniformRun {
        text,
        family: first
            .font_family
            .filter(|family| !family.trim().is_empty())
            .unwrap_or(text_defaults::FONT_FAMILY),
        size: first.font_size,
        weight: if first.font_weight > 0 {
            first.font_weight
        } else {
            text_defaults::FONT_WEIGHT
        },
        italic: first.font_style == FontStyleKind::Italic,
        letter_spacing: if first.letter_spacing.is_finite() {
            first.letter_spacing
        } else {
            0.0
        },
    })
}

impl MeasureBackend for PainterWrapMeasure {
    fn measure(&self, req: &MeasureRequest<'_>) -> MeasureResult {
        let shaped = self.paragraph.measure(req);
        let Some(max_width) = req.max_width else {
            return shaped;
        };
        let Some(run) = uniform_run(req) else {
            return shaped;
        };
        if run.size <= 0.0 || de_editor_core::text_script::needs_complex_shaping(&run.text) {
            return shaped;
        }
        let mut measure = |line: &str| {
            let gaps = line.chars().count().saturating_sub(1) as f32;
            let base = self.paragraph.font_resolver().measure_text(
                line,
                run.size,
                Some(run.family),
                run.weight,
                run.italic,
            );
            (base + gaps * run.letter_spacing).max(0.0)
        };
        // One font-lock hold for every probe (the lock is reentrant).
        let (lines, width) = jian_skia::with_font_lock(|| {
            let lines =
                de_editor_core::text_wrap::wrap_text_measured(&run.text, max_width, &mut measure);
            let width = lines
                .iter()
                .map(|line| measure(line))
                .fold(0.0_f32, f32::max);
            (lines, width)
        });
        let line_count = u16::try_from(lines.len()).unwrap_or(u16::MAX);
        let height = if req.line_height > 0.0 || line_count > 1 {
            // Authored line height, or more than one line: the painter
            // advances every line by the line height (`SkiaMeasure` does the
            // same for literal newlines), so the box holds exactly the
            // painted lines.
            f32::from(line_count) * run.size * effective_line_height(req.line_height)
        } else {
            // One line at the default line height keeps the font's own
            // line box, as `SkiaMeasure` does.
            shaped.height / f32::from(shaped.line_count.max(1))
        };
        MeasureResult {
            // Rounded up so a box sized from this width (taffy rounds rects
            // to whole px) never ends up narrower than its widest line. A
            // word wider than the line overflows when painted; layout keeps
            // the budget as the shaper does.
            width: width.ceil().min(max_width),
            height,
            line_count,
            baseline: shaped.baseline,
        }
    }

    fn font_generation(&self) -> u64 {
        self.paragraph.font_generation()
    }
}

fn effective_line_height(line_height: f32) -> f32 {
    if line_height > 0.0 {
        line_height
    } else {
        text_defaults::LINE_HEIGHT
    }
}

#[cfg(test)]
#[path = "painter_wrap_measure_tests.rs"]
mod tests;
