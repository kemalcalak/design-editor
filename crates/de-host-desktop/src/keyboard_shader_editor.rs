//! Arrow and Tab routing for the shader source editor (#164), split out
//! of `keyboard_input.rs` at the repo's 800-line cap.
//!
//! While the editor owns the keyboard the arrows move its caret (by
//! character and by line) and Tab indents; none of them may reach the
//! canvas, where an arrow would nudge the node whose shader is being
//! edited.

use winit::keyboard::NamedKey;

use crate::DesktopApp;

impl DesktopApp {
    /// Whether `key` belongs to the focused source editor. Cmd/Ctrl-held
    /// keys stay with the canvas chords.
    pub(crate) fn shader_editor_owns_key(&self, key: &NamedKey) -> bool {
        !self.zoom_modifier
            && matches!(
                key,
                NamedKey::ArrowLeft
                    | NamedKey::ArrowRight
                    | NamedKey::ArrowUp
                    | NamedKey::ArrowDown
                    | NamedKey::Tab
            )
            && self.host.shader_source_editor_focused()
    }

    /// Route one key the editor owns; always consumed.
    pub(crate) fn apply_shader_editor_key(&mut self, key: &NamedKey) -> bool {
        let extend = self.shift_modifier;
        match key {
            NamedKey::ArrowLeft => self.host.apply_shader_source_caret(false, extend),
            NamedKey::ArrowRight => self.host.apply_shader_source_caret(true, extend),
            NamedKey::ArrowUp => self.host.apply_shader_source_line(false, extend),
            NamedKey::ArrowDown => self.host.apply_shader_source_line(true, extend),
            NamedKey::Tab => self.host.apply_text('\t'),
            _ => false,
        };
        true
    }
}
