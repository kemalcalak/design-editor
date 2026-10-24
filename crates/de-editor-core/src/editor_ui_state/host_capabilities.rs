//! Host capability flags: what the running host can do, as opposed to what
//! the open document contains.
//!
//! A host sets these once at startup. Every whole-state swap (Open, New, a
//! template, a generated deck) builds a fresh [`EditorUiState`] whose
//! defaults are all `false`, so a swap seam that does not carry them back
//! silently hides host-backed controls, for example the missing-font
//! Download action (#161). [`HostCapabilities`] is the one list of these
//! flags; the hosts' swap seams and `preserve_app_preferences` copy it.
//! A new host capability flag on [`EditorUiState`] must be added here; it
//! then survives every swap without touching a caller.

use super::EditorUiState;

/// Snapshot of every host capability flag on [`EditorUiState`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HostCapabilities {
    pub font_import: bool,
    pub font_download: bool,
    pub batch_frame_export: bool,
    pub deck_html_export: bool,
    pub slide_thumbnails: bool,
    pub scene_template_generate: bool,
    pub scene_template_save_current: bool,
    pub style_import_file_picker: bool,
}

impl HostCapabilities {
    /// Read the flags `ui` currently advertises.
    pub fn of(ui: &EditorUiState) -> Self {
        Self {
            font_import: ui.font_import_supported,
            font_download: ui.font_download_supported,
            batch_frame_export: ui.batch_frame_export_supported,
            deck_html_export: ui.deck_html_export_supported,
            slide_thumbnails: ui.slide_thumbnails_supported,
            scene_template_generate: ui.scene_template_generate_supported,
            scene_template_save_current: ui.scene_template_center.save_current_supported,
            style_import_file_picker: ui.style_import_file_picker_supported,
        }
    }

    /// Write these flags onto `ui`, replacing whatever it advertised.
    pub fn apply_to(self, ui: &mut EditorUiState) {
        ui.font_import_supported = self.font_import;
        ui.font_download_supported = self.font_download;
        ui.batch_frame_export_supported = self.batch_frame_export;
        ui.deck_html_export_supported = self.deck_html_export;
        ui.slide_thumbnails_supported = self.slide_thumbnails;
        ui.scene_template_generate_supported = self.scene_template_generate;
        ui.scene_template_center.save_current_supported = self.scene_template_save_current;
        ui.style_import_file_picker_supported = self.style_import_file_picker;
    }
}

impl EditorUiState {
    /// Carry the host capability flags of `previous` (the state being
    /// replaced) onto `self` (the incoming one). The host, not the
    /// document, decides what it can do.
    pub fn carry_host_capabilities_from(&mut self, previous: &EditorUiState) {
        HostCapabilities::of(previous).apply_to(self);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all_on() -> HostCapabilities {
        HostCapabilities {
            font_import: true,
            font_download: true,
            batch_frame_export: true,
            deck_html_export: true,
            slide_thumbnails: true,
            scene_template_generate: true,
            scene_template_save_current: true,
            style_import_file_picker: true,
        }
    }

    #[test]
    fn a_fresh_state_advertises_no_capability() {
        assert_eq!(
            HostCapabilities::of(&EditorUiState::default()),
            HostCapabilities::default()
        );
    }

    #[test]
    fn every_flag_round_trips_through_the_ui_state() {
        let mut ui = EditorUiState::default();
        all_on().apply_to(&mut ui);
        assert_eq!(HostCapabilities::of(&ui), all_on());
        assert!(ui.font_download_supported, "the #161 flag is in the list");
    }

    #[test]
    fn carrying_replaces_what_the_incoming_state_advertised() {
        let mut previous = EditorUiState::default();
        all_on().apply_to(&mut previous);
        let mut next = EditorUiState::default();
        next.carry_host_capabilities_from(&previous);
        assert_eq!(HostCapabilities::of(&next), all_on());

        // And the other way: a host without a capability never gains it
        // from a state built elsewhere.
        let mut back = EditorUiState::default();
        all_on().apply_to(&mut back);
        back.carry_host_capabilities_from(&EditorUiState::default());
        assert_eq!(HostCapabilities::of(&back), HostCapabilities::default());
    }
}
