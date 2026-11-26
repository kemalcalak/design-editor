//! Cross-launch UI preferences (`~/.design-editor/ui.json`) — small,
//! non-secret editor chrome choices that should survive a restart.

use de_editor_core::{PencilCursorStyle, PencilSmoothing};

const FILE: &str = "ui.json";

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
struct UiPrefs {
    #[serde(default)]
    pencil_cursor: Option<String>,
    /// Pencil tool smoothing level (#88).
    #[serde(default)]
    pencil_smoothing: Option<String>,
    /// "Live pages": selected browser nodes stream (#324).
    #[serde(default)]
    live_pages: Option<bool>,
    /// View ▸ Pixel grid (#399).
    #[serde(default)]
    pixel_grid: Option<bool>,
    /// View ▸ Snap to pixel grid (#387 / #399).
    #[serde(default)]
    snap_pixel_grid: Option<bool>,
}

/// The preferences this file carries, as the editor holds them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct UiPrefValues {
    /// The AI generation overlay's cursor silhouette (not the Pencil tool).
    pub(crate) pencil_cursor: PencilCursorStyle,
    pub(crate) pencil_smoothing: PencilSmoothing,
    pub(crate) live_pages: bool,
    pub(crate) pixel_grid: bool,
    pub(crate) snap_pixel_grid: bool,
}

/// Saved values; a field missing from the file (or an unknown id) is
/// `None`, so the caller keeps its default.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct LoadedUiPrefs {
    pub(crate) pencil_cursor: Option<PencilCursorStyle>,
    pub(crate) pencil_smoothing: Option<PencilSmoothing>,
    pub(crate) live_pages: Option<bool>,
    pub(crate) pixel_grid: Option<bool>,
    pub(crate) snap_pixel_grid: Option<bool>,
}

pub(crate) fn save_ui_prefs(values: UiPrefValues) {
    crate::test_config_root::guard_user_config();
    let value = UiPrefs {
        pencil_cursor: Some(values.pencil_cursor.id().to_string()),
        pencil_smoothing: Some(values.pencil_smoothing.id().to_string()),
        live_pages: Some(values.live_pages),
        pixel_grid: Some(values.pixel_grid),
        snap_pixel_grid: Some(values.snap_pixel_grid),
    };
    if let Err(err) = de_config_store::write_json(FILE, &value) {
        eprintln!("[ui-prefs] write failed: {err}");
    }
}

/// The saved preferences ([`LoadedUiPrefs`]).
pub(crate) fn load_ui_prefs() -> LoadedUiPrefs {
    crate::test_config_root::guard_user_config();
    let Some(value) = de_config_store::read_json::<UiPrefs>(FILE).ok().flatten() else {
        return LoadedUiPrefs::default();
    };
    LoadedUiPrefs {
        pencil_cursor: value
            .pencil_cursor
            .as_deref()
            .and_then(PencilCursorStyle::from_id),
        pencil_smoothing: value
            .pencil_smoothing
            .as_deref()
            .and_then(PencilSmoothing::from_id),
        live_pages: value.live_pages,
        pixel_grid: value.pixel_grid,
        snap_pixel_grid: value.snap_pixel_grid,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn style_ids_round_trip() {
        for style in PencilCursorStyle::ALL {
            assert_eq!(PencilCursorStyle::from_id(style.id()), Some(style));
        }
        for level in PencilSmoothing::ALL {
            assert_eq!(PencilSmoothing::from_id(level.id()), Some(level));
        }
    }

    #[test]
    fn a_file_from_before_the_pencil_tool_still_reads() {
        let old: UiPrefs = serde_json::from_str(r#"{"pencil_cursor":"chubby"}"#).unwrap();
        assert_eq!(old.pencil_cursor.as_deref(), Some("chubby"));
        assert_eq!(old.pencil_smoothing, None);
        assert_eq!(old.live_pages, None, "live pages keep their default (on)");
        assert_eq!(
            old.pixel_grid, None,
            "the pixel grid keeps its default (on)"
        );
        assert_eq!(old.snap_pixel_grid, None);
    }
}
