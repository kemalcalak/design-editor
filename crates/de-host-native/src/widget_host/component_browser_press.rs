//! Components tab press dispatch.
//!
//! The component browser fills the rail's body while its tab is on show
//! (#30) and owns clicks on that rect ahead of every lower layer. Hit
//! kinds: kit actions, search focus, category pill, insert.

use de_editor_ui::widgets::{ComponentBrowserHit, ComponentBrowserPanel};
use de_editor_ui::Point2D;

use super::WidgetHostNative;

impl WidgetHostNative {
    /// Dispatch a press inside the rail's Components tab.
    ///
    /// Returns `true` when consumed — a search focus, a category-pill
    /// swap, a queued insert request, or a swallowed click.
    pub(in crate::widget_host) fn dispatch_component_browser_press(
        &mut self,
        x: f32,
        y: f32,
        viewport_width: f32,
        viewport_height: f32,
    ) -> bool {
        let Some(panel_rect) = self.component_browser_panel_rect(viewport_width, viewport_height)
        else {
            return false;
        };
        let point = Point2D::new(x, y);
        let Some((hit, pressed_button)) = ComponentBrowserPanel::for_editor(&self.editor_state)
            .and_then(|p| {
                Some((
                    p.hit_test(panel_rect, point)?,
                    p.hover_at(panel_rect, point),
                ))
            })
        else {
            return false;
        };
        if let Some(button) = pressed_button {
            self.editor_state.editor_ui.pressed_button =
                Some(de_editor_core::ButtonPressTarget::ComponentBrowser(button));
        }
        match hit {
            ComponentBrowserHit::ExportKit => {
                // The desktop host drains this — it owns the native
                // save dialog (TS `handleExport`).
                self.editor_state.editor_ui.component_browser_kit_request =
                    Some(de_editor_core::KitIoRequest::Export);
            }
            ComponentBrowserHit::ImportKit => {
                self.editor_state.editor_ui.component_browser_kit_request =
                    Some(de_editor_core::KitIoRequest::Import);
            }
            ComponentBrowserHit::ToggleKitPicker => {
                let ui = &mut self.editor_state.editor_ui;
                ui.component_browser_kit_picker_open = !ui.component_browser_kit_picker_open;
            }
            ComponentBrowserHit::SelectKitFilter(kit_id) => {
                let ui = &mut self.editor_state.editor_ui;
                ui.component_browser_kit_id = kit_id;
                ui.component_browser_kit_picker_open = false;
            }
            ComponentBrowserHit::RequestDeleteKit(kit_id) => {
                self.editor_state
                    .editor_ui
                    .component_browser_confirm_delete_kit = Some(kit_id);
            }
            ComponentBrowserHit::ConfirmDeleteKit(kit_id) => {
                // `remove_kit` clears the confirm state and raises the
                // `ui_kits_changed` persistence flag the desktop host
                // drains into `uikits.json`.
                let _ = self.editor_state.remove_kit(&kit_id);
            }
            ComponentBrowserHit::CancelDeleteKit => {
                self.editor_state
                    .editor_ui
                    .component_browser_confirm_delete_kit = None;
            }
            ComponentBrowserHit::FocusSearch => {
                // Every other input lets go first, then the search box
                // takes the keyboard.
                self.blur_text_inputs_on_blank_press();
                let ui = &mut self.editor_state.editor_ui;
                ui.component_browser_search_focused = true;
                ui.component_browser_select_all = false;
            }
            ComponentBrowserHit::SelectCategory(cat) => {
                self.editor_state.editor_ui.component_browser_category = cat;
            }
            ComponentBrowserHit::InsertComponent(kit_id, comp_id) => {
                // The desktop host drains this against the viewport
                // centre — it owns the viewport metrics needed to
                // compute the document-space drop point.
                self.editor_state.editor_ui.component_browser_pending_insert =
                    Some((kit_id, comp_id));
            }
            ComponentBrowserHit::Inside => {
                // Blank press on the tab's chrome — blur chrome inputs,
                // the search box included.
                self.blur_text_inputs_on_blank_press();
            }
        }
        self.mark_dirty();
        true
    }
}
