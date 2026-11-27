//! Measured chrome label widths that paint and backend-less hit-tests
//! share (#435, #440).
//!
//! A chrome control whose box is sized from its label (a chip, a footer
//! button, a segmented control, a modal's wrapped copy) must be hit-tested
//! at the size it is painted at. Paint has a backend and can measure the
//! label; the shared press / hover / tooltip flows do not. So every such
//! layout is written once, against [`LabelWidths`], and run in two modes:
//!
//! - [`LabelWidths::measuring`] — paint (or a host holding a measure-only
//!   backend outside paint): a width not measured yet is measured through
//!   [`text_metrics`](super::text_metrics), in the family and weight the run
//!   is painted in, and kept;
//! - [`LabelWidths::cached`] — hit-test: reads the kept width.
//!
//! Both modes resolve a run to the same number once paint measured it, so
//! the painted box and the clickable box are one rect.
//!
//! The cache is one thread-local map keyed on everything that decides a
//! run's width: its text, size, family and weight. The locale and the label
//! set are therefore part of the key by construction (a locale switch or a
//! new file name is a new text, measured on the next paint); a key that is
//! already there measures nothing and allocates nothing, so a repaint, a
//! hover or a download-progress tick never shapes text again.
//!
//! The map is bounded by two generations: once the current one holds
//! [`GENERATION_CAP`] widths it becomes the previous one, and a lookup that
//! hits the previous generation copies the width forward. Whatever the last
//! frames painted therefore survives a rollover; only widths nothing has
//! used for a whole generation are dropped.
//!
//! Wrapped copy is kept separately, as whole lines ([`LabelWidths::wrap`]):
//! a wrap measures many prefixes once, and keeping each of them as a width
//! would fill the width map with strings nothing reads again.
//!
//! An ellipsized label is kept the same way ([`LabelWidths::fit`]): a label
//! that fits is handed back borrowed, one that does not is cut once and the
//! cut form kept, so a repaint neither measures nor allocates for it.
//!
//! A run nothing has measured yet (a unit test without a backend, a press
//! that lands before the first frame painted a new label) reads
//! [`estimate_width`], a backend-free heuristic. It is the one place that
//! estimate remains, and only as that fallback.

use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::ops::Deref;
use std::rc::Rc;

use super::line_break::wrap_lines;
use super::text_metrics::{measure_in_family, measure_in_family_weighted, CHROME_FONT_FAMILY};
use crate::RenderBackend;
use de_editor_core::text_wrap::wrap_text_measured;

/// Widths one generation holds before it rolls over (see the module docs).
const GENERATION_CAP: usize = 4096;
/// Wrapped texts one generation holds: a wrap is a whole paragraph, so far
/// fewer are live at once than single runs.
const WRAP_GENERATION_CAP: usize = 256;
/// Ellipsized labels one generation holds: one per label that does not fit
/// its box, so far fewer than whole widths.
const FIT_GENERATION_CAP: usize = 1024;

/// How far past `max_w` a whole label may measure and still fit: float
/// noise, far below a device pixel.
const FIT_SLACK: f32 = 0.01;

/// The regular weight, measured without a weight argument (as
/// `text_metrics::measure_chrome` does).
const REGULAR: u16 = 400;

/// A map bounded by two generations of `cap` entries: a full current
/// generation becomes the previous one, and a hit in the previous one is
/// copied forward, so whatever recent frames used survives a rollover.
struct Generations<V> {
    cap: usize,
    current: HashMap<u64, V>,
    previous: HashMap<u64, V>,
}

impl<V: Clone> Generations<V> {
    fn new(cap: usize) -> Self {
        Self {
            cap,
            current: HashMap::new(),
            previous: HashMap::new(),
        }
    }

    fn get(&mut self, key: u64) -> Option<V> {
        if let Some(value) = self.current.get(&key) {
            return Some(value.clone());
        }
        let value = self.previous.get(&key).cloned()?;
        self.insert(key, value.clone());
        Some(value)
    }

    fn insert(&mut self, key: u64, value: V) {
        if self.current.len() >= self.cap {
            self.previous = std::mem::take(&mut self.current);
        }
        self.current.insert(key, value);
    }
}

thread_local! {
    static CACHE: RefCell<Generations<f32>> = RefCell::new(Generations::new(GENERATION_CAP));
    /// Wrapped lines, keyed on the text and everything its breaks depend
    /// on (`wrap_key`). The per-prefix measurements a wrap makes are not
    /// kept: only the lines are, so a long CJK or Thai paragraph costs one
    /// entry here instead of one width entry per prefix in `CACHE`.
    static WRAPS: RefCell<Generations<Rc<[String]>>> =
        RefCell::new(Generations::new(WRAP_GENERATION_CAP));
    /// Ellipsized labels, keyed like a wrap (`wrap_key`). Only the cut form
    /// is kept, never the prefixes measured to find it.
    static FITS: RefCell<Generations<Rc<str>>> =
        RefCell::new(Generations::new(FIT_GENERATION_CAP));
}

fn wrap_key(text: &str, max_w: f32, size: f32, family: &str, weight: u16) -> u64 {
    let mut hasher = DefaultHasher::new();
    key(text, size, family, weight).hash(&mut hasher);
    max_w.to_bits().hash(&mut hasher);
    hasher.finish()
}

/// Width of one run through `backend`, in `family` at `weight`.
fn measure_run(
    backend: &mut dyn RenderBackend,
    text: &str,
    size: f32,
    family: &str,
    weight: u16,
) -> f32 {
    if weight == REGULAR {
        measure_in_family(backend, text, size, family)
    } else {
        measure_in_family_weighted(backend, text, size, family, weight)
    }
}

/// Break `text` with `measure` (see [`LabelWidths::wrap`]).
fn wrap_with(text: &str, max_w: f32, measure: &mut dyn FnMut(&str) -> f32) -> Vec<String> {
    wrap_text_measured(text, max_w, measure)
        .into_iter()
        .flat_map(|line| wrap_lines(&line, max_w, &mut *measure))
        .collect()
}

/// Cut `text` until it and a trailing `…` fit `max_w` as `measure` reports
/// it (a bare `…` when not even one character does).
fn ellipsize_with(text: &str, max_w: f32, measure: &mut dyn FnMut(&str) -> f32) -> Rc<str> {
    let mut cut = String::with_capacity(text.len() + '…'.len_utf8());
    let mut fitting = 0;
    for (index, ch) in text.char_indices() {
        cut.truncate(0);
        cut.push_str(&text[..index + ch.len_utf8()]);
        cut.push('…');
        if measure(&cut) > max_w {
            break;
        }
        fitting = index + ch.len_utf8();
    }
    cut.truncate(0);
    cut.push_str(&text[..fitting]);
    cut.push('…');
    cut.into()
}

/// A label as it fits its box: borrowed whole, or the kept ellipsized form.
/// Derefs to the text to paint.
#[derive(Debug, Clone)]
pub enum Fitted<'t> {
    /// The whole label fits.
    Whole(&'t str),
    /// The label was cut and ends in `…`.
    Cut(Rc<str>),
}

impl Deref for Fitted<'_> {
    type Target = str;

    fn deref(&self) -> &str {
        match self {
            Fitted::Whole(text) => text,
            Fitted::Cut(text) => text,
        }
    }
}

/// What decides one run's painted width.
fn key(text: &str, size: f32, family: &str, weight: u16) -> u64 {
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    size.to_bits().hash(&mut hasher);
    family.hash(&mut hasher);
    weight.hash(&mut hasher);
    hasher.finish()
}

/// Label widths for one layout pass: measuring (paint) or cached
/// (hit-test). See the module docs.
pub struct LabelWidths<'a> {
    backend: Option<&'a mut dyn RenderBackend>,
}

impl<'a> LabelWidths<'a> {
    /// Measure any run not measured yet through `backend`, and keep it.
    pub fn measuring(backend: &'a mut dyn RenderBackend) -> Self {
        Self {
            backend: Some(backend),
        }
    }

    /// Read the kept widths; measure nothing.
    pub fn cached() -> LabelWidths<'static> {
        LabelWidths { backend: None }
    }

    /// Whether this pass measures (paint) rather than reads (hit-test). A
    /// caller that keeps a layout of its own built from these widths keeps
    /// only a measured one, as [`wrap`](Self::wrap) does.
    pub fn is_measuring(&self) -> bool {
        self.backend.is_some()
    }

    /// Width of a regular run in the chrome family.
    pub fn chrome(&mut self, text: &str, size: f32) -> f32 {
        self.styled(text, size, CHROME_FONT_FAMILY, REGULAR)
    }

    /// Width of a run in the chrome family at `weight`.
    pub fn chrome_weighted(&mut self, text: &str, size: f32, weight: u16) -> f32 {
        self.styled(text, size, CHROME_FONT_FAMILY, weight)
    }

    /// Width of a run painted in `family` at `weight` — for the runs that
    /// are not in the chrome family (a jian component's own face, mono).
    pub fn styled(&mut self, text: &str, size: f32, family: &str, weight: u16) -> f32 {
        let key = key(text, size, family, weight);
        if let Some(width) = CACHE.with(|cache| cache.borrow_mut().get(key)) {
            return width;
        }
        let Some(backend) = self.backend.as_deref_mut() else {
            return estimate_width(text, size);
        };
        let width = measure_run(backend, text, size, family, weight);
        CACHE.with(|cache| cache.borrow_mut().insert(key, width));
        width
    }

    /// Wrap `text` into lines no wider than `max_w` as a run in `family` at
    /// `weight`: Latin at spaces, CJK between characters
    /// (`wrap_text_measured`), and a line still too wide (Thai writes
    /// phrases without spaces) again between clusters (`wrap_lines`).
    /// The chrome's wrapped copy wraps here, so paint and hit-test break
    /// the same lines.
    ///
    /// The lines are kept whole (`WRAPS`), so a repaint or a hit-test
    /// reads them with one lookup and never re-measures a prefix. Only a
    /// measuring pass keeps a wrap; a cached pass that misses breaks
    /// against the estimate and keeps nothing.
    pub fn wrap(
        &mut self,
        text: &str,
        max_w: f32,
        size: f32,
        family: &str,
        weight: u16,
    ) -> Rc<[String]> {
        let key = wrap_key(text, max_w, size, family, weight);
        if let Some(lines) = WRAPS.with(|wraps| wraps.borrow_mut().get(key)) {
            return lines;
        }
        let Some(backend) = self.backend.as_deref_mut() else {
            return wrap_with(text, max_w, &mut |s| estimate_width(s, size)).into();
        };
        let lines: Rc<[String]> = wrap_with(text, max_w, &mut |s| {
            measure_run(backend, s, size, family, weight)
        })
        .into();
        WRAPS.with(|wraps| wraps.borrow_mut().insert(key, lines.clone()));
        lines
    }
}

impl LabelWidths<'_> {
    /// `text` as it fits `max_w` painted in `family` at `weight`: whole
    /// (borrowed) when it fits, else cut to end in `…`.
    ///
    /// The whole label's width is read like any run ([`styled`](Self::styled)),
    /// so a label that fits costs one lookup and no allocation. A cut form is
    /// found by measuring prefixes through the backend and kept whole
    /// (`FITS`), so the next repaint reads it with one lookup; a cached pass
    /// that misses cuts against the estimate and keeps nothing.
    pub fn fit<'t>(
        &mut self,
        text: &'t str,
        max_w: f32,
        size: f32,
        family: &str,
        weight: u16,
    ) -> Fitted<'t> {
        // `max_w` is usually a box sized from this very width with padding
        // added and taken off again; the slack absorbs that float noise.
        if self.styled(text, size, family, weight) <= max_w + FIT_SLACK {
            return Fitted::Whole(text);
        }
        let key = wrap_key(text, max_w, size, family, weight);
        if let Some(cut) = FITS.with(|fits| fits.borrow_mut().get(key)) {
            return Fitted::Cut(cut);
        }
        let Some(backend) = self.backend.as_deref_mut() else {
            return Fitted::Cut(ellipsize_with(text, max_w, &mut |s| {
                estimate_width(s, size)
            }));
        };
        let cut = ellipsize_with(text, max_w, &mut |s| {
            measure_run(backend, s, size, family, weight)
        });
        FITS.with(|fits| fits.borrow_mut().insert(key, cut.clone()));
        Fitted::Cut(cut)
    }

    /// [`fit`](Self::fit) for a regular run in the chrome family.
    pub fn fit_chrome<'t>(&mut self, text: &'t str, max_w: f32, size: f32) -> Fitted<'t> {
        self.fit(text, max_w, size, CHROME_FONT_FAMILY, REGULAR)
    }
}

/// A backend-free width heuristic for `text` at `size` px: the
/// [`LabelWidths::cached`] fallback for a run nothing has measured yet, and
/// nothing else. It errs wide for Latin and is only roughly right for the
/// rest, which is why no hit geometry may be sized from it once paint ran.
fn estimate_width(text: &str, size: f32) -> f32 {
    text.chars().fold(0.0, |w, c| {
        w + if c.is_ascii_uppercase() || c.is_ascii_digit() {
            size * 0.64
        } else if c == ' ' {
            size * 0.28
        } else if c.is_ascii() {
            size * 0.54
        } else if is_wide(c) {
            size
        } else {
            size * 0.6
        }
    })
}

fn is_wide(c: char) -> bool {
    matches!(c as u32,
        0x1100..=0x115F | 0x2E80..=0xA4CF | 0xAC00..=0xD7A3 | 0xF900..=0xFAFF | 0xFF00..=0xFF60)
}

/// Empty the cache, so a test starts from unmeasured labels.
#[cfg(test)]
pub(crate) fn reset() {
    CACHE.with(|cache| *cache.borrow_mut() = Generations::new(GENERATION_CAP));
    WRAPS.with(|wraps| *wraps.borrow_mut() = Generations::new(WRAP_GENERATION_CAP));
    FITS.with(|fits| *fits.borrow_mut() = Generations::new(FIT_GENERATION_CAP));
}

/// How many widths and wrapped texts the current generations hold.
#[cfg(test)]
pub(crate) fn kept() -> (usize, usize) {
    (
        CACHE.with(|cache| cache.borrow().current.len()),
        WRAPS.with(|wraps| wraps.borrow().current.len()),
    )
}

/// What an unmeasured run reads, for tests that pin the fallback.
#[cfg(test)]
pub(crate) fn unmeasured_width(text: &str, size: f32) -> f32 {
    estimate_width(text, size)
}

#[cfg(test)]
#[path = "label_widths_tests.rs"]
mod tests;
