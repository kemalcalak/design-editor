//! Runtime-locale persistence boundaries for browser settings.

use super::*;
use de_editor_core::{EditorState, Locale};

#[test]
fn pending_locale_is_transient_and_does_not_change_persistence_fingerprint() {
    let mut state = EditorState::new();
    let before = fingerprint(&state);

    state.editor_ui.pending_locale = Some(Locale::Ja);

    assert_eq!(fingerprint(&state), before);
}
