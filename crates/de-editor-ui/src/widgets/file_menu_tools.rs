//! The File menu's app-level tail (#30): the Import / Undo / Redo rows and
//! the icon footer that took over the retired title bar's buttons.
//!
//! The editor has no title bar any more, so everything that used to sit
//! in it as an icon now lives at the bottom of the File menu: the left
//! panel toggle, Variables, the design system, the Asset Center, theme,
//! language, fullscreen, Git and Settings. They stay icons — they were
//! icons before — and hovering one names it right away, the menu being
//! open already counting as the dwell.
//!
//! A CHILD module of `file_menu`, like its paint sibling, so it can read
//! the menu's private state and constants.

use crate::widgets::editor_state_ext::translate;
use crate::widgets::icons::{draw_icon, Icon};
use crate::widgets::tooltip::{self, TooltipPlacement};
use crate::widgets::PaintCx;
use crate::{Point2D, Rect};
use de_editor_core::editor_ui_state::EditorUiState;

use super::{FileMenu, MENU_WIDTH, PAD_X};

/// Height of the icon footer.
pub(super) const FOOTER_HEIGHT: f32 = 40.0;
const FOOTER_BUTTON: f32 = 26.0;
const FOOTER_ICON: f32 = 15.0;

/// Rows in the tail section: Import, Undo, Redo, Present.
pub(super) const TAIL_ROWS: usize = 4;

/// An app-level action in the File menu's icon footer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileMenuTool {
    ToggleSidebar,
    Variables,
    DesignSystem,
    AssetCenter,
    ToggleTheme,
    Language,
    ToggleFullscreen,
    Git,
    Settings,
}

/// The Git panel needs the desktop git backend; the browser has none.
const GIT_AVAILABLE: bool = !cfg!(target_arch = "wasm32");

/// The footer's tools for this host, left to right. Fullscreen is the
/// container's business inside a VS Code embed, and Git needs a desktop
/// backend.
pub(super) fn footer_tools(ui: &EditorUiState) -> Vec<FileMenuTool> {
    let embedded = ui.embed == de_editor_core::EmbedHost::VsCode;
    [
        Some(FileMenuTool::ToggleSidebar),
        Some(FileMenuTool::Variables),
        Some(FileMenuTool::DesignSystem),
        Some(FileMenuTool::AssetCenter),
        Some(FileMenuTool::ToggleTheme),
        Some(FileMenuTool::Language),
        (!embedded).then_some(FileMenuTool::ToggleFullscreen),
        GIT_AVAILABLE.then_some(FileMenuTool::Git),
        Some(FileMenuTool::Settings),
    ]
    .into_iter()
    .flatten()
    .collect()
}

/// The glyph a tool paints — toggles show the state they move to, like
/// the title bar's Sun / Moon did.
pub(super) fn tool_icon(ui: &EditorUiState, tool: FileMenuTool) -> Icon {
    match tool {
        FileMenuTool::ToggleSidebar => Icon::PanelLeft,
        FileMenuTool::Variables => Icon::Braces,
        FileMenuTool::DesignSystem => Icon::BookOpen,
        FileMenuTool::AssetCenter => Icon::Palette,
        FileMenuTool::ToggleTheme => match ui.effective_theme_mode() {
            de_editor_core::ThemeMode::Dark => Icon::Sun,
            de_editor_core::ThemeMode::Light => Icon::Moon,
        },
        FileMenuTool::Language => Icon::Globe,
        FileMenuTool::ToggleFullscreen if ui.window_fullscreen => Icon::Minimize,
        FileMenuTool::ToggleFullscreen => Icon::Maximize,
        FileMenuTool::Git => Icon::GitBranch,
        FileMenuTool::Settings => Icon::Settings,
    }
}

/// What a tool is called, resolved against the live state so toggles name
/// the action they will perform.
pub fn tool_label(ui: &EditorUiState, tool: FileMenuTool) -> &'static str {
    let key = match tool {
        FileMenuTool::ToggleSidebar if ui.sidebar_open => "topbar.hideLayers",
        FileMenuTool::ToggleSidebar => "topbar.showLayers",
        FileMenuTool::Variables => "chrome.variables",
        FileMenuTool::DesignSystem => "toolbar.designSystem",
        FileMenuTool::AssetCenter => "assetCenter.title",
        FileMenuTool::ToggleTheme => match ui.effective_theme_mode() {
            de_editor_core::ThemeMode::Dark => "topbar.lightMode",
            de_editor_core::ThemeMode::Light => "topbar.darkMode",
        },
        FileMenuTool::Language => "tooltip.topbar.language",
        FileMenuTool::ToggleFullscreen if ui.window_fullscreen => "topbar.exitFullscreen",
        FileMenuTool::ToggleFullscreen => "topbar.fullscreen",
        FileMenuTool::Git if ui.git_panel.open => "git.closePanel",
        FileMenuTool::Git => "git.openPanel",
        FileMenuTool::Settings => "topbar.agentsAndMcp",
    };
    translate(ui, key)
}

impl<'a> FileMenu<'a> {
    /// Row index of the Import row — the first row after the recent-files
    /// block (and its Clear history row, when there are recents).
    pub(super) fn tail_row_start(&self) -> usize {
        self.recent_row_start() + self.recent.len() + usize::from(!self.recent.is_empty())
    }

    /// Row index of the first footer tool. The footer shares the menu's
    /// single `hover` index space with the rows above it.
    pub(super) fn footer_row_start(&self) -> usize {
        self.tail_row_start() + TAIL_ROWS
    }

    /// The footer tool rects, left to right, for a footer whose top edge
    /// is at `y` in a menu at `panel`.
    pub(super) fn footer_slots(&self, panel: Rect, y: f32) -> Vec<(FileMenuTool, Rect)> {
        let tools = footer_tools(self.ui);
        let count = tools.len().max(1) as f32;
        let span = MENU_WIDTH - PAD_X * 2.0;
        let step = span / count;
        tools
            .into_iter()
            .enumerate()
            .map(|(i, tool)| {
                let cx = panel.origin.x + PAD_X + step * (i as f32 + 0.5);
                let rect = Rect::xywh(
                    cx - FOOTER_BUTTON / 2.0,
                    y + (FOOTER_HEIGHT - FOOTER_BUTTON) / 2.0,
                    FOOTER_BUTTON,
                    FOOTER_BUTTON,
                );
                (tool, rect)
            })
            .collect()
    }

    /// Paint the footer whose top edge sits at `y`, plus the hovered
    /// tool's name hanging below the menu.
    pub(super) fn paint_footer(&self, cx: &mut PaintCx<'_>, panel: Rect, y: f32) {
        let start = self.footer_row_start();
        let mut hovered = None;
        for (i, (tool, rect)) in self.footer_slots(panel, y).into_iter().enumerate() {
            let is_hovered = self.menu.hover == Some(start + i);
            if is_hovered {
                cx.backend
                    .fill_round_rect(rect, 6.0, self.theme.button_hover);
                hovered = Some((tool, rect));
            }
            let color = if is_hovered {
                self.theme.foreground
            } else {
                self.theme.muted_foreground
            };
            draw_icon(
                cx.backend,
                tool_icon(self.ui, tool),
                Point2D::new(
                    rect.origin.x + (FOOTER_BUTTON - FOOTER_ICON) / 2.0,
                    rect.origin.y + (FOOTER_BUTTON - FOOTER_ICON) / 2.0,
                ),
                FOOTER_ICON,
                color,
                1.5,
            );
        }
        if let Some((tool, rect)) = hovered {
            // Anchor to the menu's bottom edge so the bubble hangs clear of
            // the menu instead of covering the row above the footer.
            let anchor = Rect::xywh(
                rect.origin.x,
                rect.origin.y,
                rect.size.x,
                panel.origin.y + panel.size.y - rect.origin.y,
            );
            tooltip::paint_tooltip(
                cx,
                &self.theme,
                anchor,
                tool_label(self.ui, tool),
                None,
                TooltipPlacement::Below,
                None,
            );
        }
    }
}
