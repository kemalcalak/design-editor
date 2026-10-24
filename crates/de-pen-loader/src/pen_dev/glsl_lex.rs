//! A small lossless lexer for pen.dev GLSL ES 1.0 fragment shaders.
//!
//! The GLSL → SkSL translation ([`super::glsl`]) rewrites a token stream
//! rather than an AST: the two languages share almost all of their syntax,
//! so every token the translation leaves alone is written back verbatim
//! (comments and whitespace included) and the output stays readable.

/// What a token is. Text is kept in [`Token::text`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Kind {
    /// An identifier or keyword.
    Ident,
    /// A numeric literal (`1`, `1.0`, `.5`, `2e-3`, `0x1F`).
    Number,
    /// Spaces, tabs and newlines.
    Space,
    /// A `//` or `/* */` comment.
    Comment,
    /// A whole preprocessor line (`#version 100`).
    Directive,
    /// Any other single character (`(`, `;`, `.`, `+`, …).
    Punct,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Token {
    pub(super) kind: Kind,
    pub(super) text: String,
}

impl Token {
    pub(super) fn new(kind: Kind, text: impl Into<String>) -> Self {
        Self {
            kind,
            text: text.into(),
        }
    }

    pub(super) fn is_trivia(&self) -> bool {
        matches!(self.kind, Kind::Space | Kind::Comment)
    }

    pub(super) fn is_ident(&self, name: &str) -> bool {
        self.kind == Kind::Ident && self.text == name
    }

    pub(super) fn is_punct(&self, c: char) -> bool {
        self.kind == Kind::Punct && self.text.len() == c.len_utf8() && self.text.starts_with(c)
    }
}

/// Split `src` into tokens; concatenating their text gives `src` back.
pub(super) fn lex(src: &str) -> Vec<Token> {
    let bytes = src.as_bytes();
    let mut tokens = Vec::new();
    let mut i = 0;
    // A `#` starts a directive only as the first non-space on its line.
    let mut line_start = true;
    while i < bytes.len() {
        let start = i;
        let b = bytes[i];
        let kind = if b.is_ascii_whitespace() {
            while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                if bytes[i] == b'\n' {
                    line_start = true;
                }
                i += 1;
            }
            Kind::Space
        } else if src[i..].starts_with("//") {
            i = src[i..].find('\n').map_or(bytes.len(), |end| i + end);
            Kind::Comment
        } else if src[i..].starts_with("/*") {
            i = src[i + 2..]
                .find("*/")
                .map_or(bytes.len(), |end| i + 2 + end + 2);
            Kind::Comment
        } else if b == b'#' && line_start {
            // A directive runs to the end of the line, `\` continuations
            // included.
            loop {
                match src[i..].find('\n') {
                    Some(end) if src[i..i + end].trim_end().ends_with('\\') => i += end + 1,
                    Some(end) => {
                        i += end;
                        break;
                    }
                    None => {
                        i = bytes.len();
                        break;
                    }
                }
            }
            Kind::Directive
        } else if b.is_ascii_alphabetic() || b == b'_' {
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            Kind::Ident
        } else if b.is_ascii_digit()
            || (b == b'.' && bytes.get(i + 1).is_some_and(u8::is_ascii_digit))
        {
            i = number_end(bytes, i);
            Kind::Number
        } else {
            i += src[i..].chars().next().map_or(1, char::len_utf8);
            Kind::Punct
        };
        if kind != Kind::Space {
            line_start = false;
        }
        tokens.push(Token::new(kind, &src[start..i]));
    }
    tokens
}

fn number_end(bytes: &[u8], mut i: usize) -> usize {
    if bytes[i] == b'0' && matches!(bytes.get(i + 1), Some(b'x' | b'X')) {
        i += 2;
        while i < bytes.len() && bytes[i].is_ascii_hexdigit() {
            i += 1;
        }
        return i;
    }
    while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
        i += 1;
    }
    if matches!(bytes.get(i), Some(b'e' | b'E')) {
        let mut j = i + 1;
        if matches!(bytes.get(j), Some(b'+' | b'-')) {
            j += 1;
        }
        if bytes.get(j).is_some_and(u8::is_ascii_digit) {
            i = j;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
        }
    }
    // GLSL ES 1.0 has no integer suffixes; a stray `f` (`1.0f`) is not
    // valid there either, so it is left to fail as an identifier.
    i
}

/// Index of the next non-trivia token at or after `i`.
pub(super) fn next_significant(tokens: &[Token], mut i: usize) -> Option<usize> {
    while i < tokens.len() {
        if !tokens[i].is_trivia() {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// Index of the previous non-trivia token before `i`.
pub(super) fn prev_significant(tokens: &[Token], i: usize) -> Option<usize> {
    (0..i).rev().find(|&j| !tokens[j].is_trivia())
}

/// The concatenated source text of `tokens`.
pub(super) fn text(tokens: &[Token]) -> String {
    tokens.iter().map(|token| token.text.as_str()).collect()
}
