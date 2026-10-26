//! The "What's new" window's seen release (#251), stored under its own
//! device key like the theme (`web_settings_theme.rs`), outside the
//! settings blob: it stays writable when the blob is not, and an older
//! bundle that rewrites the blob cannot drop it.

use de_editor_core::{EditorState, EmbedHost};

use crate::web_storage::{storage_get, storage_set_checked};

/// Device-level key; no `::anon` suffix, like the theme key.
const LAST_SEEN_KEY: &str = "design-editor-rust-web-last-seen-version";

thread_local! {
    /// Last value written (or read) this session, so the per-frame check is
    /// a comparison rather than a `localStorage` write.
    static LAST_WRITTEN: std::cell::RefCell<Option<String>> = const {
        std::cell::RefCell::new(None)
    };
}

/// Mount-time decision, after the settings load. Any stored preference
/// from an earlier visit (the settings blob, the device theme or this key)
/// marks an upgrade; a first visit shows nothing. Never inside an embedding
/// host such as the VS Code iframe, which has its own release channel.
pub(crate) fn open_after_load(state: &mut EditorState) {
    if state.editor_ui.embed != EmbedHost::None {
        return;
    }
    let seen = storage_get(LAST_SEEN_KEY).filter(|value| !value.trim().is_empty());
    let had_settings = seen.is_some()
        || storage_get(&super::settings_storage_key()).is_some()
        || storage_get(super::theme::DEVICE_THEME_KEY).is_some();
    LAST_WRITTEN.with(|cell| *cell.borrow_mut() = seen.clone());
    state.editor_ui.last_seen_version = seen;
    state
        .editor_ui
        .open_whats_new_on_startup(had_settings, env!("CARGO_PKG_VERSION"));
}

/// Write the seen release when it changed. Returns `false` only when the
/// browser refused the write.
pub(crate) fn save_if_changed(state: &EditorState) -> bool {
    let Some(seen) = state.editor_ui.last_seen_version.as_deref() else {
        return true;
    };
    let unchanged = LAST_WRITTEN.with(|cell| cell.borrow().as_deref() == Some(seen));
    if unchanged {
        return true;
    }
    if !storage_set_checked(LAST_SEEN_KEY, seen) {
        return false;
    }
    LAST_WRITTEN.with(|cell| *cell.borrow_mut() = Some(seen.to_string()));
    true
}
