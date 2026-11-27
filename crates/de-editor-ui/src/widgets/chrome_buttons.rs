//! Label-sized action buttons (#441): the width of a button whose box is its
//! label plus padding — choose file, Download, Dismiss, Delete, Rename
//! layers Apply / Cancel, Replace all, the HTML-import toggle — measured in
//! the family and weight the label is painted in, shared by paint and the
//! backend-less hit-tests.

use crate::widgets::label_widths::LabelWidths;
use crate::RenderBackend;

/// Weight every label-sized action button paints its label at.
pub(crate) const BUTTON_LABEL_WEIGHT: u16 = 500;
/// Space either side of a button's label.
const BUTTON_LABEL_PAD: f32 = 12.0;

/// Width of a label-sized action button (choose file, Download, Dismiss,
/// Apply, …): its label as painted — chrome family, [`BUTTON_LABEL_WEIGHT`]
/// — plus [`BUTTON_LABEL_PAD`] either side.
///
/// Backend-less, so hit-tests can call it: it reads the label from the
/// shared [`LabelWidths`] cache, which paint fills first through
/// [`measure_button_labels`]. Only a label nothing has painted yet falls back
/// to the cache's estimate.
pub(crate) fn fit_button_width(label: &str, font_size: f32) -> f32 {
    LabelWidths::cached().chrome_weighted(label, font_size, BUTTON_LABEL_WEIGHT)
        + BUTTON_LABEL_PAD * 2.0
}

/// Measure button `labels` at `font_size` into the shared label cache in the
/// family and weight they are painted in, so [`fit_button_width`] — and
/// every hit-test built on it — resolves the painted box. Paint calls this
/// before it lays its buttons out.
pub(crate) fn measure_button_labels(
    backend: &mut dyn RenderBackend,
    font_size: f32,
    labels: &[&str],
) {
    let mut widths = LabelWidths::measuring(backend);
    for label in labels {
        widths.chrome_weighted(label, font_size, BUTTON_LABEL_WEIGHT);
    }
}
