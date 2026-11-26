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
//! A run nothing has measured yet (a unit test without a backend, a press
//! that lands before the first frame painted a new label) reads
//! [`estimate_width`], a backend-free heuristic. It is the one place that
//! estimate remains, and only as that fallback.

use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};

use super::line_break::wrap_lines;
use super::text_metrics::{measure_in_family, measure_in_family_weighted, CHROME_FONT_FAMILY};
use crate::RenderBackend;
use de_editor_core::text_wrap::wrap_text_measured;

/// Widths one generation holds before it rolls over (see the module docs).
const GENERATION_CAP: usize = 4096;

/// The regular weight, measured without a weight argument (as
/// `text_metrics::measure_chrome` does).
const REGULAR: u16 = 400;

#[derive(Default)]
struct Generations {
    current: HashMap<u64, f32>,
    previous: HashMap<u64, f32>,
}

impl Generations {
    fn get(&mut self, key: u64) -> Option<f32> {
        if let Some(width) = self.current.get(&key) {
            return Some(*width);
        }
        let width = self.previous.get(&key).copied()?;
        self.insert(key, width);
        Some(width)
    }

    fn insert(&mut self, key: u64, width: f32) {
        if self.current.len() >= GENERATION_CAP {
            self.previous = std::mem::take(&mut self.current);
        }
        self.current.insert(key, width);
    }
}

thread_local! {
    static CACHE: RefCell<Generations> = RefCell::new(Generations::default());
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
        let width = if weight == REGULAR {
            measure_in_family(backend, text, size, family)
        } else {
            measure_in_family_weighted(backend, text, size, family, weight)
        };
        CACHE.with(|cache| cache.borrow_mut().insert(key, width));
        width
    }

    /// Wrap `text` into lines no wider than `max_w` as a run in `family` at
    /// `weight`: Latin at spaces, CJK between characters
    /// (`wrap_text_measured`), and a line still too wide (Thai writes
    /// phrases without spaces) again between clusters (`wrap_lines`).
    /// The chrome's wrapped copy wraps here, so paint and hit-test break
    /// the same lines.
    pub fn wrap(
        &mut self,
        text: &str,
        max_w: f32,
        size: f32,
        family: &str,
        weight: u16,
    ) -> Vec<String> {
        let lines = wrap_text_measured(text, max_w, &mut |s: &str| {
            self.styled(s, size, family, weight)
        });
        lines
            .into_iter()
            .flat_map(|line| wrap_lines(&line, max_w, |s| self.styled(s, size, family, weight)))
            .collect()
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
    CACHE.with(|cache| *cache.borrow_mut() = Generations::default());
}

/// What an unmeasured run reads, for tests that pin the fallback.
#[cfg(test)]
pub(crate) fn unmeasured_width(text: &str, size: f32) -> f32 {
    estimate_width(text, size)
}

#[cfg(test)]
#[path = "label_widths_tests.rs"]
mod tests;
