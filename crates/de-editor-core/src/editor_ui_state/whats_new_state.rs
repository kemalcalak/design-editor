//! "What's new" window state (#251).
//!
//! On the first launch after an update the hosts call
//! [`EditorUiState::open_whats_new_on_startup`], which opens one page per
//! release the user has not seen yet (newest first) or, when there is
//! nothing to show, records the running version as seen. Closing the
//! window records it too, so it never comes back for that version. Pure
//! state: the hosts persist `last_seen_version` with their settings.

use crate::release_notes;
use crate::version_order::is_newer;

/// A button on the "What's new" card.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WhatsNewButton {
    /// One release newer.
    Previous,
    /// One release older.
    Next,
    /// Close the window and mark the running version as seen.
    GotIt,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhatsNewState {
    /// Releases with notes, newest first; never empty.
    pub versions: Vec<String>,
    /// Index into `versions` of the page on screen.
    pub page: usize,
    /// The running version, recorded as seen on close.
    pub running: String,
    pub hover: Option<WhatsNewButton>,
}

impl WhatsNewState {
    /// The release on screen.
    pub fn version(&self) -> &str {
        &self.versions[self.page.min(self.versions.len() - 1)]
    }

    /// Buttons in paint order (left to right). Paging buttons only show
    /// when there is more than one release to page through.
    pub fn buttons(&self) -> &'static [WhatsNewButton] {
        if self.versions.len() > 1 {
            &[
                WhatsNewButton::Previous,
                WhatsNewButton::Next,
                WhatsNewButton::GotIt,
            ]
        } else {
            &[WhatsNewButton::GotIt]
        }
    }

    /// Whether a paging button can move from this page.
    pub fn enabled(&self, button: WhatsNewButton) -> bool {
        match button {
            WhatsNewButton::Previous => self.page > 0,
            WhatsNewButton::Next => self.page + 1 < self.versions.len(),
            WhatsNewButton::GotIt => true,
        }
    }
}

impl super::EditorUiState {
    /// Launch-time decision. `had_settings` is whether the host found saved
    /// settings from an earlier run: without them this is a fresh install,
    /// which shows nothing. Saved settings without a seen version come from
    /// a build older than this window, so only `running`'s own notes show.
    pub fn open_whats_new_on_startup(&mut self, had_settings: bool, running: &str) {
        let locale = self.effective_locale();
        let versions: Vec<String> = match (had_settings, self.last_seen_version.as_deref()) {
            (false, _) => Vec::new(),
            (true, None) => release_notes::note_for(running, locale)
                .map(|note| vec![note.version.clone()])
                .unwrap_or_default(),
            (true, Some(seen)) => release_notes::notes_between(seen, running, locale)
                .into_iter()
                .map(|note| note.version.clone())
                .collect(),
        };
        if versions.is_empty() {
            self.mark_version_seen(running);
            return;
        }
        self.whats_new = Some(WhatsNewState {
            versions,
            page: 0,
            running: running.to_string(),
            hover: None,
        });
    }

    /// Apply a card button press.
    pub fn press_whats_new_button(&mut self, button: WhatsNewButton) {
        let Some(state) = self.whats_new.as_mut() else {
            return;
        };
        if !state.enabled(button) {
            return;
        }
        match button {
            WhatsNewButton::Previous => state.page -= 1,
            WhatsNewButton::Next => state.page += 1,
            WhatsNewButton::GotIt => {
                let running = std::mem::take(&mut state.running);
                self.whats_new = None;
                self.mark_version_seen(&running);
                // An update offer that arrived meanwhile waited hidden.
                if let Some(offer) = self
                    .update_modal
                    .as_mut()
                    .filter(|modal| modal.phase == super::UpdatePhase::Offer)
                {
                    offer.visible = true;
                }
            }
        }
    }

    /// Escape closes the window like "Got it". Returns `true` when consumed.
    pub fn escape_whats_new(&mut self) -> bool {
        if self.whats_new.is_none() {
            return false;
        }
        self.press_whats_new_button(WhatsNewButton::GotIt);
        true
    }

    /// Whether the window is on screen (and so owns pointer + keyboard).
    pub fn whats_new_visible(&self) -> bool {
        self.whats_new.is_some()
    }

    /// Never move the seen version backwards: after a downgrade, the newer
    /// release's notes were already read.
    fn mark_version_seen(&mut self, running: &str) {
        let keep = self
            .last_seen_version
            .as_deref()
            .is_some_and(|seen| is_newer(seen, running));
        if !keep {
            self.last_seen_version = Some(running.to_string());
        }
    }
}

#[cfg(test)]
#[path = "whats_new_state_tests.rs"]
mod tests;
