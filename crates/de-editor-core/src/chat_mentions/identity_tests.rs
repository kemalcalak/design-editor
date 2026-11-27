//! Identity-based mention matching: tokens are whole, unique per target.

use super::*;
use crate::find_layers::tests::multi_page_state;

fn node(key: &str, label: &str) -> ChatMention {
    ChatMention {
        kind: MentionKind::Node,
        key: key.into(),
        label: label.into(),
    }
}

#[test]
fn a_token_does_not_match_inside_a_longer_one() {
    let mentions = [node("a", "Hero"), node("b", "Hero card")];
    assert_eq!(live_mentions("fix @Hero card", &mentions), [false, true]);
    assert_eq!(
        live_mentions("fix @Hero and @Hero card", &mentions),
        [true, true]
    );
    // Followed by a word character: a different name, not this token.
    assert_eq!(live_mentions("@Heroes", &mentions[..1]), [false]);
    assert_eq!(live_mentions("see @Hero.", &mentions[..1]), [true]);
}

#[test]
fn cjk_punctuation_and_dashes_end_a_token() {
    let mentions = [node("a", "Hero")];
    for draft in [
        "改一下 @Hero，谢谢",
        "@Hero。",
        "@Hero—then",
        "@Hero、@x",
        "「@Hero」",
    ] {
        assert_eq!(live_mentions(draft, &mentions), [true], "{draft}");
    }
    // Letters of any script still continue it.
    assert_eq!(live_mentions("@Hero按钮", &mentions), [false]);
    assert_eq!(live_mentions("@Heroé", &mentions), [false]);
    // So do the characters names and paths are made of.
    assert_eq!(live_mentions("@Hero-card", &mentions), [false]);
}

#[test]
fn the_draft_fingerprint_follows_text_and_staged_count() {
    let a = identity::draft_fingerprint("fix @Hero", 1);
    assert_eq!(a, identity::draft_fingerprint("fix @Hero", 1));
    assert_ne!(a, identity::draft_fingerprint("fix @Her", 1));
    assert_ne!(a, identity::draft_fingerprint("fix @Hero", 2));
}

#[test]
fn an_unchanged_draft_skips_the_token_check() {
    let mut state = multi_page_state();
    state.chat.pending_mentions = vec![node("a", "Hero")];
    state.chat.input.set_text("@Hero go");
    state.sync_chat_mentions();
    let checked = state.chat.mentions_checked_draft;
    assert!(checked.is_some());
    // A mention staged behind the sync's back is only dropped once the
    // draft (or the staged count) moves.
    state.chat.pending_mentions[0] = node("a", "Gone");
    state.chat.input.set_text("@Hero go");
    assert!(!state.sync_chat_mentions());
    assert_eq!(state.chat.pending_mentions.len(), 1);
    state.chat.input.set_text("@Hero go!");
    assert!(state.sync_chat_mentions());
    assert!(state.chat.pending_mentions.is_empty());
}

#[test]
fn same_named_targets_get_numbered_labels() {
    let staged = [node("a", "Hero")];
    assert_eq!(unique_label(&node("b", "Hero"), &staged), "Hero#2");
    // The same target keeps its own label.
    assert_eq!(unique_label(&node("a", "Hero"), &staged), "Hero");
    let staged = [node("a", "Hero"), node("b", "Hero#2")];
    assert_eq!(unique_label(&node("c", "Hero"), &staged), "Hero#3");
    // `@Hero` is not live inside `@Hero#2`.
    assert_eq!(live_mentions("@Hero#2 x", &staged), [false, true]);
}

#[test]
fn deleting_one_of_two_same_named_tokens_unstages_only_that_one() {
    let mut state = multi_page_state();
    state.chat.focused = true;
    state.chat.pending_mentions = vec![node("a", "Hero"), node("b", "Hero#2")];
    state.chat.input.set_text("@Hero @Hero#2 swap");
    state.chat.input.set_caret(0, 0);
    assert!(!state.sync_chat_mentions());
    state.chat.input.set_text("@Hero swap");
    assert!(state.sync_chat_mentions());
    assert_eq!(state.chat.pending_mentions, [node("a", "Hero")]);
}

#[test]
fn removing_a_mention_drops_its_own_token() {
    let mut state = multi_page_state();
    state.chat.pending_mentions = vec![node("a", "Hero"), node("b", "Hero#2")];
    state.chat.input.set_text("@Hero#2 then @Hero now");
    assert!(state.remove_chat_mention(0, 0));
    assert_eq!(state.chat.input.text(), "@Hero#2 then now");
    assert_eq!(state.chat.pending_mentions, [node("b", "Hero#2")]);
}

#[test]
fn page_summary_follows_the_page_id_after_a_reorder() {
    let mut state = multi_page_state();
    let page = ChatMention {
        kind: MentionKind::Page,
        key: "p2".into(),
        label: "Settings".into(),
    };
    let before = summarize_document_mention(&state, &page).unwrap();
    if let Some(pages) = state.doc.pages.as_mut() {
        pages.reverse();
    }
    let after = summarize_document_mention(&state, &page).unwrap();
    assert!(before.contains("Settings") && after.contains("Settings"));
}
