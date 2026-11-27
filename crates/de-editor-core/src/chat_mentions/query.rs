//! Finding the `@query` the caret is typing.

/// A query is abandoned past this many bytes: nobody types a 64-byte
/// layer name after `@`, and an unbounded scan would walk a pasted essay.
const MAX_QUERY_BYTES: usize = 64;

/// The `@` token the caret sits at the end of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MentionQuery<'a> {
    /// Byte offset of the `@`.
    pub start: usize,
    /// What follows the `@`, up to the caret (no whitespace).
    pub query: &'a str,
}

/// The mention being typed at `caret`, or `None`.
///
/// An `@` counts when it opens the text or follows whitespace (so an email
/// address such as `ana@example.com` never opens the popup) and nothing
/// between it and the caret is whitespace. Does not allocate: it runs on
/// every popup sync.
pub fn active_mention_query(text: &str, caret: usize) -> Option<MentionQuery<'_>> {
    if caret > text.len() || !text.is_char_boundary(caret) {
        return None;
    }
    let before = &text[..caret];
    let lower_bound = caret.saturating_sub(MAX_QUERY_BYTES + 1);
    for (offset, ch) in before.char_indices().rev() {
        if offset < lower_bound {
            return None;
        }
        if ch.is_whitespace() {
            return None;
        }
        if ch == '@' {
            let opens = before[..offset]
                .chars()
                .next_back()
                .is_none_or(char::is_whitespace);
            if !opens {
                return None;
            }
            return Some(MentionQuery {
                start: offset,
                query: &before[offset + 1..],
            });
        }
    }
    None
}
