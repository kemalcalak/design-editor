//! Command palette keys (#395) for `DesktopApp`.
//!
//! Cmd/Ctrl+K and Cmd/Ctrl+/ open the command palette, Cmd/Ctrl+F opens
//! Find layers; a palette's own chord closes it and the other switches
//! mode. While the palette is open it owns every key: typing, caret,
//! Up / Down, Enter (Shift+Enter selects every match), Tab (replace
//! field) and Escape, plus the clipboard chords through the input-aware
//! paths. Commands only the runner can carry out come back through
//! [`DesktopApp::drain_palette_request`].

use de_editor_core::command_palette::{PaletteCommand, PaletteMode, PaletteRunnerRequest};
use winit::keyboard::{Key, NamedKey};

use crate::menu::MenuAction;
use crate::DesktopApp;

impl DesktopApp {
    /// The palette's share of a key press. `true` when it took the key.
    pub(crate) fn handle_palette_key(&mut self, logical_key: &Key, text: Option<&str>) -> bool {
        if let Some(mode) = self.palette_chord(logical_key) {
            // Unavailable (Home, Preview, Rename layers): the chord falls
            // through to the editor table.
            return self.host.apply_toggle_command_palette(mode);
        }
        if !self.host.command_palette_open() {
            return false;
        }
        match logical_key {
            Key::Named(NamedKey::Escape) => {
                self.host.palette_escape();
            }
            Key::Named(NamedKey::Enter) => {
                self.host.palette_activate(self.shift_modifier);
            }
            Key::Named(NamedKey::ArrowUp) => {
                self.host.palette_move(-1);
            }
            Key::Named(NamedKey::ArrowDown) => {
                self.host.palette_move(1);
            }
            Key::Named(NamedKey::ArrowLeft) => {
                self.host.palette_caret(false, self.shift_modifier);
            }
            Key::Named(NamedKey::ArrowRight) => {
                self.host.palette_caret(true, self.shift_modifier);
            }
            Key::Named(NamedKey::Tab) => {
                self.host.palette_tab();
            }
            Key::Named(NamedKey::Backspace) => {
                self.host.palette_backspace();
            }
            Key::Character(ch) if self.zoom_modifier => match ch.to_lowercase().as_str() {
                "a" => {
                    self.host.palette_select_all();
                }
                "c" => {
                    self.handle_cmd_copy();
                }
                "x" => {
                    self.handle_cmd_cut();
                }
                "v" => {
                    self.handle_cmd_paste();
                }
                // Every other chord is swallowed: nothing may edit the
                // document behind the modal.
                _ => {}
            },
            _ if !self.zoom_modifier => {
                for c in text.unwrap_or("").chars() {
                    if !c.is_control() {
                        self.host.palette_text(c);
                    }
                }
            }
            _ => {}
        }
        true
    }

    /// Cmd/Ctrl+K or + / (commands) and Cmd/Ctrl+F (find layers). `/` is
    /// accepted with Shift too: several layouts type it shifted.
    fn palette_chord(&self, logical_key: &Key) -> Option<PaletteMode> {
        if !self.zoom_modifier || self.alt_modifier {
            return None;
        }
        let Key::Character(ch) = logical_key else {
            return None;
        };
        match ch.to_lowercase().as_str() {
            "/" => Some(PaletteMode::Commands),
            "k" if !self.shift_modifier => Some(PaletteMode::Commands),
            "f" if !self.shift_modifier => Some(PaletteMode::FindLayers),
            _ => None,
        }
    }

    /// Run what a palette row handed to the runner: file dialogs, the Git
    /// panel, a new chat tab, a recent file. `true` when a repaint is owed.
    pub(crate) fn drain_palette_request(&mut self) -> bool {
        let Some(request) = self.host.take_palette_runner_request() else {
            return false;
        };
        let action = match request {
            PaletteRunnerRequest::OpenRecent(index) => MenuAction::OpenRecent(index),
            PaletteRunnerRequest::Command(command) => match command {
                PaletteCommand::NewChat => {
                    self.new_chat_tab();
                    return true;
                }
                PaletteCommand::GitPanel => MenuAction::ToggleGitPanel,
                PaletteCommand::NewFile => MenuAction::New,
                PaletteCommand::OpenFile => MenuAction::Open,
                PaletteCommand::Save => MenuAction::Save,
                PaletteCommand::SaveAs => MenuAction::SaveAs,
                PaletteCommand::ExportImage => MenuAction::Export,
                other => return self.host.run_palette_command(other),
            },
        };
        self.apply_menu_action(action)
    }
}

#[cfg(test)]
#[path = "keyboard_palette_tests.rs"]
mod tests;
