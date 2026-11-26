//! Palette ranking (#395): how well a typed query matches a label.
//!
//! Higher is better; `None` is no match. Exact beats prefix beats a word
//! start beats a substring beats an in-order subsequence (`dupl` →
//! "Duplicate", `sva` → "Save As"). Within a tier a shorter label wins, so
//! "Save" ranks over "Save As" for `save`.

/// Score `candidate` against `query`, which must already be lowercased and
/// trimmed. An empty query matches everything with score 0.
pub fn fuzzy_score(query: &str, candidate: &str) -> Option<u32> {
    if query.is_empty() {
        return Some(0);
    }
    let lower = candidate.to_lowercase();
    let length_penalty = lower.chars().count().min(200) as u32;
    if lower == query {
        return Some(10_000);
    }
    if lower.starts_with(query) {
        return Some(8_000 - length_penalty);
    }
    if let Some(pos) = word_start_position(&lower, query) {
        return Some(6_000 - (pos.min(200) as u32) * 4 - length_penalty);
    }
    if let Some(pos) = lower.find(query) {
        return Some(4_000 - (pos.min(200) as u32) * 4 - length_penalty);
    }
    subsequence_score(&lower, query).map(|score| score.saturating_sub(length_penalty))
}

/// Byte offset of `query` where it starts a word of `text`.
fn word_start_position(text: &str, query: &str) -> Option<usize> {
    text.match_indices(query).map(|(pos, _)| pos).find(|&pos| {
        text[..pos]
            .chars()
            .next_back()
            .is_some_and(|c| !c.is_alphanumeric())
    })
}

/// Every query char in order. Consecutive runs and word starts earn more,
/// so `sva` prefers "Save As" over a label where the letters are scattered.
fn subsequence_score(text: &str, query: &str) -> Option<u32> {
    let mut want = query.chars().filter(|c| !c.is_whitespace()).peekable();
    want.peek()?;
    let mut score: u32 = 1_000;
    let mut previous_matched = false;
    let mut previous_char: Option<char> = None;
    for c in text.chars() {
        let Some(&next) = want.peek() else {
            break;
        };
        if c == next {
            want.next();
            let word_start = previous_char.is_none_or(|p| !p.is_alphanumeric());
            score += if word_start {
                60
            } else if previous_matched {
                40
            } else {
                10
            };
            previous_matched = true;
        } else {
            previous_matched = false;
        }
        previous_char = Some(c);
    }
    want.peek().is_none().then_some(score.min(3_999))
}
