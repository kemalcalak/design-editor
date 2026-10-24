use super::*;
use jian_core::layout::measure::StyledRun;

/// Resolves to the bundled Roboto fallback, the same on every machine.
const FAMILY: &str = "De Test Missing Family";
const TEXT: &str = "Where margin goes";

fn run(text: &str, weight: u16) -> StyledRun<'_> {
    StyledRun {
        text,
        font_family: Some(FAMILY),
        font_size: 40.0,
        font_weight: weight,
        font_style: FontStyleKind::Normal,
        letter_spacing: 0.0,
    }
}

fn measure(
    backend: &PainterWrapMeasure,
    runs: &[StyledRun<'_>],
    max: f32,
    lh: f32,
) -> MeasureResult {
    backend.measure(&MeasureRequest {
        runs,
        line_height: lh,
        max_width: Some(max),
    })
}

#[cfg_attr(
    target_os = "windows",
    ignore = "WINDOWS_SKIA_DIRECTWRITE_TEXT_MEASURE_ABORT: SkiaMeasure paragraph layout aborts in Windows CI"
)]
#[test]
fn wraps_where_the_painter_does_at_the_exact_width() {
    let backend = PainterWrapMeasure::new();
    for weight in [400, 600] {
        // The painter's advance for the line (synthetic bold included).
        let natural = backend
            .resolver
            .measure_text(TEXT, 40.0, Some(FAMILY), weight, false);
        let fits = measure(&backend, &[run(TEXT, weight)], natural, 1.5);
        assert_eq!(fits.line_count, 1, "weight {weight} at its own width");
        let short = measure(&backend, &[run(TEXT, weight)], natural - 0.5, 1.5);
        assert_eq!(short.line_count, 2, "weight {weight} half a px short");
        assert_eq!(short.height, 2.0 * 40.0 * 1.5);
        assert!(short.width <= natural - 0.5);
    }
}

#[cfg_attr(
    target_os = "windows",
    ignore = "WINDOWS_SKIA_DIRECTWRITE_TEXT_MEASURE_ABORT: SkiaMeasure paragraph layout aborts in Windows CI"
)]
#[test]
fn runs_in_one_style_wrap_as_one_text() {
    let backend = PainterWrapMeasure::new();
    let natural = backend
        .resolver
        .measure_text(TEXT, 40.0, Some(FAMILY), 400, false);
    let split = [run("Where margin", 400), run(" goes", 400)];
    assert_eq!(measure(&backend, &split, natural, 0.0).line_count, 1);
    assert_eq!(measure(&backend, &split, natural - 0.5, 0.0).line_count, 2);
}

#[cfg_attr(
    target_os = "windows",
    ignore = "WINDOWS_SKIA_DIRECTWRITE_TEXT_MEASURE_ABORT: SkiaMeasure paragraph layout aborts in Windows CI"
)]
#[test]
fn mixed_styles_and_natural_extent_keep_the_shaper() {
    let backend = PainterWrapMeasure::new();
    let mixed = [run("Where margin", 700), run(" goes", 400)];
    let ours = measure(&backend, &mixed, 200.0, 0.0);
    let shaper = backend.paragraph.measure(&MeasureRequest {
        runs: &mixed,
        line_height: 0.0,
        max_width: Some(200.0),
    });
    assert_eq!(ours.line_count, shaper.line_count);
    assert_eq!(ours.height, shaper.height);

    let single = [run(TEXT, 400)];
    let request = MeasureRequest {
        runs: &single,
        line_height: 0.0,
        max_width: None,
    };
    assert_eq!(
        backend.measure(&request).width,
        backend.paragraph.measure(&request).width
    );
}
