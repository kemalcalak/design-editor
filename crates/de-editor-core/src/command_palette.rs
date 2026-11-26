//! Command palette and find layers (#395).
//!
//! One keyboard-first modal with two modes. **Commands** (Cmd+K / Cmd+/)
//! ranks every editor command (with its chord), the document's pages,
//! components (insert as an instance), variables and recent files against
//! what is typed. **Find layers** (Cmd+F) searches every page with
//! [`crate::find_layers`]: Enter selects and frames the highlighted match,
//! Shift+Enter selects every match on its page, and Tab moves to a replace
//! field whose Enter rewrites the matched text.
//!
//! This module owns the state and every transition; hosts hit-test and
//! paint it (`de_editor_ui::widgets::command_palette_panel`) and carry out
//! the [`PaletteOutcome`] an activation returns.

pub mod commands;
pub mod fuzzy;
mod keys;
mod rows;

use std::borrow::Cow;

use jian_core::scroll::ScrollState;
use jian_core::text_input::TextInputState;

use crate::find_layers::{FindQuery, LayerMatch};
use crate::{EditorState, NodeId};

pub use commands::PaletteCommand;
pub use keys::{
    command_palette_activate, command_palette_backspace, command_palette_caret,
    command_palette_escape, command_palette_move, command_palette_paste,
    command_palette_select_all, command_palette_tab, command_palette_text,
};

/// Height of one result row, in px. Shared with the panel so keyboard
/// navigation can keep the highlighted row in view.
pub const ROW_HEIGHT: f32 = 34.0;
/// Rows on show before the list scrolls.
pub const VISIBLE_ROWS: usize = 10;
/// Rows the Commands mode keeps for a query.
const MAX_COMMAND_ROWS: usize = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaletteMode {
    Commands,
    FindLayers,
}

/// Which input owns the keyboard (find mode has two).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaletteField {
    Query,
    Replace,
}

/// What a row stands for.
#[derive(Debug, Clone, PartialEq)]
pub enum PaletteItem {
    Command(PaletteCommand),
    Page(usize),
    Component(NodeId),
    Variable(String),
    RecentFile(usize),
    Layer(LayerMatch),
}

/// One result row. Labels are resolved when the results are built, so
/// painting a row never allocates.
#[derive(Debug, Clone, PartialEq)]
pub struct PaletteRow {
    pub item: PaletteItem,
    pub label: Cow<'static, str>,
    /// Muted text after the label (a text match's content, a file's folder).
    pub secondary: Cow<'static, str>,
    /// Right-aligned: a command's chord, a row's kind, a match's page.
    pub trailing: Cow<'static, str>,
    pub score: u32,
}

/// The pointer is over …
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaletteHover {
    Row(usize),
    ReplaceAll,
}

/// The open palette.
#[derive(Debug, Clone)]
pub struct CommandPaletteState {
    pub mode: PaletteMode,
    pub input: TextInputState,
    pub replace: TextInputState,
    pub field: PaletteField,
    pub rows: Vec<PaletteRow>,
    pub highlighted: usize,
    pub scroll: ScrollState,
    pub hover: Option<PaletteHover>,
    /// Footer line, formatted when the results change (`12 matches on 3
    /// pages`, `Replaced in 4 layers`).
    pub summary: String,
    /// Find mode: the parsed query the rows came from.
    pub query: FindQuery,
}

impl CommandPaletteState {
    fn new(mode: PaletteMode) -> Self {
        Self {
            mode,
            input: TextInputState::default(),
            replace: TextInputState::default(),
            field: PaletteField::Query,
            rows: Vec::new(),
            highlighted: 0,
            scroll: ScrollState::default(),
            hover: None,
            summary: String::new(),
            query: FindQuery::default(),
        }
    }

    /// The input that owns the keyboard.
    pub fn active_input(&self) -> &TextInputState {
        match self.field {
            PaletteField::Query => &self.input,
            PaletteField::Replace => &self.replace,
        }
    }

    pub fn active_input_mut(&mut self) -> &mut TextInputState {
        match self.field {
            PaletteField::Query => &mut self.input,
            PaletteField::Replace => &mut self.replace,
        }
    }

    /// Keep the highlighted row inside the scrolled window.
    pub(crate) fn reveal_highlighted(&mut self) {
        let top = self.highlighted as f32 * ROW_HEIGHT;
        let window = VISIBLE_ROWS as f32 * ROW_HEIGHT;
        if top < self.scroll.offset {
            self.scroll.offset = top;
        } else if top + ROW_HEIGHT > self.scroll.offset + window {
            self.scroll.offset = top + ROW_HEIGHT - window;
        }
    }

    /// The find matches as `(page, id)` pairs, in rank order.
    pub fn layer_matches(&self) -> Vec<(usize, NodeId)> {
        self.rows
            .iter()
            .filter_map(|row| match &row.item {
                PaletteItem::Layer(found) => Some((found.page_index, found.node_id.clone())),
                _ => None,
            })
            .collect()
    }
}

/// Work a widget host hands to the desktop runner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaletteRunnerRequest {
    Command(PaletteCommand),
    OpenRecent(usize),
}

/// What the host does after an activation.
#[derive(Debug, Clone, PartialEq)]
pub enum PaletteOutcome {
    /// Run a command (the palette is closed).
    Command(PaletteCommand),
    /// Open the recent file at this index (desktop runner).
    OpenRecent(usize),
    /// The selection changed (maybe on another page): frame these nodes.
    Frame(Vec<NodeId>),
    /// Another page is now active: fit its content.
    FitPage,
    /// Show the Variables panel.
    OpenVariables,
    /// Text was replaced in this many layers; the palette stays open.
    Replaced(usize),
    /// Nothing for the host to do beyond a repaint.
    Nothing,
}

impl EditorState {
    /// Whether the palette is open.
    pub fn command_palette_open(&self) -> bool {
        self.editor_ui.command_palette.is_some()
    }

    /// Open the palette in `mode`. The same chord again closes it; the
    /// other mode's chord switches over with an empty query.
    pub fn toggle_command_palette(&mut self, mode: PaletteMode, now_ms: u64) {
        match self.editor_ui.command_palette.as_mut() {
            Some(palette) if palette.mode == mode => {
                self.close_command_palette();
                return;
            }
            Some(palette) => {
                *palette = CommandPaletteState::new(mode);
                palette.input.touch(now_ms);
            }
            None => {
                self.editor_ui.layer_context_menu = None;
                let mut palette = CommandPaletteState::new(mode);
                palette.input.touch(now_ms);
                self.editor_ui.command_palette = Some(palette);
            }
        }
        self.refresh_command_palette();
    }

    pub fn close_command_palette(&mut self) {
        self.editor_ui.command_palette = None;
    }

    /// Rebuild the rows from the query (after every query edit).
    pub fn refresh_command_palette(&mut self) {
        let Some(mut palette) = self.editor_ui.command_palette.take() else {
            return;
        };
        match palette.mode {
            PaletteMode::Commands => {
                let mut rows = rows::command_rows(self, palette.input.text());
                rows.truncate(MAX_COMMAND_ROWS);
                palette.rows = rows;
                palette.summary.clear();
            }
            PaletteMode::FindLayers => {
                palette.query = FindQuery::parse(palette.input.text());
                let (rows, summary) = rows::find_rows(self, &palette.query);
                palette.rows = rows;
                palette.summary = summary;
            }
        }
        palette.highlighted = 0;
        palette.scroll.offset = 0.0;
        palette.hover = None;
        self.editor_ui.command_palette = Some(palette);
    }
}

#[cfg(test)]
#[path = "command_palette_tests.rs"]
mod tests;
