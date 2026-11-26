//! The excerpt rule for text the chat quotes into a turn (#398): a
//! `@`-mentioned project file and an attached text file are cut the same
//! way, so the model sees the same shape whichever way a file arrived.

/// Characters of a text file quoted into a turn.
pub(crate) const MAX_EXCERPT_CHARS: usize = 4_000;
/// Lines of a text file quoted into a turn.
pub(crate) const MAX_EXCERPT_LINES: usize = 80;

/// The first lines of `text`, cut at [`MAX_EXCERPT_LINES`] lines or
/// [`MAX_EXCERPT_CHARS`] characters (whole lines only), and whether
/// anything was cut.
pub(crate) fn text_excerpt(text: &str) -> (String, bool) {
    let mut out = String::new();
    let mut chars = 0usize;
    for (index, line) in text.lines().enumerate() {
        let line_chars = line.chars().count();
        if index >= MAX_EXCERPT_LINES || chars + line_chars > MAX_EXCERPT_CHARS {
            return (out, true);
        }
        if index > 0 {
            out.push('\n');
        }
        out.push_str(line);
        chars += line_chars;
    }
    (out, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_text_is_kept_whole() {
        assert_eq!(text_excerpt("a\nb"), ("a\nb".to_string(), false));
        assert_eq!(text_excerpt(""), (String::new(), false));
    }

    #[test]
    fn long_text_is_cut_on_whole_lines() {
        let many: String = (0..100).map(|i| format!("{i}\n")).collect();
        let (body, cut) = text_excerpt(&many);
        assert!(cut);
        assert_eq!(body.lines().count(), MAX_EXCERPT_LINES);
        let wide = format!("short\n{}", "x".repeat(MAX_EXCERPT_CHARS));
        assert_eq!(text_excerpt(&wide), ("short".to_string(), true));
    }
}
