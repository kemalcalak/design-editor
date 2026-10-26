//! Line breaking shared by the chrome's wrapped copy (Home import cards,
//! the "What's new" window): break at the last space that fits, else
//! between characters (CJK and Thai carry no spaces), never stranding a
//! combining mark or a Thai leading vowel at a line edge.

/// Wrap `text` into lines no wider than `max_w` as `measure` reports them.
/// A run that cannot break (one cluster wider than the line) stays whole.
pub(crate) fn wrap_lines(
    text: &str,
    max_w: f32,
    mut measure: impl FnMut(&str) -> f32,
) -> Vec<String> {
    let mut lines = Vec::new();
    let mut rest = text.trim();
    while !rest.is_empty() {
        if measure(rest) <= max_w {
            lines.push(rest.to_string());
            break;
        }
        let end = break_point(rest, max_w, &mut measure);
        lines.push(rest[..end].trim_end().to_string());
        rest = rest[end..].trim_start();
    }
    lines
}

/// The byte index to end this line at: after the last space that still
/// fits, else after as many characters as fit (at least one cluster). A
/// character break never strands a combining mark (Thai vowels and tone
/// marks, Devanagari matras) away from its base.
pub(crate) fn break_point(text: &str, max_w: f32, measure: &mut impl FnMut(&str) -> f32) -> usize {
    let mut last_fit = None;
    let mut last_space = None;
    let mut chars = text.char_indices().peekable();
    while let Some((index, ch)) = chars.next() {
        let end = index + ch.len_utf8();
        // Everything before this character fits, so ending the line at
        // this space (dropped by the caller's trim) fits too.
        if ch == ' ' && index > 0 {
            last_space = Some(end);
        }
        if measure(&text[..end]) > max_w {
            break;
        }
        let next_joins = chars.peek().is_some_and(|&(_, next)| joins_previous(next));
        if !next_joins && ch != VIRAMA && !leads_next(ch) {
            last_fit = Some(end);
        }
    }
    last_space
        .or(last_fit)
        .unwrap_or_else(|| first_cluster_end(text))
}

const VIRAMA: char = '\u{094D}';

/// A mark drawn on the character before it, never at a line start.
fn joins_previous(ch: char) -> bool {
    matches!(ch as u32,
        0x0300..=0x036F
            | 0x0900..=0x0903
            | 0x093A..=0x093C
            | 0x093E..=0x094F
            | 0x0951..=0x0957
            | 0x0962..=0x0963
            | 0x0E31
            | 0x0E33..=0x0E3A
            | 0x0E47..=0x0E4E)
}

/// A Thai vowel written before the consonant it follows in speech
/// (เ แ โ ใ ไ): it belongs to the next character, never at a line end.
fn leads_next(ch: char) -> bool {
    matches!(ch as u32, 0x0E40..=0x0E44)
}

/// The end of the first character plus any marks that join it.
fn first_cluster_end(text: &str) -> usize {
    let mut chars = text.char_indices();
    let mut end = chars.next().map_or(text.len(), |(_, ch)| ch.len_utf8());
    for (index, ch) in chars {
        if !joins_previous(ch) {
            break;
        }
        end = index + ch.len_utf8();
    }
    end
}
