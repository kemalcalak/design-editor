//! Chrome runs are painted in the family they are measured in (#441).
//!
//! Every surface here is painted into [`FamilyGapBackend::chrome_strict`],
//! where the chrome family ([`CHROME_FONT_FAMILY`]) and every other named
//! family measure differently, and checked three ways:
//!
//! 1. every run is painted in the chrome family, or in a family the surface
//!    deliberately uses (a jian component's own "Inter", a monospace
//!    readout) and names here;
//! 2. every run whose exact text the surface measured was measured in the
//!    family and weight it is painted in — the issue itself: a label fitted
//!    or centred in Manrope and drawn in `system-ui`, or sized at 400 and
//!    drawn at 600;
//! 3. widening the chrome face pushes no chrome run out of the surface's box
//!    further than the control (where all faces agree) does, so whatever the
//!    surface fits, centres or sizes from a label holds at the painted width.
//!    Runs in a deliberately different family are left to their owner (a
//!    jian component lays its own rows out); check 2 still pins them.
//!
//! Surfaces whose hit boxes are sized from a label additionally check the
//! painted run against the box the backend-less hit-test resolves.

use crate::widgets::label_widths::reset;
use crate::widgets::test_family_gap_backend::FamilyGapBackend;
use crate::widgets::text_metrics::CHROME_FONT_FAMILY;
use crate::widgets::PaintCx;
use crate::Rect;

mod modals;
mod overlays;
mod preview;

/// Paint `f` into `backend` from an empty label cache and hand the backend
/// back with everything it drew, plus the box `f` reports as its surface.
fn paint_into(
    mut backend: FamilyGapBackend,
    f: &impl Fn(&mut PaintCx<'_>) -> Rect,
) -> (FamilyGapBackend, Rect) {
    reset();
    let container = {
        let mut cx = PaintCx {
            backend: &mut backend,
        };
        f(&mut cx)
    };
    (backend, container)
}

/// Paint `f` (which returns the box it painted into) and assert the #441
/// contract. `also` names the non-chrome families the surface deliberately
/// paints, each measured in itself. Returns the family-strict backend; the
/// label cache is left holding the widths it measured, so a hit-test right
/// after reads the painted widths.
#[track_caller]
pub(super) fn assert_drawn_as_measured(
    what: &str,
    also: &[&str],
    f: impl Fn(&mut PaintCx<'_>) -> Rect,
) -> FamilyGapBackend {
    // A surface sized from its labels (a tooltip, a toast) is a different
    // box in each world, so each world is checked against its own.
    let (control, control_box) = paint_into(FamilyGapBackend::uniform(), &f);
    let (strict, container) = paint_into(FamilyGapBackend::chrome_strict(), &f);
    assert!(
        !strict.runs.is_empty(),
        "{what} painted no text — the check would pass vacuously"
    );

    for run in strict.runs.iter().filter(|run| !run.text.trim().is_empty()) {
        assert!(
            run.family == CHROME_FONT_FAMILY || also.contains(&run.family.as_str()),
            "{what}: {:?} is painted in {:?}, not the chrome family {CHROME_FONT_FAMILY:?}",
            run.text,
            run.family,
        );
        // Family-blind measurements (`""`) come from jian components and are
        // kept out of widget code by `tools/check-text-measure.sh`; what is
        // checked here is that no NAMED family other than the painted one
        // sized this run, and none at another weight.
        let measured: Vec<(&str, u16)> = strict
            .styles_measured_for(&run.text)
            .into_iter()
            .filter(|(family, _)| !family.is_empty())
            .collect();
        assert!(
            measured.is_empty() || measured.contains(&(run.family.as_str(), run.font_weight)),
            "{what}: {:?} is painted in {:?} at {} but measured in {measured:?}",
            run.text,
            run.family,
            run.font_weight,
        );
    }

    let chrome = |run: &&crate::widgets::test_family_gap_backend::PaintedRun| {
        run.family == CHROME_FONT_FAMILY
    };
    let strict_over: Vec<_> = strict
        .overflowing(container)
        .into_iter()
        .filter(chrome)
        .collect();
    let control_over: Vec<_> = control
        .overflowing(control_box)
        .into_iter()
        .filter(chrome)
        .collect();
    assert!(
        strict_over.len() <= control_over.len(),
        "{what}: at the painted face {} run(s) leave [{}, {}] versus {} in the control: {:?}",
        strict_over.len(),
        container.origin.x,
        container.origin.x + container.size.x,
        control_over.len(),
        strict_over
            .iter()
            .map(|run| (&run.text, &run.family, run.origin.x, run.right_edge()))
            .collect::<Vec<_>>(),
    );
    strict
}

/// The check has to be able to fail: a label fitted in the chrome family
/// but painted in `system-ui` — exactly the shipped bug — is caught.
#[test]
#[should_panic(expected = "is painted in")]
fn the_check_catches_a_label_painted_in_another_family() {
    use crate::{Color, Point2D, TextLayout};

    assert_drawn_as_measured("mismatched label", &["system-ui"], |cx| {
        let container = Rect::xywh(0.0, 0.0, 100.0, 30.0);
        let fitted = crate::widgets::text_metrics::fit_chrome(
            cx.backend,
            "a long chrome label here",
            container.size.x,
            12.0,
        );
        let layout = TextLayout::single_run(
            &fitted,
            "system-ui",
            12.0,
            Color::WHITE.to_jian(),
            Point2D::ZERO,
        );
        cx.backend.draw_text(&layout, Point2D::ZERO);
        container
    });
}

/// …and a label painted in a family nobody named is caught even when
/// nothing measured it.
#[test]
#[should_panic(expected = "not the chrome family")]
fn the_check_catches_an_unnamed_family() {
    use crate::{Color, Point2D, TextLayout};

    assert_drawn_as_measured("stray family", &[], |cx| {
        let layout = TextLayout::single_run(
            "Label",
            "system-ui",
            12.0,
            Color::WHITE.to_jian(),
            Point2D::ZERO,
        );
        cx.backend.draw_text(&layout, Point2D::ZERO);
        Rect::xywh(0.0, 0.0, 100.0, 30.0)
    });
}

/// …and a label fitted at the regular weight but painted semibold, which
/// the strict machine paints wider than it was measured.
#[test]
#[should_panic(expected = "at 600 but measured in")]
fn the_check_catches_a_label_painted_at_another_weight() {
    use crate::{Color, Point2D, TextLayout};

    assert_drawn_as_measured("mismatched weight", &[], |cx| {
        let container = Rect::xywh(0.0, 0.0, 100.0, 30.0);
        let fitted = crate::widgets::text_metrics::fit_chrome(
            cx.backend,
            "Публикация проекта",
            container.size.x,
            12.0,
        );
        let layout = TextLayout::single_run(
            &fitted,
            CHROME_FONT_FAMILY,
            12.0,
            Color::WHITE.to_jian(),
            Point2D::ZERO,
        )
        .with_font_weight(600);
        cx.backend.draw_text(&layout, Point2D::ZERO);
        container
    });
}
