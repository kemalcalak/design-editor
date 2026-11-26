//! The chat's context panel (#398): what the next turn carries besides
//! the typed text — the document, the canvas selection, staged
//! `@`-mentions and attachments.
//!
//! The rows are built here (pure state, wasm32-clean) and painted by
//! `de-editor-ui`'s `ai_chat_context_panel`. They are resolved only while
//! the panel is open, so a closed panel costs nothing per frame.

use crate::chat_mentions::MentionKind;
use crate::find_layers::document_pages;
use crate::pen_node_ext::PenNodeExt;
use crate::EditorState;

/// What a context row stands for, and what its ✕ removes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatContextItem {
    /// The open document (always present; not removable).
    Document,
    /// The canvas selection; ✕ clears it.
    Selection,
    /// `chat.pending_mentions[i]`; ✕ unstages it and drops its token.
    Mention(usize, MentionKind),
    /// `chat.pending_attachments[i]`; ✕ unstages it.
    Attachment(usize),
}

impl ChatContextItem {
    /// i18n key of the row's tag column.
    pub fn tag_key(self) -> &'static str {
        match self {
            ChatContextItem::Document => "chat.context.document",
            ChatContextItem::Selection => "chat.context.selection",
            ChatContextItem::Mention(_, kind) => kind.label_key(),
            ChatContextItem::Attachment(_) => "chat.context.attachment",
        }
    }

    /// Whether the row carries a ✕.
    pub fn removable(self) -> bool {
        !matches!(self, ChatContextItem::Document)
    }
}

/// One row of the context panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatContextRow {
    pub item: ChatContextItem,
    pub label: String,
}

/// Every row the context panel lists, in a fixed order: document,
/// selection, mentions, attachments.
pub fn chat_context_rows(state: &EditorState) -> Vec<ChatContextRow> {
    let mut rows = Vec::with_capacity(
        2 + state.chat.pending_mentions.len() + state.chat.pending_attachments.len(),
    );
    let pages = document_pages(state);
    let active = state
        .ui
        .active_page_index
        .min(pages.len().saturating_sub(1));
    let page = pages
        .get(active)
        .map(|(index, name, _)| {
            if name.trim().is_empty() {
                format!("Page {}", index + 1)
            } else {
                name.to_string()
            }
        })
        .unwrap_or_default();
    rows.push(ChatContextRow {
        item: ChatContextItem::Document,
        label: page,
    });
    let selected: Vec<&str> = state
        .selection
        .set
        .iter()
        .filter_map(|id| crate::walkers::find_node(state.active_children(), id))
        .map(|node| {
            node.base()
                .name
                .as_deref()
                .filter(|n| !n.trim().is_empty())
                .unwrap_or_else(|| node.type_tag())
        })
        .collect();
    if !selected.is_empty() {
        rows.push(ChatContextRow {
            item: ChatContextItem::Selection,
            label: selected.join(", "),
        });
    }
    for (index, mention) in state.chat.pending_mentions.iter().enumerate() {
        rows.push(ChatContextRow {
            item: ChatContextItem::Mention(index, mention.kind),
            label: mention.label.clone(),
        });
    }
    for (index, attachment) in state.chat.pending_attachments.iter().enumerate() {
        rows.push(ChatContextRow {
            item: ChatContextItem::Attachment(index),
            label: attachment.name.clone(),
        });
    }
    rows
}

/// How many staged items (mentions + attachments) the footer's context
/// button counts. The document and selection are always implied.
pub fn staged_context_count(state: &EditorState) -> usize {
    state.chat.pending_mentions.len() + state.chat.pending_attachments.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat::ChatAttachment;
    use crate::chat_mentions::ChatMention;
    use crate::find_layers::tests::multi_page_state;
    use crate::NodeId;

    #[test]
    fn rows_list_document_selection_mentions_and_attachments_in_order() {
        let mut state = multi_page_state();
        assert_eq!(
            chat_context_rows(&state),
            vec![ChatContextRow {
                item: ChatContextItem::Document,
                label: "Home".into()
            }]
        );
        state.selection.set.push(NodeId::new("hero"));
        state.chat.pending_mentions.push(ChatMention {
            kind: MentionKind::Page,
            key: "1".into(),
            label: "Settings".into(),
        });
        state.chat.pending_attachments.push(ChatAttachment {
            name: "ref.png".into(),
            media_type: "image/png".into(),
            data: vec![1],
        });
        let rows = chat_context_rows(&state);
        let items: Vec<ChatContextItem> = rows.iter().map(|r| r.item).collect();
        assert_eq!(
            items,
            [
                ChatContextItem::Document,
                ChatContextItem::Selection,
                ChatContextItem::Mention(0, MentionKind::Page),
                ChatContextItem::Attachment(0),
            ]
        );
        assert_eq!(rows[1].label, "Hero");
        assert_eq!(staged_context_count(&state), 2);
        assert!(!ChatContextItem::Document.removable());
        assert!(ChatContextItem::Attachment(0).removable());
        assert_eq!(
            ChatContextItem::Mention(0, MentionKind::File).tag_key(),
            "chat.mention.file"
        );
    }
}
