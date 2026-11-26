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
    /// Tokens (and cost, for a priced model) of the last assistant reply.
    LastReply,
    /// The same, summed over this chat's replies.
    ChatTotal,
}

impl ChatContextItem {
    /// i18n key of the row's tag column.
    pub fn tag_key(self) -> &'static str {
        match self {
            ChatContextItem::Document => "chat.context.document",
            ChatContextItem::Selection => "chat.context.selection",
            ChatContextItem::Mention(_, kind) => kind.label_key(),
            ChatContextItem::Attachment(_) => "chat.context.attachment",
            ChatContextItem::LastReply => "chat.context.lastReply",
            ChatContextItem::ChatTotal => "chat.context.chatTotal",
        }
    }

    /// Whether the row carries a ✕.
    pub fn removable(self) -> bool {
        !matches!(
            self,
            ChatContextItem::Document | ChatContextItem::LastReply | ChatContextItem::ChatTotal
        )
    }
}

/// One row of the context panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatContextRow {
    pub item: ChatContextItem,
    pub label: String,
}

/// Every row the context panel lists, in a fixed order: document,
/// selection, mentions, attachments, then the usage of the last reply and of
/// the whole chat when a provider reported any.
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
    let locale = state.editor_ui.effective_locale();
    let mut replies = state.chat.messages.iter().filter_map(|m| m.usage.as_ref());
    if let Some(last) = state
        .chat
        .messages
        .iter()
        .rev()
        .find_map(|m| m.usage.as_ref())
    {
        rows.push(ChatContextRow {
            item: ChatContextItem::LastReply,
            label: crate::chat_usage_label::usage_label(locale, last),
        });
        if replies.clone().count() > 1 {
            let total = crate::chat_usage_label::sum_usage(&mut replies);
            rows.push(ChatContextRow {
                item: ChatContextItem::ChatTotal,
                label: crate::chat_usage_label::usage_label(locale, &total),
            });
        }
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

    #[test]
    fn usage_rows_follow_the_replies_that_reported_usage() {
        use crate::chat::{ChatMessage, TurnUsage};
        use de_ai::chat_usage::TokenUsage;
        let mut state = multi_page_state();
        let usage_rows = |state: &EditorState| {
            chat_context_rows(state)
                .into_iter()
                .filter(|r| {
                    matches!(
                        r.item,
                        ChatContextItem::LastReply | ChatContextItem::ChatTotal
                    )
                })
                .collect::<Vec<_>>()
        };
        state.chat.messages.push(ChatMessage::assistant("no usage"));
        assert!(usage_rows(&state).is_empty(), "CLI replies report nothing");
        let reply = |state: &mut EditorState, input: u64, cost: Option<f64>| {
            let mut msg = ChatMessage::assistant("hi");
            msg.usage = Some(TurnUsage::new(
                TokenUsage {
                    input_tokens: Some(input),
                    output_tokens: Some(10),
                    ..TokenUsage::default()
                },
                cost,
            ));
            state.chat.messages.push(msg);
        };
        reply(&mut state, 1_000, Some(0.01));
        let rows = usage_rows(&state);
        assert_eq!(rows.len(), 1, "one reply: no separate total");
        assert_eq!(rows[0].label, "1000 in · 10 out · $0.010");
        reply(&mut state, 2_000, Some(0.02));
        let rows = usage_rows(&state);
        assert_eq!(rows[0].item, ChatContextItem::LastReply);
        assert_eq!(rows[0].label, "2000 in · 10 out · $0.020");
        assert_eq!(rows[1].label, "3000 in · 20 out · $0.030");
        assert!(!ChatContextItem::ChatTotal.removable());
    }
}
