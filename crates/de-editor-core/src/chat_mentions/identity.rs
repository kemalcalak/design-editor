//! Which staged mention a token in the draft stands for.
//!
//! A mention is identified by its token, never by a loose substring test:
//! every staged mention's token is unique (a second layer called "Hero" is
//! written `@Hero#2`, see [`unique_label`]), and a token only counts where
//! the draft holds it whole — followed by the end of the text, whitespace or
//! punctuation — and not inside a longer token already matched. So `@Hero`
//! is neither found inside `@Hero#2` nor inside `@Hero card`.

use super::ChatMention;

/// Characters that may follow a token: anything else continues it.
fn ends_token(next: Option<char>) -> bool {
    next.is_none_or(|c| c.is_whitespace() || ",.;:!?)]}\"'".contains(c))
}

/// Byte span of the first whole occurrence of `token` in `text` that does
/// not overlap `taken`.
pub(crate) fn find_token(
    text: &str,
    token: &str,
    taken: &[(usize, usize)],
) -> Option<(usize, usize)> {
    text.match_indices(token)
        .map(|(start, _)| (start, start + token.len()))
        .find(|&(start, end)| {
            ends_token(text[end..].chars().next())
                && !taken.iter().any(|&(a, b)| start < b && a < end)
        })
}

/// For each of `mentions`, the byte span of its token in `text`, or `None`
/// when the user typed it away. Longer tokens claim their span first, so a
/// shorter one never matches inside them.
pub fn mention_spans(text: &str, mentions: &[ChatMention]) -> Vec<Option<(usize, usize)>> {
    let tokens: Vec<String> = mentions.iter().map(ChatMention::token).collect();
    let mut order: Vec<usize> = (0..mentions.len()).collect();
    order.sort_by_key(|&i| std::cmp::Reverse(tokens[i].len()));
    let mut taken: Vec<(usize, usize)> = Vec::new();
    let mut spans = vec![None; mentions.len()];
    for i in order {
        if let Some(span) = find_token(text, &tokens[i], &taken) {
            taken.push(span);
            spans[i] = Some(span);
        }
    }
    spans
}

/// For each of `mentions`, whether its token is still in `text`.
pub fn live_mentions(text: &str, mentions: &[ChatMention]) -> Vec<bool> {
    mention_spans(text, mentions)
        .into_iter()
        .map(|span| span.is_some())
        .collect()
}

/// Keep only the mentions whose token is still in `text`. True when any
/// was dropped.
pub fn retain_live(text: &str, mentions: &mut Vec<ChatMention>) -> bool {
    let live = live_mentions(text, mentions);
    if live.iter().all(|l| *l) {
        return false;
    }
    let mut keep = live.into_iter();
    mentions.retain(|_| keep.next().unwrap_or(false));
    true
}

/// The label `mention` should be staged under next to `staged`: its own when
/// no other staged mention uses that token, else `Label#2`, `Label#3`, … so
/// every token names exactly one target.
pub fn unique_label(mention: &ChatMention, staged: &[ChatMention]) -> String {
    let clashes = |label: &str| {
        staged.iter().any(|other| {
            (other.kind, other.key.as_str()) != (mention.kind, mention.key.as_str())
                && other.kind.trigger() == mention.kind.trigger()
                && other.label == label
        })
    };
    if !clashes(&mention.label) {
        return mention.label.clone();
    }
    (2..)
        .map(|n| format!("{}#{n}", mention.label))
        .find(|label| !clashes(label))
        .unwrap_or_default()
}
