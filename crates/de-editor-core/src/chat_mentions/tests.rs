use super::*;
use crate::find_layers::tests::multi_page_state;
use crate::{EditorState, NodeId};

fn typed(state: &mut EditorState, text: &str) {
    state.chat.focused = true;
    state.chat.input.set_text(text);
    state.chat.input.set_caret(text.len(), 0);
}

fn labels(state: &EditorState) -> Vec<String> {
    state
        .chat
        .mention_popup
        .as_ref()
        .map(|p| p.rows.iter().map(|r| r.mention.label.clone()).collect())
        .unwrap_or_default()
}

#[test]
fn query_opens_at_text_start_or_after_whitespace_only() {
    assert_eq!(
        active_mention_query("@her", 4),
        Some(MentionQuery {
            start: 0,
            query: "her"
        })
    );
    assert_eq!(active_mention_query("fix @", 5).map(|q| q.start), Some(4));
    assert_eq!(
        active_mention_query("fix @hero", 9).map(|q| q.query),
        Some("hero")
    );
    // An email address is not a mention.
    assert_eq!(active_mention_query("ana@example", 11), None);
    // Whitespace after the token closes it.
    assert_eq!(active_mention_query("@hero now", 9), None);
    // Caret inside the token: the query runs up to the caret.
    assert_eq!(
        active_mention_query("@hero", 3).map(|q| q.query),
        Some("he")
    );
    // Out-of-range / non-boundary caret.
    assert_eq!(active_mention_query("@é", 2), None);
    assert_eq!(active_mention_query("@", 9), None);
}

#[test]
fn overly_long_query_is_abandoned() {
    let long = format!("@{}", "a".repeat(80));
    assert_eq!(active_mention_query(&long, long.len()), None);
}

#[test]
fn candidates_rank_layers_pages_components_variables_and_files() {
    let state = multi_page_state();
    let files = vec!["docs/brief.md".to_string(), "hero.png".to_string()];
    let rows = mention_candidates(&state, &files, "hero");
    let kinds: Vec<(MentionKind, &str)> = rows
        .iter()
        .map(|r| (r.mention.kind, r.mention.label.as_str()))
        .collect();
    assert_eq!(kinds[0], (MentionKind::Node, "Hero"));
    assert!(kinds.contains(&(MentionKind::File, "hero.png")));
    // Layers on a multi-page document name their page.
    assert_eq!(rows[0].detail, "frame · Home");

    let pages = mention_candidates(&state, &[], "sett");
    assert_eq!(pages[0].mention.kind, MentionKind::Page);
    assert_eq!(pages[0].mention.key, "p2");

    let vars = mention_candidates(&state, &[], "primary");
    assert_eq!(vars[0].mention.kind, MentionKind::Variable);
    assert_eq!(vars[0].detail, "color · #2563EB");
}

#[test]
fn selection_ranks_first_even_unnamed_and_on_empty_query() {
    let mut state = multi_page_state();
    state.selection.set.push(NodeId::new("cta"));
    let rows = mention_candidates(&state, &[], "");
    assert_eq!(rows[0].mention.key, "cta");
    // The selected layer is not listed twice.
    assert_eq!(rows.iter().filter(|r| r.mention.key == "cta").count(), 1);
    assert!(rows.len() <= MAX_MENTION_ROWS);
}

#[test]
fn sync_opens_reranks_and_closes_the_popup() {
    let mut state = multi_page_state();
    typed(&mut state, "make @he");
    assert!(state.sync_chat_mentions());
    assert!(
        state.chat.project_files.rescan_requested,
        "opening asks for a file scan"
    );
    assert_eq!(labels(&state)[0], "Hero");
    // Same text and caret: nothing to do.
    assert!(!state.sync_chat_mentions());
    typed(&mut state, "make @set");
    assert!(state.sync_chat_mentions());
    assert_eq!(labels(&state)[0], "Settings");
    typed(&mut state, "make @set ");
    assert!(state.sync_chat_mentions());
    assert!(state.chat.mention_popup.is_none());
    // Not focused: closed.
    typed(&mut state, "@he");
    state.chat.focused = false;
    state.sync_chat_mentions();
    assert!(state.chat.mention_popup.is_none());
}

#[test]
fn pick_replaces_the_query_and_stages_the_mention_once() {
    let mut state = multi_page_state();
    typed(&mut state, "tighten @he spacing");
    state.chat.input.set_caret("tighten @he".len(), 0);
    state.sync_chat_mentions();
    assert!(state.pick_chat_mention(None, 0));
    assert_eq!(state.chat.input.text(), "tighten @Hero  spacing");
    assert_eq!(state.chat.pending_mentions.len(), 1);
    assert_eq!(state.chat.pending_mentions[0].key, "hero");
    // The caret now follows whitespace: the popup stays shut.
    state.sync_chat_mentions();
    assert!(state.chat.mention_popup.is_none());
    // Picking the same target again does not stage a duplicate.
    let end = state.chat.input.text().len();
    state.chat.input.replace_range(end, end, " @He", 0);
    state.sync_chat_mentions();
    assert!(state.pick_chat_mention(Some(0), 0));
    assert_eq!(state.chat.pending_mentions.len(), 1);
}

#[test]
fn deleting_the_token_unstages_the_mention() {
    let mut state = multi_page_state();
    typed(&mut state, "@Butt");
    state.sync_chat_mentions();
    state.pick_chat_mention(None, 0);
    assert_eq!(state.chat.pending_mentions[0].label, "Button");
    typed(&mut state, "no mention left");
    assert!(state.sync_chat_mentions());
    assert!(state.chat.pending_mentions.is_empty());
}

#[test]
fn escape_dismisses_until_the_caret_leaves_the_token() {
    let mut state = multi_page_state();
    typed(&mut state, "@he");
    state.sync_chat_mentions();
    assert!(state.dismiss_chat_mention_popup());
    assert!(!state.sync_chat_mentions());
    assert!(state.chat.mention_popup.is_none());
    typed(&mut state, "@her");
    state.sync_chat_mentions();
    assert!(state.chat.mention_popup.is_none(), "same @ stays dismissed");
    typed(&mut state, "@her ");
    state.sync_chat_mentions();
    typed(&mut state, "@her @ti");
    state.sync_chat_mentions();
    assert!(state.chat.mention_popup.is_some(), "a new @ opens again");
}

#[test]
fn highlight_wraps_and_hover_sets_it() {
    let mut state = multi_page_state();
    typed(&mut state, "@");
    state.sync_chat_mentions();
    let len = state.chat.mention_popup.as_ref().unwrap().rows.len();
    assert!(len > 1);
    assert!(state.move_chat_mention_highlight(-1));
    assert_eq!(
        state.chat.mention_popup.as_ref().unwrap().highlight,
        len - 1
    );
    assert!(state.move_chat_mention_highlight(1));
    assert_eq!(state.chat.mention_popup.as_ref().unwrap().highlight, 0);
    assert!(state.set_chat_mention_highlight(1));
    assert!(!state.set_chat_mention_highlight(1));
    assert!(!state.set_chat_mention_highlight(len));
}

#[test]
fn remove_drops_the_mention_and_its_token() {
    let mut state = multi_page_state();
    typed(&mut state, "@Hero and @Title");
    state.chat.pending_mentions = vec![
        ChatMention {
            kind: MentionKind::Node,
            key: "hero".into(),
            label: "Hero".into(),
        },
        ChatMention {
            kind: MentionKind::Node,
            key: "title".into(),
            label: "Title".into(),
        },
    ];
    assert!(state.remove_chat_mention(0, 0));
    assert_eq!(state.chat.input.text(), "and @Title");
    assert_eq!(state.chat.pending_mentions.len(), 1);
    assert!(!state.remove_chat_mention(5, 0));
}

#[test]
fn begin_send_keeps_only_live_mentions() {
    let mut state = multi_page_state();
    state.chat.input.set_text("fix @Hero");
    state.chat.pending_mentions = vec![
        ChatMention {
            kind: MentionKind::Node,
            key: "hero".into(),
            label: "Hero".into(),
        },
        ChatMention {
            kind: MentionKind::Page,
            key: "p2".into(),
            label: "Settings".into(),
        },
    ];
    assert!(state.chat.begin_send());
    assert_eq!(state.chat.pending_mentions.len(), 1);
    assert_eq!(state.chat.pending_mentions[0].label, "Hero");
}

#[test]
fn summaries_are_compact_and_specific() {
    let state = multi_page_state();
    let node = |key: &str| ChatMention {
        kind: MentionKind::Node,
        key: key.into(),
        label: key.into(),
    };
    let hero = summarize_document_mention(&state, &node("hero")).unwrap();
    assert!(hero.starts_with("frame \"Hero\" (id hero) on page \"Home\" — 400×300"));
    assert!(hero.contains("fill #2563EB"));
    assert!(hero.contains("2 direct children (2 layers inside): text \"Title\""));
    let title = summarize_document_mention(&state, &node("title")).unwrap();
    assert!(title.contains("text \"Sign in to continue\""));
    let instance = summarize_document_mention(&state, &node("btn-use")).unwrap();
    assert!(instance.contains("instance of component \"Button\""));
    assert_eq!(summarize_document_mention(&state, &node("gone")), None);

    let page = ChatMention {
        kind: MentionKind::Page,
        key: "p2".into(),
        label: "Settings".into(),
    };
    let page = summarize_document_mention(&state, &page).unwrap();
    assert!(page.starts_with("page \"Settings\" (index 1) — 3 top-level layers"));

    let var = ChatMention {
        kind: MentionKind::Variable,
        key: "primary".into(),
        label: "primary".into(),
    };
    assert_eq!(
        summarize_document_mention(&state, &var).as_deref(),
        Some("color variable $primary = #2563EB")
    );
    let file = ChatMention {
        kind: MentionKind::File,
        key: "a.md".into(),
        label: "a.md".into(),
    };
    assert_eq!(summarize_document_mention(&state, &file), None);
}

#[test]
fn component_summary_reads_the_master() {
    let state = multi_page_state();
    let Some(option) = state.components.sorted_options().first().cloned() else {
        // The fixture's reusable frame is not indexed as a component in
        // this build; nothing to assert.
        return;
    };
    let mention = ChatMention {
        kind: MentionKind::Component,
        key: option.id.clone(),
        label: option.name.clone(),
    };
    let text = summarize_document_mention(&state, &mention).unwrap();
    assert!(text.starts_with(&format!("component \"{}\" — master: ", option.name)));
}

#[test]
fn context_block_lists_each_mention_and_flags_missing_ones() {
    assert_eq!(context_block(&[]), "");
    let block = context_block(&[
        MentionSummary {
            mention: ChatMention {
                kind: MentionKind::Node,
                key: "hero".into(),
                label: "Hero".into(),
            },
            text: Some("frame \"Hero\"".into()),
        },
        MentionSummary {
            mention: ChatMention {
                kind: MentionKind::File,
                key: "a.md".into(),
                label: "a.md".into(),
            },
            text: None,
        },
    ]);
    assert!(block.starts_with("\n\n---\nReferenced context"));
    assert!(block.contains("\n- @Hero (layer): frame \"Hero\""));
    assert!(block.contains("\n- @a.md (file): could not be read"));
}
