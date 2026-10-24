//! Layout measure that wraps text the way the canvas painter does (#186).
//!
//! jian's `SkiaMeasure` breaks lines with Skia's paragraph shaper, but the
//! canvas paints (and the inline editor places carets) with the greedy wrap
//! in `de_editor_core::text_wrap` over jian-skia's `FontResolver` advances:
//! no kerning, plus the synthetic-bold width allowance for weights the face
//! does not carry. The two disagree by a pixel or two on a plain line and by
//! about 3 % on a synthetic-bold one, so a `fit_content` box could be sized
//! for one line while the canvas painted two. For text in one style and a
//! simple script this backend counts the lines with the painter's wrap and
//! measure; everything else (mixed styles, scripts that need shaping, the
//! natural extent) keeps `SkiaMeasure`'s answer.

use jian_core::layout::measure::{FontStyleKind, MeasureBackend, MeasureRequest, MeasureResult};

/// Bundled last-resort face of the native painter (`NativeBackend`), so a
/// family nothing resolves measures in the face the canvas paints.
const ROBOTO_TTF: &[u8] = include_bytes!("../../de-host-native/assets/Roboto-Regular.ttf");

/// The painter's family when a node names none.
const DEFAULT_FAMILY: &str = "system-ui";

/// Painter line advance when no `lineHeight` is authored.
const DEFAULT_LINE_HEIGHT: f32 = 1.2;

pub(crate) struct PainterWrapMeasure {
    paragraph: jian_skia::SkiaMeasure,
    resolver: jian_skia::FontResolver,
}

impl PainterWrapMeasure {
    pub(crate) fn new() -> Self {
        let resolver = jian_skia::with_font_lock(|| {
            let font_mgr = skia_safe::FontMgr::new();
            let default_typeface = font_mgr.new_from_data(ROBOTO_TTF, None);
            jian_skia::FontResolver::with_default_typeface(font_mgr, default_typeface)
        });
        Self {
            paragraph: jian_skia::SkiaMeasure::new(),
            resolver,
        }
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
            .unwrap_or(DEFAULT_FAMILY),
        size: first.font_size,
        weight: if first.font_weight > 0 {
            first.font_weight
        } else {
            400
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
            let base = self.resolver.measure_text(
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
        let height = if req.line_height > 0.0 || run.text.contains('\n') {
            // Same rule as `SkiaMeasure`: authored or literal multi-line
            // text advances by the painter's line height.
            f32::from(line_count) * run.size * effective_line_height(req.line_height)
        } else if line_count == shaped.line_count {
            shaped.height
        } else {
            // Default line height: keep the shaper's per-line metric.
            shaped.height / f32::from(shaped.line_count.max(1)) * f32::from(line_count)
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
        DEFAULT_LINE_HEIGHT
    }
}

#[cfg(test)]
#[path = "painter_wrap_measure_tests.rs"]
mod tests;
