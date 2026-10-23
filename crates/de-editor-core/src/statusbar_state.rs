//! State-layer enum for the floating zoom status bar's controls.
//!
//! `StatusBarButton` is the hit-test result of `StatusBar::hit_test`
//! and the value stored on `EditorUiState.statusbar_hover` for the
//! per-control `theme.button_hover` wash. It lives in `de-editor-core`
//! (not the widget crate) so the state struct can hold it while the
//! crate stays wasm32-clean — same discipline as `chrome_button_state`.

/// Which status-bar control the cursor is over / clicked. The pill
/// shows `[- N% +]`; the percentage frames the content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusBarButton {
    /// The percentage label — frames the page content within the viewport.
    ZoomToFit,
    /// Minus — zooms the canvas out one step.
    ZoomOut,
    /// Plus — zooms the canvas in one step.
    ZoomIn,
}
