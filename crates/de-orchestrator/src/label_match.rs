//! Nav label ↔ screen / section name matching, for every language (#223).
//!
//! Nothing here knows a word of any language. Text is compared after
//! Unicode lower-casing and canonical decomposition with the generic
//! combining diacritics (U+0300–U+036F) dropped, so "Café" meets "cafe"
//! and "İletişim" meets "iletişim"; marks that are letters' vowel signs in
//! their own scripts (Thai, Devanagari, …) are kept. Words split on
//! anything that is neither a letter, a digit nor such a mark.
//!
//! Strength, strongest first:
//! - [`MatchStrength::Exact`]: the whole normalized texts are equal;
//! - [`MatchStrength::Word`]: one whole text starts the other ("Pricing"
//!   / "Pricing plans"), or they share a whole word ("Trips" / "Wander —
//!   Trips");
//! - [`MatchStrength::Stem`]: two words share a stem, the longest common
//!   prefix covering at least 4 characters and three quarters of the
//!   shorter word ("Teams" / "Team page", inflected forms in any
//!   alphabet), or, for scripts written without spaces (Han, kana, Thai,
//!   Lao, Khmer, Myanmar), one text contains the other ("价格" / "我们的价格").
//!
//! Callers pick the strongest match and bind nothing on a tie.

use unicode_normalization::UnicodeNormalization;

/// How strongly a label names a target; orders strongest first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum MatchStrength {
    Exact,
    Word,
    Stem,
}

/// Shortest shared prefix that counts as a stem, in characters.
const STEM_MIN_CHARS: usize = 4;

/// Lower-cased, diacritic-free letters, digits and script marks of `s`.
pub(crate) fn normalize_label(s: &str) -> String {
    s.nfd()
        .filter(|&c| !is_generic_diacritic(c) && is_word_char(c))
        .flat_map(char::to_lowercase)
        .collect()
}

/// `s` split into normalized words. An apostrophe joins rather than
/// splits ("Today's" is one word, "todays"): split, its one-letter halves
/// would match any other label with an apostrophe.
pub(crate) fn label_tokens(s: &str) -> Vec<String> {
    s.nfd()
        .collect::<String>()
        .split(|c: char| !is_word_char(c) && !is_apostrophe(c))
        .map(normalize_label)
        .filter(|t| !t.is_empty())
        .collect()
}

/// How strongly `a` and `b` name the same thing; `None` when they don't.
/// Takes raw text: word boundaries are found before normalizing.
pub(crate) fn match_strength(a: &str, b: &str) -> Option<MatchStrength> {
    let (na, nb) = (normalize_label(a), normalize_label(b));
    if na.is_empty() || nb.is_empty() {
        return None;
    }
    if na == nb {
        return Some(MatchStrength::Exact);
    }
    if na.starts_with(&nb) || nb.starts_with(&na) {
        return Some(MatchStrength::Word);
    }
    let (ta, tb) = (label_tokens(a), label_tokens(b));
    if ta.iter().any(|t| tb.contains(t)) {
        return Some(MatchStrength::Word);
    }
    let stem = ta.iter().any(|x| tb.iter().any(|y| shares_stem(x, y)));
    let spaceless = (na.chars().any(is_spaceless_script) || nb.chars().any(is_spaceless_script))
        && na.chars().count().min(nb.chars().count()) >= 2
        && (na.contains(&nb) || nb.contains(&na));
    (stem || spaceless).then_some(MatchStrength::Stem)
}

/// Exact or word matches only: the strict rule the generation passes use
/// (`wire_nav_tabs`, `nav_issues`, `unify_shared_nav`), first match wins.
pub(crate) fn labels_match(a: &str, b: &str) -> bool {
    match_strength(a, b).is_some_and(|s| s <= MatchStrength::Word)
}

/// The one target `label` names most strongly. `targets` yields
/// `(name, rank, value)`; among the strongest matches the lowest `rank`
/// wins (callers rank pages before sections, shallow before deep), and a
/// tie that survives both binds nothing.
pub(crate) fn best_match<'a, T>(
    label: &str,
    targets: impl IntoIterator<Item = (&'a str, usize, T)>,
) -> Option<T> {
    let mut best: Option<((MatchStrength, usize), T)> = None;
    let mut tied = false;
    for (name, rank, value) in targets {
        let Some(strength) = match_strength(label, name) else {
            continue;
        };
        let key = (strength, rank);
        match &best {
            Some((best_key, _)) if key > *best_key => {}
            Some((best_key, _)) if key == *best_key => tied = true,
            _ => {
                best = Some((key, value));
                tied = false;
            }
        }
    }
    best.filter(|_| !tied).map(|(_, value)| value)
}

fn shares_stem(a: &str, b: &str) -> bool {
    let common = a.chars().zip(b.chars()).take_while(|(x, y)| x == y).count();
    let shorter = a.chars().count().min(b.chars().count());
    common >= STEM_MIN_CHARS && common * 4 >= shorter * 3
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || unicode_normalization::char::is_combining_mark(c)
}

fn is_apostrophe(c: char) -> bool {
    matches!(c, '\'' | '\u{2019}' | '\u{02BC}')
}

fn is_generic_diacritic(c: char) -> bool {
    ('\u{0300}'..='\u{036F}').contains(&c)
}

/// Scripts written without spaces between words.
fn is_spaceless_script(c: char) -> bool {
    matches!(c,
        '\u{3040}'..='\u{30FF}'   // Hiragana, Katakana
        | '\u{3400}'..='\u{4DBF}' // Han extension A
        | '\u{4E00}'..='\u{9FFF}' // Han
        | '\u{0E00}'..='\u{0EFF}' // Thai, Lao
        | '\u{1000}'..='\u{109F}' // Myanmar
        | '\u{1780}'..='\u{17FF}' // Khmer
    )
}

#[cfg(test)]
#[path = "label_match_tests.rs"]
mod tests;
