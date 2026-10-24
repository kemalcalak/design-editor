//! Greedy line wrap for text with an explicit width — the one wrap the
//! canvas painter, the inline text editor and the layout measure share
//! (#186), so a `fit_content` text box is sized for exactly the lines the
//! canvas paints in it.
//!
//! The caller supplies the line measure (the node's face, size and letter
//! spacing), so this stays font-free and wasm32-clean: the painter measures
//! through its `RenderBackend`, layout through jian-skia's font resolver,
//! which is the same glyph-advance model on native.

/// Wrap `text` to lines no wider than `max_width` as `measure` reports
/// them. Splits on `\n` first, then breaks CJK text between any two
/// characters and Latin text at spaces; a space that would overflow the
/// line is dropped at the break, and a word wider than the whole line
/// stays on a line of its own. Empty segments survive as blank lines so
/// authored paragraph breaks keep their gap. A `max_width` of zero or less
/// disables wrapping. Always returns at least one line.
pub fn wrap_text_measured(
    text: &str,
    max_width: f32,
    measure: &mut dyn FnMut(&str) -> f32,
) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for segment in text.split('\n') {
        if max_width <= 0.0 || measure(segment) <= max_width {
            out.push(segment.to_string());
            continue;
        }
        wrap_segment(segment, max_width, measure, &mut out);
    }
    if out.is_empty() {
        out.push(String::new());
    }
    out
}

/// Wrap a single newline-free segment.
fn wrap_segment(
    segment: &str,
    max_width: f32,
    measure: &mut dyn FnMut(&str) -> f32,
    out: &mut Vec<String>,
) {
    let chars: Vec<char> = segment.chars().collect();
    let mut current = String::new();
    let mut probe = String::with_capacity(segment.len());
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        if is_cjk(ch) {
            let mut ch_buf = [0; 4];
            build_probe(&mut probe, &current, ch.encode_utf8(&mut ch_buf));
            if measure(&probe) > max_width && !current.is_empty() {
                out.push(std::mem::take(&mut current));
                current.push(ch);
            } else {
                std::mem::swap(&mut current, &mut probe);
            }
            i += 1;
        } else if ch == ' ' {
            build_probe(&mut probe, &current, " ");
            if measure(&probe) > max_width && !current.is_empty() {
                out.push(std::mem::take(&mut current));
            } else {
                std::mem::swap(&mut current, &mut probe);
            }
            i += 1;
        } else {
            let mut word = String::new();
            while i < chars.len() && chars[i] != ' ' && !is_cjk(chars[i]) {
                word.push(chars[i]);
                i += 1;
            }
            build_probe(&mut probe, &current, &word);
            if measure(&probe) > max_width && !current.is_empty() {
                out.push(std::mem::take(&mut current));
                current.push_str(&word);
            } else {
                std::mem::swap(&mut current, &mut probe);
            }
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
}

fn build_probe(probe: &mut String, current: &str, suffix: &str) {
    probe.clear();
    probe.push_str(current);
    probe.push_str(suffix);
}

/// Codepoints that wrap per character (no inter-word spaces): CJK
/// ideographs, kana, Hangul syllables, CJK punctuation and full-width
/// forms. The word shapers split text at the same characters; copies that
/// must stay equal: jian-skia `shaped_words::is_cjk` and `isShapeCjk` in
/// `de-host-web/src/op_ck_bridge.js`.
pub fn is_cjk(ch: char) -> bool {
    let c = ch as u32;
    (0x4e00..=0x9fff).contains(&c)
        || (0x3400..=0x4dbf).contains(&c)
        || (0x3040..=0x30ff).contains(&c)
        || (0xac00..=0xd7af).contains(&c)
        || (0x3000..=0x303f).contains(&c)
        || (0xff00..=0xffef).contains(&c)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 10 px per char — exact arithmetic.
    fn wrap(text: &str, max_width: f32) -> Vec<String> {
        wrap_text_measured(text, max_width, &mut |s: &str| {
            s.chars().count() as f32 * 10.0
        })
    }

    #[test]
    fn breaks_at_the_exact_width() {
        // "hello world" = 110 px: fits in 110, wraps in 109.
        assert_eq!(wrap("hello world", 110.0), vec!["hello world"]);
        assert_eq!(wrap("hello world", 109.0), vec!["hello ", "world"]);
    }

    #[test]
    fn drops_the_space_that_overflows_at_a_break() {
        // "hello" = 50 fits, "hello " = 60 does not: the space goes.
        assert_eq!(wrap("hello world", 50.0), vec!["hello", "world"]);
    }

    #[test]
    fn cjk_breaks_between_characters() {
        assert_eq!(wrap("设计工具", 20.0), vec!["设计", "工具"]);
    }

    #[test]
    fn keeps_blank_lines_and_an_over_long_word() {
        assert_eq!(wrap("a\n\nb", 100.0), vec!["a", "", "b"]);
        assert_eq!(wrap("abcdefgh x", 30.0), vec!["abcdefgh", "x"]);
    }

    #[test]
    fn zero_width_does_not_wrap() {
        assert_eq!(wrap("hello world", 0.0), vec!["hello world"]);
    }

    #[test]
    fn probe_builder_reuses_existing_string_allocation() {
        let mut probe = String::with_capacity(64);
        let ptr = probe.as_ptr();

        build_probe(&mut probe, "hello", " world");

        assert_eq!(probe, "hello world");
        assert_eq!(
            probe.as_ptr(),
            ptr,
            "probe construction should reuse the existing String allocation"
        );
    }
}
