//! Hover tooltips for the floating chrome's icon buttons (the file pill
//! and the canvas action group).
//!
//! An icon is not self-explanatory: a download glyph is the export menu,
//! `</>` is the Code view. Resting on a button for a moment names it.
//!
//! Three rules make this feel like a desktop tooltip rather than a
//! flicker of labels:
//!
//! 1. **Dwell.** Nothing appears until the cursor has rested on the
//!    chrome for [`TOOLTIP_DWELL_MS`], so sweeping across it on the way
//!    somewhere else shows nothing. The clock lives on
//!    `EditorUiState::chrome_hover_since_ms` and starts on entry, not per
//!    button — once the first tooltip has earned its way on screen,
//!    moving along the group swaps labels immediately.
//! 2. **A scheduled repaint.** The dwell expiring is not an input event,
//!    so [`next_deadline_ms`] reports the due instant to the hosts'
//!    animation scheduler.
//! 3. **Suppression.** A button whose menu is open is already answering
//!    the question, and a labelled button (Export) says it already.

use de_editor_core::editor_ui_state::EditorUiState;
use de_editor_core::{ChromeButton, EditorState};

use crate::widgets::canvas_actions::CanvasActions;
use crate::widgets::editor_state_ext::{theme_for, translate};
use crate::widgets::file_pill::FilePill;
use crate::widgets::host_canvas_geometry::chrome_layout;
use crate::widgets::tooltip::{self, TooltipPlacement};
use crate::widgets::PaintCx;
use crate::Rect;

/// How long the cursor must rest on the chrome before the first tooltip
/// appears.
pub const TOOLTIP_DWELL_MS: u64 = 450;

/// What a chrome button's tooltip says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChromeTooltip {
    /// What the button is — never how to use it.
    pub label: &'static str,
    /// Its keyboard shortcut, when one really exists in this build.
    pub shortcut: Option<&'static str>,
}

/// Whether this build binds the desktop-only `zoom_modifier` shortcuts
/// (`de-host-desktop`'s keyboard dispatch). The browser host ships none
/// of them, so promising `⌘P` there would be a lie.
const DESKTOP_SHORTCUTS: bool = !cfg!(target_arch = "wasm32");

/// The panel toggle's chord (#86), bound by both hosts' keyboard dispatch.
pub const PANEL_SHORTCUT: &str = "⌘\\";

/// The label + shortcut for `button`, resolved against the live UI state
/// so toggles name the action they will perform. An empty label means
/// the button needs no tooltip.
pub fn tooltip_for(ui: &EditorUiState, button: ChromeButton) -> ChromeTooltip {
    let (label, shortcut) = match button {
        ChromeButton::Home => (translate(ui, "fileMenu.home"), None),
        ChromeButton::ToggleFileMenu => (translate(ui, "tooltip.topbar.file"), None),
        // de-host-desktop/src/keyboard_input.rs — Cmd+S saves.
        ChromeButton::Save => (
            translate(ui, "fileMenu.save"),
            DESKTOP_SHORTCUTS.then_some("⌘S"),
        ),
        // de-host-desktop/src/keyboard_input.rs — Cmd+Shift+C.
        ChromeButton::ToggleCodeView => (
            translate(ui, "rightPanel.code"),
            DESKTOP_SHORTCUTS.then_some("⌘⇧C"),
        ),
        ChromeButton::TogglePreview => (
            if ui.preview.mode {
                translate(ui, "tooltip.topbar.exitPreview")
            } else {
                translate(ui, "tooltip.topbar.preview")
            },
            // de-host-desktop/src/keyboard_input.rs — `"p" =>
            // toggle_preview_with_cached_viewport()`.
            DESKTOP_SHORTCUTS.then_some("⌘P"),
        ),
        ChromeButton::OpenExportMenu => (translate(ui, "export.title"), None),
        // Same strings as File ▸ Show / Hide panel; both hosts bind ⌘\.
        ChromeButton::TogglePanel => (
            if ui.sidebar_open {
                translate(ui, "topbar.hideLayers")
            } else {
                translate(ui, "topbar.showLayers")
            },
            Some(PANEL_SHORTCUT),
        ),
        // These buttons say what they are in their own text.
        ChromeButton::OpenExportDialog | ChromeButton::OpenAgentSettings => ("", None),
    };
    ChromeTooltip { label, shortcut }
}

/// Whether `button`'s own surface is already open. Its dropdown says more
/// than a tooltip would, and the tooltip would paint over it.
pub fn suppressed(ui: &EditorUiState, button: ChromeButton) -> bool {
    match button {
        ChromeButton::ToggleFileMenu => ui.file_menu_open,
        ChromeButton::OpenExportMenu => ui.export_quick_menu_open,
        ChromeButton::OpenExportDialog | ChromeButton::OpenAgentSettings => true,
        ChromeButton::Home
        | ChromeButton::Save
        | ChromeButton::ToggleCodeView
        | ChromeButton::TogglePreview
        | ChromeButton::TogglePanel => false,
    }
}

/// Which button's tooltip should be on screen at `now_ms`, if any.
pub fn visible_button(ui: &EditorUiState, now_ms: u64) -> Option<ChromeButton> {
    let button = ui.chrome_button_hover?;
    if suppressed(ui, button) {
        return None;
    }
    let due = ui.chrome_hover_since_ms?.saturating_add(TOOLTIP_DWELL_MS);
    (now_ms >= due).then_some(button)
}

/// The instant a pending tooltip becomes due, for the hosts' animation
/// scheduler. `None` once it is showing (or was never coming).
pub fn next_deadline_ms(ui: &EditorUiState, now_ms: u64) -> Option<u64> {
    let button = ui.chrome_button_hover?;
    if suppressed(ui, button) {
        return None;
    }
    let due = ui.chrome_hover_since_ms?.saturating_add(TOOLTIP_DWELL_MS);
    (now_ms < due).then_some(due)
}

/// The rect `button` occupies this frame, for anchoring.
pub fn button_anchor(
    state: &EditorState,
    viewport_w: f32,
    viewport_h: f32,
    button: ChromeButton,
) -> Option<Rect> {
    let chrome = chrome_layout(state, viewport_w, viewport_h);
    if button == ChromeButton::TogglePanel {
        return chrome.panel_toggle;
    }
    let ui = &state.editor_ui;
    chrome
        .file_pill
        .and_then(|layout| FilePill::for_editor_ui(ui).button_rect(&layout, button))
        .or_else(|| {
            chrome
                .actions
                .and_then(|layout| CanvasActions::for_editor_ui(ui).button_rect(&layout, button))
        })
}

/// Paint the due tooltip, if any, under its button. Hosts call this after
/// the chrome and the rails (so it can hang over the canvas) but before
/// every dropdown and modal.
///
/// Returns the painted rect, for tests.
pub fn paint_chrome_tooltip(
    cx: &mut PaintCx<'_>,
    state: &EditorState,
    viewport_w: f32,
    viewport_h: f32,
    now_ms: u64,
) -> Option<Rect> {
    let ui = &state.editor_ui;
    let button = visible_button(ui, now_ms)?;
    let ChromeTooltip { label, shortcut } = tooltip_for(ui, button);
    if label.is_empty() {
        return None;
    }
    let anchor = button_anchor(state, viewport_w, viewport_h, button)?;
    Some(tooltip::paint_tooltip(
        cx,
        &theme_for(ui),
        anchor,
        label,
        shortcut,
        TooltipPlacement::Below,
        Some(tooltip::viewport_clamp(viewport_w)),
    ))
}

/// Name the left rail's icon-only tab under the cursor (#30). The tab on
/// show already reads its label, so only the others get a tooltip, and
/// it appears at once — the row is a strip of bare glyphs, not chrome
/// one crosses on the way somewhere else. Hosts call this in the same
/// pass as [`paint_chrome_tooltip`].
pub fn paint_rail_tab_tooltip(
    cx: &mut PaintCx<'_>,
    state: &EditorState,
    viewport_h: f32,
) -> Option<Rect> {
    use crate::widgets::slides_panel_flow::{body_tab, tab_row, tab_row_desc};
    let ui = &state.editor_ui;
    if !ui.sidebar_open || crate::widgets::host_canvas_geometry::workspace_docked(state) {
        return None;
    }
    let tab = ui.slides_panel.hover?.tab()?;
    if tab == body_tab(state) {
        return None;
    }
    let panel = crate::widgets::host_canvas_geometry::layer_panel_rect(state, viewport_h);
    let anchor = tab_row(state, panel)?.rect_of(tab);
    if anchor.size.x <= 0.0 {
        return None;
    }
    Some(tooltip::paint_tooltip(
        cx,
        &theme_for(ui),
        anchor,
        tab_row_desc(state).label(tab),
        None,
        TooltipPlacement::Below,
        None,
    ))
}

/// Every button the chrome can hover, for exhaustive coverage checks.
pub const ALL_CHROME_BUTTONS: [ChromeButton; 9] = [
    ChromeButton::Home,
    ChromeButton::ToggleFileMenu,
    ChromeButton::Save,
    ChromeButton::ToggleCodeView,
    ChromeButton::TogglePreview,
    ChromeButton::OpenExportMenu,
    ChromeButton::OpenExportDialog,
    ChromeButton::OpenAgentSettings,
    ChromeButton::TogglePanel,
];
