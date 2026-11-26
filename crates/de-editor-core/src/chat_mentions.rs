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
//! - [`summary`] — compact summaries of document mentions + the context block
//! - `sync` — `impl EditorState`: popup sync, pick, highlight, removal

pub mod candidates;
pub mod query;
pub mod summary;
mod sync;
#[cfg(test)]
mod tests;

use std::sync::Arc;

pub use candidates::{mention_candidates, skill_candidates, MAX_MENTION_ROWS};
pub use query::{active_mention_query, MentionQuery, MENTION_TRIGGER, SKILL_TRIGGER};
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
    /// A user skill (`SKILL.md` in the project's `.design-editor/skills/`
    /// or the user's skills folder), picked with `/`.
    Skill,
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
            MentionKind::Skill => "chat.mention.skill",
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
            MentionKind::Skill => "skill",
        }
    }

    /// The character the draft writes before the label.
    pub fn trigger(self) -> char {
        match self {
            MentionKind::Skill => SKILL_TRIGGER,
            _ => MENTION_TRIGGER,
        }
    }
}

/// One `@`-reference staged for the next turn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatMention {
    pub kind: MentionKind,
    /// Stable key: a page index (decimal), a node / component id, a
    /// variable name, or a project-relative file path.
    pub key: String,
    /// What the draft shows after the `@` (the layer / page / component /
    /// variable name, or the file path).
    pub label: String,
}

impl ChatMention {
    /// The text this mention occupies in the draft, trigger included
    /// (`@Hero`, `/brand-voice`).
    pub fn token(&self) -> String {
        let mut token = String::with_capacity(self.label.len() + 1);
        token.push(self.kind.trigger());
        token.push_str(&self.label);
        token
    }

    /// Whether `text` still carries this mention's token.
    pub fn is_in(&self, text: &str) -> bool {
        text.contains(&self.token())
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MentionPopup {
    /// Byte offset of the trigger in the draft.
    pub start: usize,
    /// `@` (document items and files) or `/` (skills).
    pub trigger: char,
    /// The text typed after the `@` (up to the caret).
    pub query: String,
    /// Ranked rows, at most [`MAX_MENTION_ROWS`].
    pub rows: Vec<MentionCandidate>,
    /// Keyboard / hover highlight into `rows`.
    pub highlight: usize,
}

/// One user skill the `/` popup offers: its name and one-line description.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillOption {
    pub name: String,
    pub description: String,
}

/// The user skills the `/` popup offers, loaded by the host (project
/// folder first, then the user's skills folder). Shared by every tab, on
/// `ChatSessions`, like [`ProjectFiles`].
#[derive(Debug, Clone, Default)]
pub struct ChatSkills {
    pub skills: Arc<[SkillOption]>,
    /// Raised when a `/` popup opens, so the host reloads the folders.
    pub rescan_requested: bool,
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
}
