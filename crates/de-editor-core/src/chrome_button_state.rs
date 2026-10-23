//! State-layer mirror of the editor chrome's hit-test enum.
//!
//! `ChromeButton` mirrors `de_editor_ui::widgets::ChromeHit` so the host
//! can record which floating chrome button (the file pill and the
//! canvas action group) the cursor is over on
//! `EditorUiState.chrome_button_hover` without `de-editor-core`
//! depending on the widget crate (keeps the crate wasm32-clean, same
//! discipline as `toolbar_state::ToolbarHover`).

/// Which chrome button the cursor is over. `None` on
/// `EditorUiState.chrome_button_hover` = no hover wash.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChromeButton {
    /// File pill house icon — returns to the Home surface.
    Home,
    /// File pill name — toggles the file menu.
    ToggleFileMenu,
    /// File pill save-state label — saves the document.
    Save,
    /// `</>` — flips the right panel between Design and Code.
    ToggleCodeView,
    /// Play / Stop — enters / exits canvas Preview (Play) mode.
    TogglePreview,
    /// Download — opens the scenario-aware export quick menu.
    OpenExportMenu,
    /// Export — opens the export dialog.
    OpenExportDialog,
}
