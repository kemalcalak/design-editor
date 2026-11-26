//! The layer context menu is sized from the native backend's real text
//! measurement in the bundled chrome face (#435): the host's hover and
//! press hit-tests use the width paint draws, already before the first
//! paint after the opening right press.

use super::WidgetHostNative;
use crate::backend::{NativeBackend, NativeFrameBackend};
use de_editor_core::editor_ui_state::{LayerContextMenuState, LayerContextTarget};
use de_editor_core::{EditorState, Locale, NodeId};
use de_editor_ui::widgets::layer_context_menu::{LayerContextMenu, LAYER_CONTEXT_MENU_WIDTH};
use de_editor_ui::widgets::press_flow::{press_open_layer_context_menu, OpenLayerMenuPress};
use de_editor_ui::widgets::text_metrics::{measure_chrome, measure_in_family};
use de_editor_ui::{Point2D, Rect};

const W: f32 = 1200.0;
const H: f32 = 800.0;

fn paint(host: &mut WidgetHostNative, backend: &mut NativeBackend) {
    let mut surface =
        skia_safe::surfaces::raster_n32_premul((W as i32, H as i32)).expect("raster surface");
    let mut frame = NativeFrameBackend::new(backend, surface.canvas());
    host.paint(&mut frame, W, H);
}

/// Run `f` with a frame backend over `backend` (measurement only).
fn with_frame<R>(backend: &mut NativeBackend, f: impl FnOnce(&mut NativeFrameBackend) -> R) -> R {
    let mut surface = skia_safe::surfaces::raster_n32_premul((8, 8)).expect("surface");
    let mut frame = NativeFrameBackend::new(backend, surface.canvas());
    f(&mut frame)
}

fn open_menu(host: &mut WidgetHostNative, locale: Locale) {
    *host.editor_state_mut() = EditorState::sample();
    let state = host.editor_state_mut();
    state.editor_ui.locale = locale;
    state.set_single_selection(NodeId::new("n11"));
    state.editor_ui.layer_context_menu = Some(LayerContextMenuState {
        target: LayerContextTarget::Layer(NodeId::new("n11")),
        anchor_x: 220.0,
        anchor_y: 120.0,
        menu: Default::default(),
    });
    host.mark_paint_dirty_for_test();
}

fn open_menu_widget(host: &WidgetHostNative) -> LayerContextMenu {
    let state = host.editor_state().editor_ui.layer_context_menu.clone();
    LayerContextMenu::for_state(host.editor_state(), state.expect("menu open"))
}

fn hover(host: &WidgetHostNative) -> Option<usize> {
    host.editor_state()
        .editor_ui
        .layer_context_menu
        .as_ref()
        .expect("menu open")
        .menu
        .hover
}

/// The last pixel inside the menu's right edge and the first one past it,
/// on the first row.
fn edge_points(rect: Rect) -> (Point2D, Point2D) {
    let right = rect.origin.x + rect.size.x;
    let row_y = rect.origin.y + 6.0 + 16.0;
    (
        Point2D::new(right - 1.0, row_y),
        Point2D::new(right + 1.0, row_y),
    )
}

fn assert_hit_tests_end_at(host: &mut WidgetHostNative, rect: Rect, locale: Locale) {
    let (inside, outside) = edge_points(rect);
    host.apply_cursor_move(inside.x, inside.y);
    assert_eq!(hover(host), Some(0), "{locale:?}: last px is a row");
    host.apply_cursor_move(outside.x, outside.y);
    assert_eq!(hover(host), None, "{locale:?}: past the edge is not");
    assert!(matches!(
        press_open_layer_context_menu(host.editor_state(), inside),
        Some(OpenLayerMenuPress::Action { .. })
    ));
    assert_eq!(
        press_open_layer_context_menu(host.editor_state(), outside),
        Some(OpenLayerMenuPress::Outside)
    );
}

#[test]
fn hit_tests_use_the_measured_width_before_and_after_paint() {
    let _guard = crate::font_registry_test_support::lock();
    crate::font_registry_test_support::register_bundled_fonts();
    let mut backend = NativeBackend::with_dpi(1.0);
    // The chrome face really is the bundled Manrope, not a fallback.
    let (manrope, fallback) = with_frame(&mut backend, |frame| {
        (
            measure_chrome(frame, "Component from copies", 13.0),
            measure_in_family(frame, "Component from copies", 13.0, "No Such Family 435"),
        )
    });
    assert_ne!(manrope, fallback, "Manrope resolved");

    let mut host = WidgetHostNative::new();
    host.last_viewport_w = W;
    host.last_viewport_h = H;
    let mut grown = 0;
    for locale in Locale::ALL {
        open_menu(&mut host, locale);
        let expected = with_frame(&mut backend, |frame| {
            open_menu_widget(&host).required_width(frame)
        });
        if expected > LAYER_CONTEXT_MENU_WIDTH {
            grown += 1;
        }

        // Before any paint (cursor moves queued behind the opening press).
        let rect = host.layer_context_menu_rect().expect("menu open");
        assert_eq!(rect.size.x, expected, "{locale:?}: before paint");
        assert_hit_tests_end_at(&mut host, rect, locale);

        // After a frame painted it, the same box.
        paint(&mut host, &mut backend);
        let rect = host.layer_context_menu_rect().expect("menu open");
        assert_eq!(rect.size.x, expected, "{locale:?}: after paint");
        assert_hit_tests_end_at(&mut host, rect, locale);
    }
    assert!(grown > 0, "some locale's labels need more than 168 px");
}
