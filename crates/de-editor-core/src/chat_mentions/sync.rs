//! `impl EditorState` for the `@` popup: keeping it in step with the
//! draft, moving the highlight, picking a row and removing a staged
//! mention. Hosts call [`EditorState::sync_chat_mentions`] once before
//! painting. While neither the draft nor the staged mentions changed it only
//! hashes the draft (no allocation); the staged tokens are re-checked, and
//! the popup re-ranked, only when they did.

use super::{active_mention_query, mention_candidates, MentionPopup};
use crate::EditorState;

impl EditorState {
    /// Bring the `@` popup and the staged mentions in line with the chat
    /// draft and caret. Returns true when either changed (repaint).
    ///
    /// Opens the popup when the caret ends an `@query`, re-ranks it when
    /// the query changed, closes it otherwise, and drops staged mentions
    /// whose token the user deleted. Opening raises
    /// `chat.project_files.rescan_requested` so the host refreshes the file
    /// list and then calls [`EditorState::refresh_chat_mention_rows`].
    pub fn sync_chat_mentions(&mut self) -> bool {
        let mut changed = false;
        if !self.chat.pending_mentions.is_empty() {
            let chat = &mut *self.chat;
            let seen =
                super::identity::draft_fingerprint(chat.input.text(), chat.pending_mentions.len());
            if chat.mentions_checked_draft != Some(seen) {
                changed |= super::retain_live(chat.input.text(), &mut chat.pending_mentions);
                chat.mentions_checked_draft = Some(super::identity::draft_fingerprint(
                    chat.input.text(),
                    chat.pending_mentions.len(),
                ));
            }
        }
        let usable = self.chat.focused && !self.chat.has_streaming_turn();
        // Byte range of the query (after the `@`), so nothing borrowed from
        // the draft outlives the mutations below.
        let found = if usable {
            active_mention_query(self.chat.input.text(), self.chat.input.caret())
                .map(|q| (q.start, q.start + 1, q.start + 1 + q.query.len()))
        } else {
            None
        };
        let Some((start, query_from, query_to)) = found else {
            self.chat.mention_dismissed_at = None;
            if self.chat.mention_popup.take().is_some() {
                changed = true;
            }
            return changed;
        };
        if self.chat.mention_dismissed_at == Some(start) {
            if self.chat.mention_popup.take().is_some() {
                changed = true;
            }
            return changed;
        }
        self.chat.mention_dismissed_at = None;
        let unchanged = {
            let query = &self.chat.input.text()[query_from..query_to];
            self.chat
                .mention_popup
                .as_ref()
                .is_some_and(|popup| popup.start == start && popup.query == query)
        };
        if unchanged {
            return changed;
        }
        let query = self.chat.input.text()[query_from..query_to].to_string();
        let opening = self.chat.mention_popup.is_none();
        let rows = mention_candidates(self, &self.chat.project_files.files, &query);
        self.chat.mention_popup = Some(MentionPopup {
            start,
            query,
            rows,
            highlight: 0,
        });
        if opening {
            self.chat.project_files.rescan_requested = true;
        }
        true
    }

    /// Re-rank the open popup against the current file list (after the
    /// host rescanned the project folder). Keeps the highlight in range.
    pub fn refresh_chat_mention_rows(&mut self) -> bool {
        let Some(query) = self.chat.mention_popup.as_ref().map(|p| p.query.clone()) else {
            return false;
        };
        let rows = mention_candidates(self, &self.chat.project_files.files, &query);
        let Some(popup) = self.chat.mention_popup.as_mut() else {
            return false;
        };
        if popup.rows == rows {
            return false;
        }
        popup.highlight = popup.highlight.min(rows.len().saturating_sub(1));
        popup.rows = rows;
        true
    }

    /// Whether the `@` popup is open with at least one row — the state in
    /// which it owns Up / Down / Tab / Enter.
    pub fn chat_mention_popup_active(&self) -> bool {
        self.chat
            .mention_popup
            .as_ref()
            .is_some_and(|popup| !popup.rows.is_empty())
    }

    /// Move the highlight by `delta` rows, wrapping. False when closed.
    pub fn move_chat_mention_highlight(&mut self, delta: i32) -> bool {
        let Some(popup) = self.chat.mention_popup.as_mut() else {
            return false;
        };
        let len = popup.rows.len() as i32;
        if len == 0 {
            return false;
        }
        popup.highlight = (popup.highlight as i32 + delta).rem_euclid(len) as usize;
        true
    }

    /// Point the highlight at `row` (pointer hover). False when unchanged.
    pub fn set_chat_mention_highlight(&mut self, row: usize) -> bool {
        match self.chat.mention_popup.as_mut() {
            Some(popup) if row < popup.rows.len() && popup.highlight != row => {
                popup.highlight = row;
                true
            }
            _ => false,
        }
    }

    /// Close the popup for the `@` it is showing (Escape). It stays shut
    /// until the caret leaves that token.
    pub fn dismiss_chat_mention_popup(&mut self) -> bool {
        let Some(popup) = self.chat.mention_popup.take() else {
            return false;
        };
        self.chat.mention_dismissed_at = Some(popup.start);
        true
    }

    /// Pick `row` (or the highlighted row): replace the typed `@query` with
    /// `@Label ` and stage the mention. False when there is nothing to pick.
    pub fn pick_chat_mention(&mut self, row: Option<usize>, now_ms: u64) -> bool {
        let Some(popup) = self.chat.mention_popup.take() else {
            return false;
        };
        let Some(candidate) = popup.rows.get(row.unwrap_or(popup.highlight)).cloned() else {
            self.chat.mention_popup = Some(popup);
            return false;
        };
        let end = (popup.start + 1 + popup.query.len()).min(self.chat.input.text().len());
        // The same target picked again reuses its token; a different target
        // whose name a staged mention already uses gets `Name#2`, so every
        // token names exactly one target.
        let mention = match self.chat.pending_mentions.iter().find(|m| {
            (m.kind, m.key.as_str()) == (candidate.mention.kind, candidate.mention.key.as_str())
        }) {
            Some(staged) => staged.clone(),
            None => {
                let mut mention = candidate.mention;
                mention.label = super::unique_label(&mention, &self.chat.pending_mentions);
                mention
            }
        };
        let mut replacement = mention.token();
        replacement.push(' ');
        self.chat
            .input
            .replace_range(popup.start, end, &replacement, now_ms);
        if !self.chat.pending_mentions.contains(&mention) {
            self.chat.pending_mentions.push(mention);
        }
        self.chat.focused = true;
        true
    }

    /// Drop staged mention `index` and its `@Label` token from the draft
    /// (the context panel's ✕). False when out of range.
    pub fn remove_chat_mention(&mut self, index: usize, now_ms: u64) -> bool {
        if index >= self.chat.pending_mentions.len() {
            return false;
        }
        let span = super::mention_spans(self.chat.input.text(), &self.chat.pending_mentions)[index];
        self.chat.pending_mentions.remove(index);
        if let Some((start, end)) = span {
            let mut end = end;
            if self.chat.input.text()[end..].starts_with(' ') {
                end += 1;
            }
            self.chat.input.replace_range(start, end, "", now_ms);
        }
        true
    }
}
