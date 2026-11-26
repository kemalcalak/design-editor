//! Test support for #440: paint a chrome surface into the family-aware
//! [`FamilyGapBackend`] and check each label run against the box the
//! backend-less hit-test resolves for it.

use crate::widgets::label_widths::reset;
use crate::widgets::test_family_gap_backend::{FamilyGapBackend, PaintedRun};
use crate::Rect;

/// Slack for float sums laid end to end.
const EPS: f32 = 0.01;

/// A fresh cache and a backend whose named families measure wider than
/// its default face (and wider than the `label_widths` estimate for
/// Latin), so a box sized from anything but the painted face shows.
pub(crate) fn fresh_backend() -> FamilyGapBackend {
    reset();
    FamilyGapBackend::default()
}

/// The run painted with exactly `text`, unclipped and unellipsized.
pub(crate) fn run<'a>(backend: &'a FamilyGapBackend, text: &str, what: &str) -> &'a PaintedRun {
    backend
        .runs
        .iter()
        .find(|run| run.text == text)
        .unwrap_or_else(|| {
            let painted: Vec<&str> = backend.runs.iter().map(|r| r.text.as_str()).collect();
            panic!("{what}: {text:?} was not painted whole; painted {painted:?}")
        })
}

/// The run painted with exactly `text` whose baseline lies inside `rect`
/// (the same label may be painted elsewhere on the surface too).
pub(crate) fn run_in<'a>(
    backend: &'a FamilyGapBackend,
    text: &str,
    rect: Rect,
    what: &str,
) -> &'a PaintedRun {
    backend
        .runs
        .iter()
        .find(|run| {
            run.text == text
                && run.origin.y > rect.origin.y
                && run.origin.y <= rect.origin.y + rect.size.y
        })
        .unwrap_or_else(|| panic!("{what}: {text:?} was not painted whole in {rect:?}"))
}

/// `run` lies inside `rect` horizontally, inset by `inset` on each side.
pub(crate) fn assert_inside(run: &PaintedRun, rect: Rect, inset: f32, what: &str) {
    let left = rect.origin.x + inset - EPS;
    let right = rect.origin.x + rect.size.x - inset + EPS;
    assert!(
        run.origin.x >= left && run.right_edge() <= right,
        "{what}: {:?} paints {}..{} outside its hit box {}..{} (inset {inset})",
        run.text,
        run.origin.x,
        run.right_edge(),
        rect.origin.x,
        rect.origin.x + rect.size.x,
    );
}

/// `run` is centred in `rect`.
pub(crate) fn assert_centred(run: &PaintedRun, rect: Rect, what: &str) {
    let left_gap = run.origin.x - rect.origin.x;
    let right_gap = rect.origin.x + rect.size.x - run.right_edge();
    assert!(
        (left_gap - right_gap).abs() < 0.05,
        "{what}: {:?} sits {left_gap} / {right_gap} in its box",
        run.text
    );
}
