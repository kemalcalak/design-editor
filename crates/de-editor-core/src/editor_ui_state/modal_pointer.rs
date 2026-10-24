//! Which surface owns the pointer while a modal dialog is up (#86).
//!
//! A scrim-backed dialog owns every pointer event: hover, press and the
//! cursor shape must not reach anything behind it. The hosts' cursor
//! ladders run the surfaces in paint order, and Home paints under the
//! dialogs it opens but used to take the hover before the modal tier ever
//! ran — the cards behind the scrim lit up. Every routing site asks this
//! one predicate instead of listing dialogs itself, so a new dialog only
//! has to be added here.

use super::EditorUiState;

impl EditorUiState {
    /// Whether a modal dialog (one painted over a full-window scrim) is on
    /// screen. Popovers and dropdowns are not modal: they close on an
    /// outside press and leave hover below them alone.
    pub fn pointer_modal_open(&self) -> bool {
        self.agent_settings_open
            || self.export_dialog_open
            || self.figma_import_open
            || self.missing_fonts_modal_open
            || self.update_modal_visible()
    }

    /// Drop the hover state of the surfaces that sit behind a modal, so
    /// nothing keeps the highlight it had when the dialog opened. Returns
    /// whether anything changed (the caller's repaint signal).
    pub fn clear_hover_behind_modal(&mut self) -> bool {
        let changed = self.home.hover.is_some() || self.window_control_hover;
        self.home.hover = None;
        self.window_control_hover = false;
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_scrim_dialog_owns_the_pointer() {
        let mut ui = EditorUiState::default();
        assert!(!ui.pointer_modal_open());
        ui.agent_settings_open = true;
        assert!(ui.pointer_modal_open());
        ui.agent_settings_open = false;
        ui.export_dialog_open = true;
        assert!(ui.pointer_modal_open());
        ui.export_dialog_open = false;
        ui.figma_import_open = true;
        assert!(ui.pointer_modal_open());
        ui.figma_import_open = false;
        ui.missing_fonts_modal_open = true;
        assert!(ui.pointer_modal_open());
    }

    #[test]
    fn popovers_are_not_modal() {
        let mut ui = EditorUiState::default();
        ui.home.mcp_popover_open = true;
        ui.locale_picker.open = true;
        ui.file_menu_open = true;
        assert!(!ui.pointer_modal_open());
    }

    #[test]
    fn clearing_drops_the_hover_behind_the_scrim() {
        let mut ui = EditorUiState::default();
        ui.home.hover = Some(crate::HomeHit::NavSettings);
        ui.window_control_hover = true;
        assert!(ui.clear_hover_behind_modal());
        assert!(ui.home.hover.is_none() && !ui.window_control_hover);
        assert!(!ui.clear_hover_behind_modal());
    }
}
