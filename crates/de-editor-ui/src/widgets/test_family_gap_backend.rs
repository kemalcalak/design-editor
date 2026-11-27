//! Test-only `RenderBackend` whose named-family measurement is 40% wider
//! than its family-blind one — the shape of the real native gap, where
//! `measure_text` resolves the bundled Roboto but a named run paints in
//! its own face.
//!
//! Every other test backend measures blind and family-aware identically, so
//! a widget that fits / centres / sizes against `RenderBackend::measure_text`
//! looks perfect under them and shears on a real machine. Painting a panel
//! into this backend reproduces the machine: a fitter that measured blind
//! emits a string too wide for its column, and
//! [`PaintedRun::width_in_paint_family`] catches it.
//!
//! The cross-panel guard in `text_metrics_paint_tests` is the intended
//! consumer — assert every captured run fits the column it was painted into.
//!
//! [`FamilyGapBackend::chrome_strict`] goes further (#441). There every face
//! differs the way real faces do: a named family other than the chrome one
//! is wider in Latin, Cyrillic and CJK alike, Cyrillic is narrower than a
//! full em in either, and a heavier weight is wider than a lighter one. Every
//! family-aware measurement is recorded with its family and weight
//! ([`FamilyGapBackend::measured`]), so a surface that measures a label in
//! one family or weight and paints it in another both spills its box and is
//! caught naming the two.

use crate::widgets::text_metrics::CHROME_FONT_FAMILY;
use crate::{Color, Point2D, Rect, RenderBackend, TextLayout};

/// Advance as a fraction of font size for the backend's *default* typeface —
/// what `measure_text` reports. Matches jian's own fallback heuristic
/// (`Painter::measure_text`: `font_size * 0.55` per ASCII char, full size for
/// everything else) so the blind number here is the number a font-less
/// environment really produces.
const BLIND_ASCII_RATIO: f32 = 0.55;
/// Same, for any *named* family — what `draw_text` will actually paint.
/// Deliberately 40% wider, the shape of the Roboto → SF Pro gap.
const FAMILY_ASCII_RATIO: f32 = 0.77;
/// Non-ASCII advances outside [`FamilyGapBackend::chrome_strict`] are ~1em
/// and the SAME in every face: those backends model the Latin-advance gap
/// between the default face and a named one, and inflating non-Latin there
/// would manufacture failures the cross-panel guard was not built for.
const WIDE_RATIO: f32 = 1.0;

/// Per-script advances of one face in [`FamilyGapBackend::chrome_strict`].
#[derive(Debug, Clone, Copy)]
struct ScriptAdvances {
    /// ASCII and the Latin supplements (é, ı, ş, ã …).
    latin: f32,
    /// Cyrillic (and Greek).
    cyrillic: f32,
    /// CJK, kana and Hangul — full-width glyphs.
    wide: f32,
}

/// The chrome face in the strict machine.
const STRICT_CHROME: ScriptAdvances = ScriptAdvances {
    latin: FAMILY_ASCII_RATIO,
    cyrillic: 0.7,
    wide: 1.0,
};
/// Every other named face in the strict machine: wider in every script, so a
/// run painted in one family and fitted in the other cannot land in the
/// right box whichever language it is in.
const STRICT_FOREIGN: ScriptAdvances = ScriptAdvances {
    latin: 0.95,
    cyrillic: 0.85,
    wide: 1.1,
};
/// Extra advance per 100 weight units above regular (400) in the strict
/// machine: a 600 run is 8% wider than the same text at 400.
const WEIGHT_STEP: f32 = 0.04;

/// Width model of one backend: blind, uniform, default, or strict.
#[derive(Debug, Clone, Copy)]
struct Faces {
    /// Latin advance of a named family outside the strict machine.
    family_ascii_ratio: f32,
    /// Whether families, scripts and weights are told apart (#441).
    strict: bool,
}

impl Faces {
    fn width(&self, text: &str, font_size: f32, family: &str, weight: u16) -> f32 {
        if family.is_empty() {
            return text
                .chars()
                .map(|c| {
                    font_size
                        * if c.is_ascii() {
                            BLIND_ASCII_RATIO
                        } else {
                            WIDE_RATIO
                        }
                })
                .sum();
        }
        if !self.strict {
            return text
                .chars()
                .map(|c| {
                    font_size
                        * if c.is_ascii() {
                            self.family_ascii_ratio
                        } else {
                            WIDE_RATIO
                        }
                })
                .sum();
        }
        let face = if family == CHROME_FONT_FAMILY {
            STRICT_CHROME
        } else {
            STRICT_FOREIGN
        };
        let weight_factor = 1.0 + f32::from(weight.saturating_sub(400)) / 100.0 * WEIGHT_STEP;
        text.chars()
            .map(|c| {
                let advance = match c as u32 {
                    0x00..=0x024F => face.latin,
                    0x0370..=0x04FF => face.cyrillic,
                    0x1100..=0x115F
                    | 0x2E80..=0xA4CF
                    | 0xAC00..=0xD7A3
                    | 0xF900..=0xFAFF
                    | 0xFF00..=0xFF60 => face.wide,
                    // Thai, Devanagari and the rest resolve through the
                    // same system fallback whichever family a run names.
                    _ => WIDE_RATIO,
                };
                font_size * advance * weight_factor
            })
            .sum()
    }
}

/// One `draw_text` call, with everything needed to re-measure it the way it
/// was painted.
#[derive(Debug, Clone)]
pub(crate) struct PaintedRun {
    pub(crate) text: String,
    pub(crate) family: String,
    pub(crate) font_size: f32,
    pub(crate) font_weight: u16,
    /// Origin passed to `draw_text` — x is the run's left edge.
    pub(crate) origin: Point2D,
    /// Innermost clip in effect when the run was drawn, if any. A run wider
    /// than this is SHEARED on screen — glyphs cut mid-stroke with no
    /// ellipsis, which is the exact symptom this whole guard exists for.
    pub(crate) clip: Option<Rect>,
    /// The painting backend's width model.
    faces: Faces,
}

impl PaintedRun {
    /// Width this run occupies on screen, measured in the family and weight
    /// it names.
    pub(crate) fn width_in_paint_family(&self) -> f32 {
        self.faces
            .width(&self.text, self.font_size, &self.family, self.font_weight)
    }

    /// Right edge of the painted run.
    pub(crate) fn right_edge(&self) -> f32 {
        self.origin.x + self.width_in_paint_family()
    }

    /// Whether the run spills past `container` or past its own clip. Both
    /// read the same on screen: text that does not fit the box it was put in.
    fn spills(&self, container: Rect) -> bool {
        let (mut left, mut right) = (container.origin.x, container.origin.x + container.size.x);
        if let Some(clip) = self.clip {
            left = left.max(clip.origin.x);
            right = right.min(clip.origin.x + clip.size.x);
        }
        self.origin.x < left - 0.01 || self.right_edge() > right + 0.01
    }
}

/// One family-aware measurement: the text and the family and weight it was
/// asked in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MeasuredRun {
    pub(crate) text: String,
    pub(crate) family: String,
    pub(crate) weight: u16,
}

pub(crate) struct FamilyGapBackend {
    /// Every `draw_text`, in paint order.
    pub(crate) runs: Vec<PaintedRun>,
    /// Every measurement, in call order (the family-blind ones are recorded
    /// with an empty family).
    pub(crate) measured: Vec<MeasuredRun>,
    /// Every `fill_rect`, in paint order (a text input's caret is one).
    pub(crate) rect_fills: Vec<Rect>,
    /// Live clip stack — `save` / `restore` / `clip_rect` are modelled so a
    /// run's recorded clip is the one actually in effect when it was drawn.
    clip_stack: Vec<Option<Rect>>,
    faces: Faces,
}

impl Default for FamilyGapBackend {
    fn default() -> Self {
        Self {
            runs: Vec::new(),
            measured: Vec::new(),
            rect_fills: Vec::new(),
            clip_stack: vec![None],
            faces: Faces {
                family_ascii_ratio: FAMILY_ASCII_RATIO,
                strict: false,
            },
        }
    }
}

impl FamilyGapBackend {
    /// The control: named families measure exactly like the default face, so
    /// nothing a family-aware fitter does can change the outcome. Paint a
    /// panel into this and into [`Self::default`] and diff — see
    /// `text_metrics_paint_tests`.
    pub(crate) fn uniform() -> Self {
        Self {
            faces: Faces {
                family_ascii_ratio: BLIND_ASCII_RATIO,
                strict: false,
            },
            ..Self::default()
        }
    }

    /// The family-strict machine (#441): see the module docs. A run fitted
    /// in one family or weight and painted in another spills its box here,
    /// and [`Self::measured`] names the family and weight each width was
    /// asked in.
    pub(crate) fn chrome_strict() -> Self {
        Self {
            faces: Faces {
                family_ascii_ratio: FAMILY_ASCII_RATIO,
                strict: true,
            },
            ..Self::default()
        }
    }

    /// The (family, weight) pairs `text` was measured in, in call order.
    pub(crate) fn styles_measured_for<'a>(&'a self, text: &'a str) -> Vec<(&'a str, u16)> {
        self.measured
            .iter()
            .filter(|m| m.text == text)
            .map(|m| (m.family.as_str(), m.weight))
            .collect()
    }

    /// Runs that do not fit the box they were painted into — either
    /// `container` or, where one is in effect, their own clip.
    pub(crate) fn overflowing(&self, container: Rect) -> Vec<&PaintedRun> {
        self.runs
            .iter()
            .filter(|run| run.spills(container))
            .collect()
    }

    fn clip(&self) -> Option<Rect> {
        self.clip_stack.last().copied().flatten()
    }

    fn record(&mut self, text: &str, font_size: f32, family: &str, weight: u16) -> f32 {
        self.measured.push(MeasuredRun {
            text: text.to_string(),
            family: family.to_string(),
            weight,
        });
        self.faces.width(text, font_size, family, weight)
    }
}

impl RenderBackend for FamilyGapBackend {
    fn begin_frame(&mut self) {}
    fn end_frame(&mut self) {}
    fn fill_rect(&mut self, rect: Rect, _: Color) {
        self.rect_fills.push(rect);
    }
    fn stroke_rect(&mut self, _: Rect, _: Color, _: f32) {}
    fn draw_text(&mut self, layout: &TextLayout, point: Point2D) {
        for run in layout.runs() {
            self.runs.push(PaintedRun {
                text: run.content.clone(),
                family: run.font_family.clone(),
                font_size: run.font_size,
                font_weight: run.font_weight,
                origin: Point2D::new(point.x + run.origin.x, point.y + run.origin.y),
                clip: self.clip(),
                faces: self.faces,
            });
        }
    }
    fn clip_rect(&mut self, rect: Rect) {
        let merged = match self.clip() {
            Some(current) => intersect_x(current, rect),
            None => rect,
        };
        if let Some(top) = self.clip_stack.last_mut() {
            *top = Some(merged);
        }
    }
    fn stroke_line(&mut self, _: Point2D, _: Point2D, _: Color, _: f32) {}
    fn fill_round_rect(&mut self, _: Rect, _: f32, _: Color) {}
    fn stroke_round_rect(&mut self, _: Rect, _: f32, _: Color, _: f32) {}
    fn stroke_svg_path(&mut self, _: &str, _: Point2D, _: f32, _: Color, _: f32) {}
    fn measure_text(&mut self, text: &str, font_size: f32) -> f32 {
        self.record(text, font_size, "", 400)
    }
    fn measure_text_weighted(&mut self, text: &str, font_size: f32, weight: u16) -> f32 {
        self.record(text, font_size, "", weight)
    }
    fn measure_text_family(&mut self, text: &str, font_size: f32, family: &str) -> f32 {
        self.record(text, font_size, family, 400)
    }
    fn measure_text_family_styled(
        &mut self,
        text: &str,
        font_size: f32,
        family: &str,
        weight: u16,
        _italic: bool,
    ) -> f32 {
        self.record(text, font_size, family, weight)
    }
    fn save(&mut self) {
        self.clip_stack.push(self.clip());
    }
    fn restore(&mut self) {
        if self.clip_stack.len() > 1 {
            self.clip_stack.pop();
        }
    }
    fn translate(&mut self, _: Point2D) {}
    fn resize(&mut self, _: u32, _: u32) {}
    fn dpi_scale(&self) -> f32 {
        1.0
    }
}

/// Horizontal intersection of two clips. Only x matters here — every guard
/// this backend serves asks "does the text fit its column".
fn intersect_x(a: Rect, b: Rect) -> Rect {
    let left = a.origin.x.max(b.origin.x);
    let right = (a.origin.x + a.size.x).min(b.origin.x + b.size.x);
    Rect::xywh(left, a.origin.y, (right - left).max(0.0), a.size.y)
}
