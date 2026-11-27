//! Chat `@`-mentions (#398): point the agent at a page, a layer, a
//! component, a variable or a file in the project folder.
//!
//! Typing `@` in the chat composer opens a popup of ranked candidates;
//! picking one writes `@Label ` into the draft and records a
//! [`ChatMention`] on the active chat tab. On send, the mentions whose
//! token is still in the text are handed to the host, which resolves each
//! into a compact summary (never a dump of the whole node tree) and
//! appends them to the provider-bound user message.
//!
//! The state here is transport-free and wasm32-clean: document mentions
//! resolve in [`summary`]; file mentions only carry their project-relative
//! path, because reading the file is the host's job
//! (`de_host_services::chat_mention_context`).
//!
//! ### Module layout
//!
//! - this spine — the types and re-exports
//! - [`query`] — finding the `@query` the caret sits in
//! - [`candidates`] — ranking the popup rows
//! - [`identity`] — which token in the draft is which staged mention
//! - [`source`] — the host seam that lists the project folder
//! - [`summary`] — compact summaries of document mentions + the context block
//! - `sync` — `impl EditorState`: popup sync, pick, highlight, removal

pub mod candidates;
pub mod identity;
#[cfg(test)]
mod identity_tests;
pub mod query;
pub mod source;
pub mod summary;
mod sync;
#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::sync::Arc;

pub use candidates::{mention_candidates, MAX_MENTION_ROWS};
pub use identity::{live_mentions, mention_spans, retain_live, unique_label};
pub use query::{active_mention_query, MentionQuery};
pub use source::{ProjectFileList, ProjectFileSource};
pub use summary::{context_block, summarize_document_mention, MentionSummary};

/// What a mention points at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MentionKind {
    Page,
    /// A layer on any page (the canvas selection ranks first).
    Node,
    Component,
    Variable,
    /// A file in the project folder (the open document's directory).
    File,
}

impl MentionKind {
    /// The i18n key of the short kind tag shown in popup rows and the
    /// context panel.
    pub fn label_key(self) -> &'static str {
        match self {
            MentionKind::Page => "chat.mention.page",
            MentionKind::Node => "chat.mention.layer",
            MentionKind::Component => "chat.mention.component",
            MentionKind::Variable => "chat.mention.variable",
            MentionKind::File => "chat.mention.file",
        }
    }

    /// English word used inside the provider-bound context block. The
    /// block is model input, not chrome, so it is not localised.
    pub fn wire_name(self) -> &'static str {
        match self {
            MentionKind::Page => "page",
            MentionKind::Node => "layer",
            MentionKind::Component => "component",
            MentionKind::Variable => "variable",
            MentionKind::File => "file",
        }
    }
}

/// One `@`-reference staged for the next turn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatMention {
    pub kind: MentionKind,
    /// Stable key: a page id, a node / component id, a
    /// variable name, or a project-relative file path.
    pub key: String,
    /// What the draft shows after the `@` (the layer / page / component /
    /// variable name, or the file path).
    pub label: String,
}

impl ChatMention {
    /// The text this mention occupies in the draft, `@` included.
    pub fn token(&self) -> String {
        format!("@{}", self.label)
    }
}

/// One popup row: the mention it would stage plus a secondary line
/// (the page a layer sits on, a file's folder, a variable's kind).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MentionCandidate {
    pub mention: ChatMention,
    pub detail: String,
}

/// The open `@` popup for the active chat tab.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MentionPopup {
    /// Byte offset of the `@` in the draft.
    pub start: usize,
    /// The text typed after the `@` (up to the caret).
    pub query: String,
    /// Ranked rows, at most [`MAX_MENTION_ROWS`].
    pub rows: Vec<MentionCandidate>,
    /// Keyboard / hover highlight into `rows`.
    pub highlight: usize,
}

/// Files the `@` popup offers, scanned by the host from the project folder.
///
/// Lives on `ChatSessions` (not a tab, not `EditorUiState`) so every tab
/// shares one scan and `request_snapshot::narrowed_snapshot` drops it along
/// with the rest of the chat state. The list is an `Arc` so a rescan swaps
/// it without copying every path into each consumer.
#[derive(Debug, Clone, Default)]
pub struct ProjectFiles {
    /// The project folder, absolute; `None` for an unsaved document.
    pub root: Option<String>,
    /// Project-relative paths with `/` separators, sorted.
    pub files: Arc<[String]>,
    /// Raised by the popup sync when the popup opens, so the host rescans
    /// the folder (files come and go while the editor is open).
    pub rescan_requested: bool,
    /// Summaries of the staged file mentions, read by the host on a worker
    /// as soon as a file is staged (`None` while the read runs), so a send
    /// never reads the disk on the UI thread. Keyed by project-relative
    /// path; dropped when the folder changes and after the turn uses them.
    pub summaries: HashMap<String, Option<String>>,
}

impl ProjectFiles {
    /// Whether a staged file in `mentions` is still being read (the send
    /// waits for it).
    pub fn reading_any(&self, mentions: &[ChatMention]) -> bool {
        mentions.iter().any(|m| {
            m.kind == MentionKind::File && matches!(self.summaries.get(&m.key), Some(None))
        })
    }
}
