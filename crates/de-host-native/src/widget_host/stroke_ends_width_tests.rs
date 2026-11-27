//! The Stroke section's Start / End / Miter labels and end names (#389)
//! fit their controls in all 15 locales on the native backend's real
//! text measurement in the bundled chrome face, the way the rows fit them
//! when they paint (no estimate).

use crate::backend::{NativeBackend, NativeFrameBackend};
use de_editor_core::editor_ui_state::EditorUiState;
use de_editor_core::Locale;
use de_editor_ui::widgets::property_panel_sections::PropertyLabels;
use de_editor_ui::widgets::property_panel_stroke_ends::cut_end_labels;

#[test]
fn every_end_label_fits_in_every_locale_on_real_metrics() {
    let mut backend = NativeBackend::with_dpi(1.0);
    let mut surface = skia_safe::surfaces::raster_n32_premul((8, 8)).expect("surface");
    let mut frame = NativeFrameBackend::new(&mut backend, surface.canvas());
    let mut cut = Vec::new();
    for locale in Locale::ALL {
        let ui = EditorUiState {
            locale,
            ..Default::default()
        };
        let labels = PropertyLabels::for_editor_ui(&ui);
        for label in cut_end_labels(&mut frame, &labels) {
            cut.push(format!("{locale:?}: {label:?}"));
        }
    }
    assert!(cut.is_empty(), "cut on screen:\n{}", cut.join("\n"));
}
