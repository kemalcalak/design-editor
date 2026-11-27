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

const ZWJ: char = '\u{200D}';

/// A character drawn as part of the one before it: the marks above, a
/// variation selector, an emoji skin tone or tag, or a zero-width joiner.
fn extends_cluster(ch: char) -> bool {
    joins_previous(ch)
        || ch == ZWJ
        || matches!(ch as u32,
            0x0300..=0x036F
                | 0x1AB0..=0x1AFF
                | 0x1DC0..=0x1DFF
                | 0x20D0..=0x20FF
                | 0xFE00..=0xFE0F
                | 0xFE20..=0xFE2F
                | 0x1F3FB..=0x1F3FF
                | 0xE0020..=0xE007F
                | 0xE0100..=0xE01EF)
}

/// The text split into the clusters a caret or a per-cluster measurement
/// must not cut: a base character with the marks, selectors and modifiers
/// drawn on it, a character joined to the next by a ZWJ or a virama (emoji
/// sequences, Devanagari conjuncts), and a Thai leading vowel with the
/// consonant after it. Yields `(byte offset, cluster)`. An approximation of
/// UAX #29 extended grapheme clusters that covers the scripts and emoji the
/// chrome meets, without a segmentation table.
pub(crate) fn clusters(text: &str) -> impl Iterator<Item = (usize, &str)> {
    let mut chars = text.char_indices().peekable();
    std::iter::from_fn(move || {
        let (start, first) = chars.next()?;
        let mut end = start + first.len_utf8();
        let mut glue = first == VIRAMA || leads_next(first);
        while let Some(&(index, next)) = chars.peek() {
            if !(glue || extends_cluster(next)) {
                break;
            }
            glue = next == ZWJ || next == VIRAMA;
            end = index + next.len_utf8();
            chars.next();
        }
        Some((start, &text[start..end]))
    })
}

#[cfg(test)]
mod cluster_tests {
    use super::clusters;

    fn split(text: &str) -> Vec<&str> {
        clusters(text).map(|(_, cluster)| cluster).collect()
    }

    #[test]
    fn marks_selectors_and_joiners_stay_on_their_base() {
        assert_eq!(split("ab"), vec!["a", "b"]);
        assert_eq!(split("e\u{301}x"), vec!["e\u{301}", "x"]);
        assert_eq!(split("\u{2764}\u{FE0F}!"), vec!["\u{2764}\u{FE0F}", "!"]);
        // Family emoji: four people joined by ZWJs, one cluster.
        let family = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}\u{200D}\u{1F466}";
        let text = format!("{family}a");
        assert_eq!(split(&text), vec![family, "a"]);
        assert_eq!(
            split("\u{1F44D}\u{1F3FD} "),
            vec!["\u{1F44D}\u{1F3FD}", " "]
        );
        // Devanagari conjunct with a matra (क्षि), Thai leading vowel (เก).
        assert_eq!(
            split("\u{915}\u{94D}\u{937}\u{93F}"),
            vec!["\u{915}\u{94D}\u{937}\u{93F}"]
        );
        assert_eq!(split("\u{E40}\u{E01}"), vec!["\u{E40}\u{E01}"]);
        let offsets: Vec<usize> = clusters("a\u{E9}").map(|(at, _)| at).collect();
        assert_eq!(offsets, vec![0, 1]);
    }
}
