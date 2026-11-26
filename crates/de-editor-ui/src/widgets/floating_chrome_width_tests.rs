//! The file pill and the canvas action group are hit-tested at the size
//! they are painted at, in every locale (#440).

use crate::widgets::canvas_actions::CanvasActions;
use crate::widgets::chrome_hit::ChromeHit;
use crate::widgets::file_pill::FilePill;
use crate::widgets::host_canvas_geometry::{chrome_hit_at, chrome_layout, measured_chrome_layout};
use crate::widgets::label_fit_test_support::{assert_centred, assert_inside, fresh_backend, run};
use crate::widgets::label_widths::LabelWidths;
use crate::widgets::text_metrics::measure_chrome_weighted;
use crate::widgets::PaintCx;
use crate::{Point2D, Rect};
use de_editor_core::{EditorState, Locale};

const VIEWPORT: (f32, f32) = (1600.0, 900.0);

fn state(locale: Locale, dirty: bool) -> EditorState {
    let mut state = EditorState::default();
    state.editor_ui.locale = locale;
    state.editor_ui.file_name_display = Some("pricing page.de".to_string());
    state.editor_ui.document_dirty = dirty;
    state
}

fn mid_y(rect: Rect) -> f32 {
    rect.origin.y + rect.size.y / 2.0
}

#[test]
fn paint_and_hit_test_agree_in_every_locale() {
    for locale in Locale::ALL {
        for dirty in [false, true] {
            let state = state(locale, dirty);
            let ui = &state.editor_ui;
            let mut backend = fresh_backend();
            let (w, h) = VIEWPORT;
            let measured = measured_chrome_layout(&state, w, h, &mut backend);
            assert_eq!(chrome_layout(&state, w, h), measured, "{locale:?}");
            let pill_layout = measured.file_pill.expect("the pill fits");
            let actions_layout = measured.actions.expect("the group fits");
            let pill = FilePill::for_editor_ui(ui);
            let actions = CanvasActions::for_editor_ui(ui);
            {
                let mut cx = PaintCx {
                    backend: &mut backend,
                };
                pill.paint_layout(&mut cx, &pill_layout);
                actions.paint_layout(&mut cx, &actions_layout);
            }
            let what = format!("{locale:?} dirty={dirty}");

            let name = run(&backend, &pill.file_name, &what);
            assert_inside(name, pill_layout.name, 4.0, &what);
            let save = pill_layout.save.expect("not embedded");
            assert_inside(run(&backend, pill.save_label(), &what), save, 4.0, &what);
            assert!(!actions_layout.status_compact, "{what}");
            let status = run(&backend, &actions.status_text, &what);
            assert_inside(status, actions_layout.status, 9.0, &what);
            let export = run(&backend, actions.label_export, &what);
            assert_inside(export, actions_layout.export, 0.0, &what);
            assert_centred(export, actions_layout.export, &what);

            // The last pixel of each labelled box is still that control.
            for (rect, hit) in [
                (pill_layout.name, ChromeHit::ToggleFileMenu),
                (save, ChromeHit::Save),
                (actions_layout.status, ChromeHit::OpenAgentSettings),
                (actions_layout.export, ChromeHit::OpenExportDialog),
            ] {
                let right = Point2D::new(rect.origin.x + rect.size.x - 0.5, mid_y(rect));
                assert_eq!(chrome_hit_at(&state, w, h, right), Some(hit), "{what}");
            }
        }
    }
}

/// The boxes follow the painted face: a family wider than the estimate
/// widens them, so no slack is needed to hold the label.
#[test]
fn the_boxes_are_exactly_as_wide_as_the_painted_labels() {
    let state = state(Locale::EnUs, true);
    let ui = &state.editor_ui;
    let mut backend = fresh_backend();
    let pill = FilePill::for_editor_ui(ui);
    let layout = pill.layout_at(Point2D::ZERO, &mut LabelWidths::measuring(&mut backend));
    let name_w = measure_chrome_weighted(&mut backend, &pill.file_name, 13.0, 600);
    assert_eq!(layout.name.size.x, name_w + 8.0);
    let save_w = measure_chrome_weighted(&mut backend, pill.save_label(), 12.0, 500);
    // The Edited label leads with its dot.
    assert_eq!(layout.save.unwrap().size.x, 6.0 + 5.0 + save_w + 8.0);

    let actions = CanvasActions::for_editor_ui(ui);
    let wide = actions.width(false, &mut LabelWidths::measuring(&mut backend));
    crate::widgets::label_widths::reset();
    let narrow = actions.width(
        false,
        &mut LabelWidths::measuring(
            &mut crate::widgets::test_family_gap_backend::FamilyGapBackend::uniform(),
        ),
    );
    assert!(wide > narrow, "{wide} vs {narrow}");
}

/// A long file name stops growing the pill at its cap and is ellipsized
/// inside it, the hit box still matching the paint.
#[test]
fn a_long_name_is_capped_and_fitted() {
    let mut state = state(Locale::EnUs, false);
    state.editor_ui.file_name_display =
        Some("an extremely long document name that cannot fit the pill.de".to_string());
    let mut backend = fresh_backend();
    let (w, h) = VIEWPORT;
    let layout = measured_chrome_layout(&state, w, h, &mut backend)
        .file_pill
        .unwrap();
    assert_eq!(layout.name.size.x, 220.0 + 8.0);
    let pill = FilePill::for_editor_ui(&state.editor_ui);
    pill.paint_layout(
        &mut PaintCx {
            backend: &mut backend,
        },
        &layout,
    );
    let name = backend
        .runs
        .iter()
        .find(|run| run.text.ends_with('…'))
        .expect("ellipsized");
    assert_inside(name, layout.name, 4.0, "long name");
}
