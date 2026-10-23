//! What a press on the floating editor chrome resolved to.
//!
//! The editor has no title bar (#30): the file pill floats at the canvas
//! top-left and the action group at its top-right. Both widgets hit-test
//! into this one enum so the shared press flow and the hover state treat
//! them as a single chrome family.

/// A hit on the file pill or the canvas action group.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChromeHit {
    /// File pill house icon — return to the Home surface.
    Home,
    /// File pill name — toggle the file menu.
    ToggleFileMenu,
    /// File pill save-state label — save the document (Cmd+S).
    Save,
    /// `</>` — flip the right panel between Design and Code.
    ToggleCodeView,
    /// Play / Stop — enter / exit canvas Preview (Play) mode.
    TogglePreview,
    /// Download — toggle the scenario-aware export quick menu.
    OpenExportMenu,
    /// Export — open the export dialog.
    OpenExportDialog,
    /// Agent / MCP status chip — open the Agents & MCP settings.
    OpenAgentSettings,
}

impl ChromeHit {
    /// The state-layer twin stored on `EditorUiState.chrome_button_hover`.
    pub fn button(self) -> de_editor_core::ChromeButton {
        use de_editor_core::ChromeButton as B;
        match self {
            Self::Home => B::Home,
            Self::ToggleFileMenu => B::ToggleFileMenu,
            Self::Save => B::Save,
            Self::ToggleCodeView => B::ToggleCodeView,
            Self::TogglePreview => B::TogglePreview,
            Self::OpenExportMenu => B::OpenExportMenu,
            Self::OpenExportDialog => B::OpenExportDialog,
            Self::OpenAgentSettings => B::OpenAgentSettings,
        }
    }
}
