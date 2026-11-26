//! Finding the `@query` / `/query` the caret is typing.

/// A query is abandoned past this many bytes: nobody types a 64-byte
/// layer name after `@`, and an unbounded scan would walk a pasted essay.
const MAX_QUERY_BYTES: usize = 64;

/// The character that opens the layer / file popup.
pub const MENTION_TRIGGER: char = '@';
/// The character that opens the skill popup (#398).
pub const SKILL_TRIGGER: char = '/';

/// The token the caret sits at the end of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MentionQuery<'a> {
    /// Byte offset of the trigger character.
    pub start: usize,
    /// [`MENTION_TRIGGER`] or [`SKILL_TRIGGER`].
    pub trigger: char,
    /// What follows the trigger, up to the caret (no whitespace).
    pub query: &'a str,
}

/// The `@` / `/` token being typed at `caret`, or `None`.
///
/// A trigger counts when it opens the text or follows whitespace (so an
/// email address `ana@example.com` or a path `docs/brief.md` never opens
/// the popup) and nothing between it and the caret is whitespace. A `/`
/// query stops at another `/` for the same reason. Does not allocate: it
/// runs on every popup sync.
pub fn active_mention_query(text: &str, caret: usize) -> Option<MentionQuery<'_>> {
    if caret > text.len() || !text.is_char_boundary(caret) {
        return None;
    }
    let before = &text[..caret];
    let lower_bound = caret.saturating_sub(MAX_QUERY_BYTES + 1);
    let mut saw_slash = false;
    for (offset, ch) in before.char_indices().rev() {
        if offset < lower_bound || ch.is_whitespace() {
            return None;
        }
        if ch == MENTION_TRIGGER || ch == SKILL_TRIGGER {
            let opens = before[..offset]
                .chars()
                .next_back()
                .is_none_or(char::is_whitespace);
            if !opens {
                if ch == SKILL_TRIGGER {
                    // A path separator: keep looking for an `@` before it
                    // (`@docs/brief.md` is a file mention being typed).
                    saw_slash = true;
                    continue;
                }
                return None;
            }
            if ch == SKILL_TRIGGER && saw_slash {
                return None;
            }
            return Some(MentionQuery {
                start: offset,
                trigger: ch,
                query: &before[offset + 1..],
            });
        }
    }
    None
}
