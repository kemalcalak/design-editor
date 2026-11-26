//! The palette's keyboard and activation (#395). The palette is modal:
//! while it is open it owns every key, so a typed letter never switches
//! tools and an arrow never nudges a layer.
//!
//! Each transition returns `None` when the palette is closed (the host
//! keeps walking its ladder) and `Some(changed)` when the palette took the
//! key.

use super::rows::replaced_summary;
use super::{PaletteCommand, PaletteField, PaletteItem, PaletteMode, PaletteOutcome};
use crate::id_allocator::SequentialIdAllocator;
use crate::{EditorState, NodeId};

fn after_edit(state: &mut EditorState) {
    let query_field = state
        .editor_ui
        .command_palette
        .as_ref()
        .is_some_and(|palette| palette.field == PaletteField::Query);
    if query_field {
        state.refresh_command_palette();
    }
}

/// A typed character: into the focused field.
pub fn command_palette_text(state: &mut EditorState, c: char, now_ms: u64) -> Option<bool> {
    let palette = state.editor_ui.command_palette.as_mut()?;
    if c.is_control() {
        return Some(false);
    }
    let mut buf = [0u8; 4];
    palette
        .active_input_mut()
        .insert_str(c.encode_utf8(&mut buf), now_ms);
    after_edit(state);
    Some(true)
}

/// Pasted text: into the focused field, line breaks as spaces.
pub fn command_palette_paste(state: &mut EditorState, text: &str, now_ms: u64) -> Option<bool> {
    let palette = state.editor_ui.command_palette.as_mut()?;
    let clean: String = text
        .chars()
        .map(|c| if c == '\n' || c == '\r' { ' ' } else { c })
        .filter(|c| !c.is_control())
        .collect();
    if clean.is_empty() {
        return Some(false);
    }
    palette.active_input_mut().insert_str(&clean, now_ms);
    after_edit(state);
    Some(true)
}

/// Backspace in the focused field.
pub fn command_palette_backspace(state: &mut EditorState, now_ms: u64) -> Option<bool> {
    let palette = state.editor_ui.command_palette.as_mut()?;
    let before = palette.active_input().text().len();
    palette.active_input_mut().backspace(now_ms);
    let changed = palette.active_input().text().len() != before;
    if changed {
        after_edit(state);
    }
    Some(changed)
}

/// Left / Right (with Shift: extend the selection) in the focused field.
pub fn command_palette_caret(
    state: &mut EditorState,
    forward: bool,
    select: bool,
    now_ms: u64,
) -> Option<bool> {
    let input = state.editor_ui.command_palette.as_mut()?.active_input_mut();
    if forward {
        input.move_right(select, now_ms);
    } else {
        input.move_left(select, now_ms);
    }
    Some(true)
}

/// Cmd+A in the focused field.
pub fn command_palette_select_all(state: &mut EditorState) -> Option<bool> {
    state
        .editor_ui
        .command_palette
        .as_mut()?
        .active_input_mut()
        .select_all();
    Some(true)
}

/// Up / Down: move the highlight, wrapping at either end.
pub fn command_palette_move(state: &mut EditorState, delta: i32) -> Option<bool> {
    let palette = state.editor_ui.command_palette.as_mut()?;
    let count = palette.rows.len();
    if count == 0 {
        return Some(false);
    }
    let next = (palette.highlighted as i64 + delta as i64).rem_euclid(count as i64) as usize;
    if next == palette.highlighted {
        return Some(false);
    }
    palette.highlighted = next;
    palette.reveal_highlighted();
    Some(true)
}

/// Tab: in find mode, move between the query and the replace field.
pub fn command_palette_tab(state: &mut EditorState, now_ms: u64) -> Option<bool> {
    let palette = state.editor_ui.command_palette.as_mut()?;
    if palette.mode != PaletteMode::FindLayers {
        return Some(false);
    }
    palette.field = match palette.field {
        PaletteField::Query => PaletteField::Replace,
        PaletteField::Replace => PaletteField::Query,
    };
    palette.active_input_mut().touch(now_ms);
    Some(true)
}

/// Escape: close.
pub fn command_palette_escape(state: &mut EditorState) -> Option<bool> {
    state.editor_ui.command_palette.as_ref()?;
    state.close_command_palette();
    Some(true)
}

/// Enter (`all` = Shift+Enter): run the highlighted row, or replace when
/// the replace field is focused.
pub fn command_palette_activate(state: &mut EditorState, all: bool) -> Option<PaletteOutcome> {
    let palette = state.editor_ui.command_palette.as_ref()?;
    if palette.field == PaletteField::Replace {
        return Some(state.replace_in_command_palette());
    }
    let index = palette.highlighted;
    Some(state.activate_command_palette_row(index, all))
}

impl EditorState {
    /// Run row `index` (a click, or Enter on the highlight). `all` selects
    /// every find match on the row's page instead of the row alone.
    pub fn activate_command_palette_row(&mut self, index: usize, all: bool) -> PaletteOutcome {
        let Some(palette) = self.editor_ui.command_palette.as_ref() else {
            return PaletteOutcome::Nothing;
        };
        let Some(row) = palette.rows.get(index) else {
            return PaletteOutcome::Nothing;
        };
        let item = row.item.clone();
        let matches = palette.layer_matches();
        if let PaletteItem::Command(PaletteCommand::FindLayers) = item {
            if let Some(palette) = self.editor_ui.command_palette.as_mut() {
                *palette = super::CommandPaletteState::new(PaletteMode::FindLayers);
            }
            self.refresh_command_palette();
            return PaletteOutcome::Nothing;
        }
        self.close_command_palette();
        match item {
            PaletteItem::Command(command) => PaletteOutcome::Command(command),
            PaletteItem::RecentFile(index) => PaletteOutcome::OpenRecent(index),
            PaletteItem::Variable(_) => PaletteOutcome::OpenVariables,
            PaletteItem::Page(page) => {
                if page != self.ui.active_page_index && self.set_active_page(page) {
                    PaletteOutcome::FitPage
                } else {
                    PaletteOutcome::Nothing
                }
            }
            PaletteItem::Component(id) => self.insert_palette_instance(&id),
            PaletteItem::Layer(found) if all => {
                let ids: Vec<NodeId> = matches
                    .into_iter()
                    .filter(|(page, _)| *page == found.page_index)
                    .map(|(_, id)| id)
                    .collect();
                let selected = self.select_layer_matches(found.page_index, &ids);
                if selected.is_empty() {
                    PaletteOutcome::Nothing
                } else {
                    PaletteOutcome::Frame(selected)
                }
            }
            PaletteItem::Layer(found) => {
                if self.select_layer_match(found.page_index, &found.node_id) {
                    PaletteOutcome::Frame(vec![found.node_id])
                } else {
                    PaletteOutcome::Nothing
                }
            }
        }
    }

    /// A component row: a linked instance on the active page, selected.
    fn insert_palette_instance(&mut self, component: &NodeId) -> PaletteOutcome {
        let Ok(mut allocator) = SequentialIdAllocator::for_document(&self.doc, 1) else {
            return PaletteOutcome::Nothing;
        };
        match self.insert_component_instance_with_allocator(
            component,
            &NodeId::NONE,
            (None, None),
            None,
            &mut allocator,
        ) {
            Ok(Some(id)) => PaletteOutcome::Frame(vec![id]),
            _ => PaletteOutcome::Nothing,
        }
    }

    /// Replace the query's text in every text match with the replace
    /// field's text, as one undo step. The palette stays open and its
    /// results refresh; the footer reports how many layers changed.
    pub fn replace_in_command_palette(&mut self) -> PaletteOutcome {
        let Some(palette) = self.editor_ui.command_palette.as_ref() else {
            return PaletteOutcome::Nothing;
        };
        if palette.mode != PaletteMode::FindLayers {
            return PaletteOutcome::Nothing;
        }
        let query = palette.query.clone();
        let matches = palette.layer_matches();
        let replacement = palette.replace.text().to_string();
        let count = self.replace_text_in_layer_matches(&query, &matches, &replacement);
        self.refresh_command_palette();
        let locale = self.editor_ui.effective_locale();
        if let Some(palette) = self.editor_ui.command_palette.as_mut() {
            palette.summary = replaced_summary(locale, count);
        }
        PaletteOutcome::Replaced(count)
    }
}
