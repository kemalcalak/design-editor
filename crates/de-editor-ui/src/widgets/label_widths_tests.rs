//! The shared label-width cache (#440): a measuring pass keeps what it
//! measured, a cached pass reads it back without a backend, and an
//! unmeasured run reads the estimate.

use super::{reset, unmeasured_width, LabelWidths, GENERATION_CAP};
use crate::widgets::test_family_gap_backend::FamilyGapBackend;
use crate::widgets::text_metrics::{measure_chrome, measure_in_family, CHROME_FONT_FAMILY};

#[test]
fn a_cached_pass_reads_what_the_measuring_pass_measured() {
    reset();
    let mut backend = FamilyGapBackend::default();
    let expected = measure_chrome(&mut backend, "Export", 13.0);
    assert_eq!(
        LabelWidths::cached().chrome("Export", 13.0),
        unmeasured_width("Export", 13.0)
    );

    let measured = LabelWidths::measuring(&mut backend).chrome("Export", 13.0);

    assert_eq!(measured, expected);
    assert_eq!(LabelWidths::cached().chrome("Export", 13.0), expected);
}

#[test]
fn the_key_is_text_size_family_and_weight() {
    reset();
    let mut backend = FamilyGapBackend::default();
    LabelWidths::measuring(&mut backend).chrome("Save", 13.0);
    let mut cached = LabelWidths::cached();
    assert_eq!(cached.chrome("Save", 12.0), unmeasured_width("Save", 12.0));
    assert_eq!(
        cached.chrome("Saved", 13.0),
        unmeasured_width("Saved", 13.0)
    );
    assert_eq!(
        cached.chrome_weighted("Save", 13.0, 600),
        unmeasured_width("Save", 13.0)
    );
    assert_eq!(
        cached.styled("Save", 13.0, "Inter", 400),
        unmeasured_width("Save", 13.0)
    );
}

/// A run is measured in the family it names, not the backend default.
#[test]
fn a_run_is_measured_in_its_own_family() {
    reset();
    let mut backend = FamilyGapBackend::default();
    // The control backend's named families measure like its default face.
    let blind = measure_in_family(&mut FamilyGapBackend::uniform(), "Connect", 12.0, "Inter");
    let inter = LabelWidths::measuring(&mut backend).styled("Connect", 12.0, "Inter", 400);
    assert_eq!(
        inter,
        measure_in_family(&mut backend, "Connect", 12.0, "Inter")
    );
    assert!(inter > blind, "{inter} vs {blind}");
    assert_eq!(
        LabelWidths::measuring(&mut backend).chrome("Connect", 12.0),
        measure_in_family(&mut backend, "Connect", 12.0, CHROME_FONT_FAMILY)
    );
}

/// A hit measures nothing: a backend that would answer differently is
/// never asked again for a kept run.
#[test]
fn a_kept_run_is_not_measured_again() {
    reset();
    let first = LabelWidths::measuring(&mut FamilyGapBackend::default()).chrome("Got it", 13.0);
    let again = LabelWidths::measuring(&mut FamilyGapBackend::uniform()).chrome("Got it", 13.0);
    assert_eq!(first, again);
}

/// A rollover keeps the widths still in use: the previous generation is
/// read and copied forward.
#[test]
fn a_rollover_keeps_recently_used_widths() {
    reset();
    let mut backend = FamilyGapBackend::default();
    let kept = LabelWidths::measuring(&mut backend).chrome("kept", 13.0);
    for i in 0..GENERATION_CAP {
        LabelWidths::measuring(&mut backend).chrome(&format!("filler {i}"), 13.0);
    }
    assert_eq!(LabelWidths::cached().chrome("kept", 13.0), kept);
    for i in 0..GENERATION_CAP * 2 {
        LabelWidths::measuring(&mut backend).chrome(&format!("other {i}"), 13.0);
    }
    assert_eq!(
        LabelWidths::cached().chrome("kept", 13.0),
        unmeasured_width("kept", 13.0),
        "a width unused for two generations is dropped"
    );
}

/// A wrap is kept whole: its prefix measurements do not land in the width
/// cache, a cached pass reads the same lines, and a repaint measures
/// nothing (a backend that would break differently is never asked).
#[test]
fn a_wrap_is_kept_as_lines_not_as_prefix_widths() {
    reset();
    let text = "Claude で作ったものを含む HTML のスライドを開くと、各スライドが編集できるボードになります。";
    let mut gap = FamilyGapBackend::default();
    let lines = LabelWidths::measuring(&mut gap).wrap(text, 200.0, 13.0, "Manrope", 400);
    assert!(lines.len() > 2, "{lines:?}");
    assert_eq!(super::kept(), (0, 1), "one wrap entry, no prefix widths");
    let cached = LabelWidths::cached().wrap(text, 200.0, 13.0, "Manrope", 400);
    assert_eq!(cached, lines);
    let mut uniform = FamilyGapBackend::uniform();
    let again = LabelWidths::measuring(&mut uniform).wrap(text, 200.0, 13.0, "Manrope", 400);
    assert_eq!(again, lines);
    // Another width is another wrap.
    let wider = LabelWidths::measuring(&mut gap).wrap(text, 400.0, 13.0, "Manrope", 400);
    assert!(wider.len() < lines.len());
    assert_eq!(super::kept(), (0, 2));
}

/// A cached pass that misses breaks against the estimate and keeps
/// nothing, so a later paint still measures.
#[test]
fn an_unmeasured_wrap_is_not_kept() {
    reset();
    let lines = LabelWidths::cached().wrap("a b c d e f g h", 30.0, 13.0, "Manrope", 400);
    assert!(lines.len() > 1);
    assert_eq!(super::kept(), (0, 0));
}
