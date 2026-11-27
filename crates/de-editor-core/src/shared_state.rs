//! A copy-on-write handle on an [`EditorState`] (#456).
//!
//! A live MCP call materializes one owned state for the request; every
//! tool that used to clone it (`snapshot_layout`, `batch_get`,
//! `read_nodes`, `find_nodes`, `execute`'s simulation, …) now holds a
//! [`SharedState`] instead. Reads go through `Deref` and copy nothing; the
//! first write through `DerefMut` clones the state only while another
//! holder still shares it (`Arc::make_mut`), so a read-only call costs no
//! copy beyond the request's own and a write costs at most the one copy it
//! cost before.

use std::ops::{Deref, DerefMut};
use std::sync::Arc;

use crate::state::EditorState;

/// Shared, copy-on-write [`EditorState`]. No `Default`: an empty state is
/// never what a tool should be handed by accident.
#[derive(Debug, Clone)]
pub struct SharedState(Arc<EditorState>);

impl SharedState {
    /// Share an owned state.
    pub fn new(state: EditorState) -> Self {
        Self(Arc::new(state))
    }

    /// Share a borrowed state: one copy, as the callers that cloned did.
    pub fn copied(state: &EditorState) -> Self {
        Self::new(state.clone())
    }

    /// Move `state` into a shared handle for a while, leaving a hollow
    /// stand-in behind (no document, kits or registry, and unlike
    /// `EditorState::new` no process-wide side effect). Put it back with
    /// [`Self::restore_into`].
    pub fn take_from(state: &mut EditorState) -> Self {
        Self::new(std::mem::replace(state, EditorState::hollow()))
    }

    /// Put the state back where [`Self::take_from`] took it, moved when
    /// nothing else shares it, else copied.
    pub fn restore_into(self, state: &mut EditorState) {
        *state = self.into_owned();
    }

    /// The owned state: moved out when nothing else shares it, else copied.
    pub fn into_owned(self) -> EditorState {
        Arc::unwrap_or_clone(self.0)
    }

    /// Whether `self` and `other` are the same allocation (tests).
    pub fn ptr_eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

/// [`SharedState::take_from`] that puts the state back when it drops,
/// also while a panic unwinds, so the caller's state is never left hollow.
/// Drop every other handle first and the state moves back uncopied.
pub struct TakenState<'a> {
    home: &'a mut EditorState,
    shared: Option<SharedState>,
}

impl<'a> TakenState<'a> {
    /// Take `state` into a shared handle until the guard drops.
    pub fn new(state: &'a mut EditorState) -> Self {
        let shared = SharedState::take_from(state);
        Self {
            home: state,
            shared: Some(shared),
        }
    }
}

impl Deref for TakenState<'_> {
    type Target = SharedState;

    fn deref(&self) -> &SharedState {
        self.shared.as_ref().expect("held until drop")
    }
}

impl DerefMut for TakenState<'_> {
    fn deref_mut(&mut self) -> &mut SharedState {
        self.shared.as_mut().expect("held until drop")
    }
}

impl Drop for TakenState<'_> {
    fn drop(&mut self) {
        if let Some(shared) = self.shared.take() {
            shared.restore_into(self.home);
        }
    }
}

impl From<EditorState> for SharedState {
    fn from(state: EditorState) -> Self {
        Self::new(state)
    }
}

impl Deref for SharedState {
    type Target = EditorState;

    fn deref(&self) -> &EditorState {
        &self.0
    }
}

impl DerefMut for SharedState {
    /// Copies the state first when another holder shares it.
    fn deref_mut(&mut self) -> &mut EditorState {
        Arc::make_mut(&mut self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_share_and_the_first_write_copies_only_while_shared() {
        let mut state = EditorState::new();
        state.doc.name = Some("A".into());
        let mut first = SharedState::new(state);
        let second = first.clone();
        assert!(first.ptr_eq(&second), "a clone shares the state");
        assert_eq!(second.doc.name.as_deref(), Some("A"));

        // Writing through a shared handle copies; the other keeps its view.
        first.doc.name = Some("B".into());
        assert!(!first.ptr_eq(&second));
        assert_eq!(second.doc.name.as_deref(), Some("A"));
        assert_eq!(first.doc.name.as_deref(), Some("B"));

        // A sole holder writes in place.
        let before = first.clone();
        drop(before);
        let ptr = Arc::as_ptr(&first.0);
        first.doc.name = Some("C".into());
        assert_eq!(Arc::as_ptr(&first.0), ptr, "no copy for a sole holder");
    }

    #[test]
    fn a_taken_state_comes_back_whole() {
        let mut state = EditorState::new();
        state.doc.name = Some("Live".into());
        let kits = state.ui_kits.len();
        let shared = SharedState::take_from(&mut state);
        assert!(state.ui_kits.is_empty(), "the stand-in is hollow");
        assert_eq!(shared.doc.name.as_deref(), Some("Live"));
        shared.restore_into(&mut state);
        assert_eq!(state.doc.name.as_deref(), Some("Live"));
        assert_eq!(state.ui_kits.len(), kits);
    }

    #[test]
    fn a_taken_state_comes_back_when_the_holder_panics() {
        let mut state = EditorState::new();
        state.doc.name = Some("Live".into());
        let kits = state.ui_kits.len();
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let taken = TakenState::new(&mut state);
            assert_eq!(taken.doc.name.as_deref(), Some("Live"));
            panic!("a tool panics while it holds the state");
        }));
        assert!(caught.is_err());
        assert_eq!(state.doc.name.as_deref(), Some("Live"), "restored");
        assert_eq!(state.ui_kits.len(), kits, "not the hollow stand-in");

        // And on the normal path, a write through the guard lands.
        {
            let mut taken = TakenState::new(&mut state);
            taken.doc.name = Some("Edited".into());
        }
        assert_eq!(state.doc.name.as_deref(), Some("Edited"));
    }

    #[test]
    fn into_owned_moves_a_sole_state_and_copies_a_shared_one() {
        let mut state = EditorState::new();
        state.doc.name = Some("Doc".into());
        let shared = SharedState::new(state);
        let other = shared.clone();
        let owned = shared.into_owned();
        assert_eq!(owned.doc, other.doc);
        drop(other);
        let moved = SharedState::new(owned).into_owned();
        assert_eq!(moved.doc.name.as_deref(), Some("Doc"));
    }
}
