//! The layer context menu is sized from the native backend's real text
//! measurement (#435): after a frame paints it, the host's hover and press
//! hit-tests use exactly the width that frame measured.

use super::WidgetHostNative;
use crate::backend::{NativeBackend, NativeFrameBackend};
use de_editor_core::editor_ui_state::{LayerContextMenuState, LayerContextTarget};
use de_editor_core::{EditorState, Locale, NodeId};
use de_editor_ui::widgets::layer_context_menu::{
    LayerContextMenu, LABEL_RIGHT_PAD, LABEL_X, LAYER_CONTEXT_MENU_WIDTH, ROW_FONT,
};
use de_editor_ui::widgets::press_flow::{press_open_layer_context_menu, OpenLayerMenuPress};
use de_editor_ui::widgets::text_metrics::measure_chrome;
use de_editor_ui::Point2D;

const W: f32 = 1200.0;
const H: f32 = 800.0;

fn paint(host: &mut WidgetHostNative, backend: &mut NativeBackend) {
    let mut surface =
        skia_safe::surfaces::raster_n32_premul((W as i32, H as i32)).expect("raster surface");
    let mut frame = NativeFrameBackend::new(backend, surface.canvas());
    host.paint(&mut frame, W, H);
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

#[test]
fn hit_tests_use_the_width_the_frame_measured() {
    let mut backend = NativeBackend::with_dpi(1.0);
    let mut host = WidgetHostNative::new();
    host.last_viewport_w = W;
    host.last_viewport_h = H;
    for locale in Locale::ALL {
        open_menu(&mut host, locale);
        paint(&mut host, &mut backend);

        let menu = open_menu_widget(&host);
        let expected = {
            let mut surface = skia_safe::surfaces::raster_n32_premul((8, 8)).expect("surface");
            let mut frame = NativeFrameBackend::new(&mut backend, surface.canvas());
            menu.row_labels()
                .map(|label| {
                    LABEL_X + measure_chrome(&mut frame, label, ROW_FONT) + LABEL_RIGHT_PAD
                })
                .fold(LAYER_CONTEXT_MENU_WIDTH, f32::max)
                .ceil()
        };
        let rect = host.layer_context_menu_rect().expect("menu open");
        assert_eq!(rect.size.x, expected, "{locale:?}");

        let right = rect.origin.x + rect.size.x;
        let row_y = rect.origin.y + 6.0 + 16.0;
        let inside = Point2D::new(right - 1.0, row_y);
        let outside = Point2D::new(right + 1.0, row_y);
        host.apply_cursor_move(inside.x, inside.y);
        assert_eq!(hover(&host), Some(0), "{locale:?}: last px is a row");
        host.apply_cursor_move(outside.x, outside.y);
        assert_eq!(hover(&host), None, "{locale:?}: past the edge is not");
        assert!(matches!(
            press_open_layer_context_menu(host.editor_state(), inside),
            Some(OpenLayerMenuPress::Action { .. })
        ));
        assert_eq!(
            press_open_layer_context_menu(host.editor_state(), outside),
            Some(OpenLayerMenuPress::Outside)
        );
    }
}
