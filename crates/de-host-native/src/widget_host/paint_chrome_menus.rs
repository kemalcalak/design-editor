//! Floating-chrome paint arms — the file pill and canvas action group, the
//! File menu, the export quick menu, and the hover tooltip.
//!
//! Carved out of `paint.rs` at the 800-line ceiling. The menus and the
//! tooltip all hang off the chrome and paint in the same overlay band, so
//! they sit together; `paint.rs` keeps only the call sites that fix their
//! Z-order.

use super::WidgetHostNative;
use crate::backend::NativeFrameBackend;
use de_editor_ui::widgets::host_canvas_geometry as canvas_geometry;
use de_editor_ui::widgets::host_overlay_geometry as overlay_geometry;
use de_editor_ui::widgets::{
    paint_window_control_dots, CanvasActions, ExportQuickMenu, FilePill, PaintCx, Widget,
};

impl WidgetHostNative {
    /// The file pill and the canvas action group, floating over the canvas
    /// — plus the window-control dots when the left rail is shut and they
    /// sit over the canvas's top-left corner instead of the rail's strip.
    pub(in crate::widget_host) fn paint_floating_chrome(
        &self,
        frame: &mut NativeFrameBackend<'_>,
        viewport_width: f32,
        viewport_height: f32,
    ) {
        let ui = &self.editor_state.editor_ui;
        let chrome =
            canvas_geometry::chrome_layout(&self.editor_state, viewport_width, viewport_height);
        let mut cx = PaintCx {
            backend: &mut *frame,
        };
        if let Some(layout) = chrome.file_pill {
            FilePill::for_editor_ui(ui).paint_layout(&mut cx, &layout);
        }
        if let Some(layout) = chrome.actions {
            CanvasActions::for_editor_ui(ui).paint_layout(&mut cx, &layout);
        }
        if !ui.sidebar_open {
            paint_window_control_dots(
                cx.backend,
                de_editor_ui::widgets::window_strip::window_controls_rect(),
                ui.window_control_hover,
            );
        }
    }

    /// Hover tooltip for whichever chrome button has been dwelt on long
    /// enough. Paints nothing until the dwell elapses, and nothing at all
    /// for a button whose own menu is already open.
    pub(in crate::widget_host) fn paint_chrome_tooltip_overlay(
        &self,
        frame: &mut NativeFrameBackend<'_>,
        viewport_width: f32,
        viewport_height: f32,
    ) {
        let mut cx = PaintCx {
            backend: &mut *frame,
        };
        de_editor_ui::widgets::chrome_tooltip::paint_chrome_tooltip(
            &mut cx,
            &self.editor_state,
            viewport_width,
            viewport_height,
            self.now_ms,
        );
        de_editor_ui::widgets::chrome_tooltip::paint_rail_tab_tooltip(
            &mut cx,
            &self.editor_state,
            viewport_height,
        );
    }

    /// File-menu dropdown — anchored under the file pill.
    pub(in crate::widget_host) fn paint_file_menu_overlay(
        &self,
        frame: &mut NativeFrameBackend<'_>,
        viewport_width: f32,
        viewport_height: f32,
    ) {
        use de_editor_ui::widgets::file_menu::FileMenu;
        let anchor =
            overlay_geometry::file_menu_anchor(&self.editor_state, viewport_width, viewport_height);
        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        // Selected-frame count only names the batch-export row; it leaves
        // row geometry untouched, so the hit-test call sites keep building
        // the menu without it.
        let selected_frames =
            de_editor_core::export_batch::selected_frame_count(&self.editor_state);
        let menu = FileMenu::from_editor_ui(&self.editor_state.editor_ui, now_secs)
            .with_selected_frames(selected_frames);
        let menu_rect = menu.rect_at(anchor);
        let mut cx = PaintCx {
            backend: &mut *frame,
        };
        menu.paint(&mut cx, menu_rect);
    }

    /// Export quick menu — anchored under the action group's Download button.
    pub(in crate::widget_host) fn paint_export_quick_menu_overlay(
        &self,
        frame: &mut NativeFrameBackend<'_>,
        viewport_width: f32,
    ) {
        let menu_rect = self.export_quick_menu_rect(viewport_width);
        let menu = ExportQuickMenu::for_editor_ui(&self.editor_state.editor_ui);
        let mut cx = PaintCx {
            backend: &mut *frame,
        };
        menu.paint(&mut cx, menu_rect);
    }
}
