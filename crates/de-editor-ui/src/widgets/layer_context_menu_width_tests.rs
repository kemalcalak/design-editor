//! The layer / page context menu's width comes from real text measurement
//! (#435): 168 px when every label fits, exactly as wide as the widest
//! label otherwise, and the hit-tests read the width paint measured.

use de_editor_core::editor_ui_state::{LayerContextMenuState, LayerContextTarget};
use de_editor_core::{EditorState, Locale};

use super::similar_tests::{menu_for, offers, state};
use super::{
    LayerContextMenu, MenuHit, LABEL_RIGHT_PAD, LABEL_X, LAYER_CONTEXT_MENU_WIDTH, ROW_FONT,
};
use crate::widgets::label_widths::reset;
use crate::widgets::test_capture_backend::CaptureBackend;
use crate::widgets::test_family_gap_backend::FamilyGapBackend;
use crate::widgets::text_metrics::{measure_chrome, CHROME_FONT_FAMILY};
use crate::widgets::{PaintCx, Widget};
use crate::{Point2D, RenderBackend};

fn page_menu(s: &EditorState) -> LayerContextMenu {
    LayerContextMenu::for_state(
        s,
        LayerContextMenuState {
            target: LayerContextTarget::Page(0),
            anchor_x: 30.0,
            anchor_y: 40.0,
            menu: Default::default(),
        },
    )
}

/// What the menu must measure to: 168, or the widest label's run
/// (label inset + label + right pad) rounded up to a whole pixel.
fn expected_width(menu: &LayerContextMenu, backend: &mut dyn RenderBackend) -> f32 {
    menu.rows
        .iter()
        .map(|row| {
            LABEL_X
                + measure_chrome(
                    backend,
                    de_i18n::translate(menu.locale, row.label_key),
                    ROW_FONT,
                )
                + LABEL_RIGHT_PAD
        })
        .fold(LAYER_CONTEXT_MENU_WIDTH, f32::max)
        .ceil()
}

/// Both menus in `locale`, each measured from an empty slot.
fn menus_in(locale: Locale) -> [LayerContextMenu; 2] {
    let mut s = state();
    s.editor_ui.locale = locale;
    let layer = menu_for(&s, "b");
    assert!(offers(&layer), "the longest layer row set is under test");
    [layer, page_menu(&s)]
}

#[test]
fn the_menu_is_168_where_labels_fit_and_exactly_wide_enough_otherwise() {
    let (mut kept, mut grown) = (0, 0);
    for locale in Locale::ALL {
        for menu in menus_in(locale) {
            reset();
            let expected = expected_width(&menu, &mut CaptureBackend::default());
            let rect = menu.measured_rect(&mut CaptureBackend::default());
            assert_eq!(rect.size.x, expected, "{locale:?}");
            assert_eq!(menu.width(), expected, "{locale:?}: hit-test width");
            assert_eq!(menu.rect(), rect, "{locale:?}: hit-test rect");
            let widest_run = menu
                .rows
                .iter()
                .map(|row| {
                    LABEL_X
                        + measure_chrome(
                            &mut CaptureBackend::default(),
                            de_i18n::translate(locale, row.label_key),
                            ROW_FONT,
                        )
                        + LABEL_RIGHT_PAD
                })
                .fold(0.0, f32::max);
            if widest_run <= LAYER_CONTEXT_MENU_WIDTH {
                assert_eq!(rect.size.x, LAYER_CONTEXT_MENU_WIDTH, "{locale:?}");
                kept += 1;
            } else {
                assert_eq!(rect.size.x, widest_run.ceil(), "{locale:?}");
                grown += 1;
            }
        }
    }
    assert!(
        kept > 0 && grown > 0,
        "both cases covered: {kept} / {grown}"
    );
}

/// Measured in the family the labels are painted in, not the backend's
/// default face: under a backend whose named family is wider than its
/// default, the menu follows the named one.
#[test]
fn the_width_is_measured_in_the_chrome_family() {
    let mut s = state();
    s.editor_ui.locale = Locale::EnUs;
    let menu = menu_for(&s, "b");
    reset();
    let family_aware = expected_width(&menu, &mut FamilyGapBackend::default());
    // The control backend's named family measures like its default face.
    let blind = expected_width(&menu, &mut FamilyGapBackend::uniform());
    assert!(family_aware > blind, "{family_aware} vs {blind}");
    assert_eq!(
        menu.measured_rect(&mut FamilyGapBackend::default()).size.x,
        family_aware
    );
}

/// Paint the way the host does (measure, then paint into that rect) and
/// check every label fits what was painted, in the family it names, and
/// that the hit-tests see the same box.
#[test]
fn paint_and_hit_test_agree_in_every_locale() {
    for locale in Locale::ALL {
        for menu in menus_in(locale) {
            reset();
            let mut backend = FamilyGapBackend::default();
            let rect = menu.measured_rect(&mut backend);
            menu.paint(
                &mut PaintCx {
                    backend: &mut backend,
                },
                rect,
            );
            assert_eq!(backend.runs.len(), menu.rows.len());
            for run in &backend.runs {
                assert_eq!(run.family, CHROME_FONT_FAMILY, "{locale:?}");
                assert!(
                    run.right_edge() <= rect.origin.x + rect.size.x - LABEL_RIGHT_PAD + 0.001,
                    "{locale:?}: {:?} runs to {}, the menu ends at {}",
                    run.text,
                    run.right_edge(),
                    rect.origin.x + rect.size.x
                );
            }
            assert_eq!(menu.rect(), rect, "{locale:?}");
            let right = rect.origin.x + rect.size.x;
            let row_y = rect.origin.y + 6.0 + 16.0;
            assert_eq!(menu.hit(Point2D::new(right - 1.0, row_y)), MenuHit::Row(0));
            assert_eq!(
                menu.hovered_row_at(Point2D::new(right - 1.0, row_y)),
                Some(0)
            );
            assert_eq!(menu.hit(Point2D::new(right + 1.0, row_y)), MenuHit::Outside);
        }
    }
}

/// A repaint measures nothing it measured before; a locale change
/// measures the new labels; menus with different rows keep their widths
/// side by side (no single slot to thrash).
#[test]
fn the_width_is_measured_once_per_label() {
    reset();
    let [layer, page] = menus_in(Locale::EnUs);
    let first = layer.measured_rect(&mut CaptureBackend::default()).size.x;
    // A backend that would measure wider is never asked: same labels.
    let again = layer.measured_rect(&mut FamilyGapBackend::default()).size.x;
    assert_eq!(first, again);
    // Rebuilding the menu (as every frame does) reads the same widths.
    let [rebuilt, _] = menus_in(Locale::EnUs);
    assert_eq!(rebuilt.width(), first);
    // Measuring the page menu leaves the layer menu's width in place.
    let page_width = page.measured_rect(&mut CaptureBackend::default()).size.x;
    assert_eq!(page.width(), page_width);
    assert_eq!(layer.width(), first);
    // A different locale is a different set of labels.
    let [de_layer, _] = menus_in(Locale::De);
    let de_width = de_layer
        .measured_rect(&mut CaptureBackend::default())
        .size
        .x;
    assert_eq!(
        de_width,
        expected_width(&de_layer, &mut CaptureBackend::default())
    );
    assert_eq!(de_layer.width(), de_width);
}
