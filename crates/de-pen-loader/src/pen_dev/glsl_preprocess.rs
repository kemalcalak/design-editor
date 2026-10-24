//! The GLSL ES 1.0 preprocessor of the pen.dev shader import (#165).
//!
//! SkSL has no preprocessor, so the translation ([`super::glsl`]) runs this
//! one first: conditional blocks (`#if` / `#ifdef` / `#ifndef` / `#elif` /
//! `#else` / `#endif`) keep only the active branch, `#define` (object-like
//! and function-like) and `#undef` expand as in C, positionally, and the
//! directives SkSL has no use for (`#version`, `#extension`, `#pragma`,
//! `#line`) are dropped. `#error` in an active branch fails the shader, as
//! it would fail it on WebGL. A directive keeps its lines (as empty ones)
//! so compile errors still point at the right line.
//!
//! `GL_ES`, `__VERSION__` (100) and `GL_FRAGMENT_PRECISION_HIGH` are
//! predefined, as in a WebGL 1.0 fragment shader. `#` and `##` are not
//! supported (GLSL ES 1.0 reserves them).

use std::collections::{HashMap, HashSet};

use super::glsl_error::GlslError;
use super::glsl_lex::{lex, text, Kind, Token};

/// Expansion depth after which a macro is assumed to recurse forever.
const MAX_EXPANSION_DEPTH: usize = 64;

/// Tokens the whole expansion may produce: the input is an untrusted file,
/// and nested macros can multiply (`#define B A A`, `#define C B B`, …).
const MAX_EXPANDED_TOKENS: usize = 1 << 20;

#[derive(Clone, Debug)]
struct Macro {
    /// `Some` for a function-like macro.
    params: Option<Vec<String>>,
    body: Vec<Token>,
}

/// One open conditional block.
#[derive(Clone, Copy)]
struct Cond {
    /// Whether the enclosing block is active.
    parent: bool,
    /// Whether the current branch is active.
    active: bool,
    /// Whether some branch of this block was taken already.
    taken: bool,
    /// Whether `#else` was seen.
    in_else: bool,
}

/// Run the preprocessor over `tokens`.
pub(super) fn preprocess(tokens: Vec<Token>) -> Result<Vec<Token>, GlslError> {
    let mut defines: HashMap<String, Macro> = HashMap::new();
    for (name, value) in [
        ("GL_ES", "1"),
        ("__VERSION__", "100"),
        ("GL_FRAGMENT_PRECISION_HIGH", "1"),
    ] {
        defines.insert(
            name.to_owned(),
            Macro {
                params: None,
                body: lex(value),
            },
        );
    }
    let mut out = Vec::with_capacity(tokens.len());
    let mut pending: Vec<Token> = Vec::new();
    let mut conds: Vec<Cond> = Vec::new();
    let mut budget = MAX_EXPANDED_TOKENS;
    for token in tokens {
        let active = conds.last().is_none_or(|cond| cond.active);
        if token.kind != Kind::Directive {
            if active {
                pending.push(token);
            } else if token.text.contains('\n') {
                // Keep the lines of a dropped branch.
                let lines = token.text.matches('\n').count();
                pending.push(Token::new(Kind::Space, "\n".repeat(lines)));
            }
            continue;
        }
        // Macros expand with the definitions in force at this point.
        out.extend(expand(
            &std::mem::take(&mut pending),
            &defines,
            &mut budget,
        )?);
        let line = directive_text(&token.text);
        let (name, rest) = split_directive(&line);
        match name {
            "ifdef" | "ifndef" | "if" => {
                let value = if !active {
                    false
                } else if name == "if" {
                    evaluate(rest, &defines, &mut budget)
                        .ok_or_else(|| GlslError::Directive(line.clone()))?
                } else {
                    let defined = defines.contains_key(rest.trim());
                    defined == (name == "ifdef")
                };
                conds.push(Cond {
                    parent: active,
                    active: active && value,
                    taken: value,
                    in_else: false,
                });
            }
            "elif" => {
                let cond = conds
                    .last_mut()
                    .filter(|cond| !cond.in_else)
                    .ok_or_else(|| GlslError::Directive(line.clone()))?;
                if cond.parent && !cond.taken {
                    let value = evaluate(rest, &defines, &mut budget)
                        .ok_or_else(|| GlslError::Directive(line.clone()))?;
                    cond.active = value;
                    cond.taken = value;
                } else {
                    cond.active = false;
                }
            }
            "else" => {
                let cond = conds
                    .last_mut()
                    .filter(|cond| !cond.in_else)
                    .ok_or_else(|| GlslError::Directive(line.clone()))?;
                cond.active = cond.parent && !cond.taken;
                cond.taken = true;
                cond.in_else = true;
            }
            "endif" => {
                conds
                    .pop()
                    .ok_or_else(|| GlslError::Directive(line.clone()))?;
            }
            _ if !active => {}
            "version" | "extension" | "pragma" | "line" | "" => {}
            "define" => {
                let (macro_name, definition) =
                    define(rest).ok_or_else(|| GlslError::Directive(line.clone()))?;
                defines.insert(macro_name, definition);
            }
            "undef" => {
                defines.remove(rest.trim());
            }
            _ => return Err(GlslError::Directive(line)),
        }
        // The directive's own continuation lines; the newline ending it
        // is the next token.
        let lines = token.text.matches('\n').count();
        out.push(Token::new(Kind::Space, "\n".repeat(lines)));
    }
    if !conds.is_empty() {
        return Err(GlslError::Directive("#if without #endif".to_owned()));
    }
    out.extend(expand(&pending, &defines, &mut budget)?);
    Ok(out)
}

/// A directive line without its comments and continuations
/// (`#endif // GL_ES` → `#endif`).
fn directive_text(raw: &str) -> String {
    // A `\` ending a line (trailing spaces allowed, as the lexer reads it)
    // joins the next one.
    let joined: String = raw
        .split('\n')
        .map(|line| {
            let line = line.trim_end();
            line.strip_suffix('\\').unwrap_or(line)
        })
        .collect::<Vec<_>>()
        .join(" ");
    let body = joined.trim().trim_start_matches('#');
    // A comment separates tokens, as in C (`A/**/B` is two words).
    let kept: Vec<Token> = lex(body)
        .into_iter()
        .map(|token| match token.kind {
            Kind::Comment => Token::new(Kind::Space, " "),
            _ => token,
        })
        .collect();
    format!("#{}", text(&kept).trim())
}

/// `#name rest` → `(name, rest)`.
fn split_directive(line: &str) -> (&str, &str) {
    let body = line.trim_start_matches('#').trim_start();
    let end = body
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .unwrap_or(body.len());
    (&body[..end], &body[end..])
}

/// `NAME body` or `NAME(a, b) body` → the macro.
fn define(rest: &str) -> Option<(String, Macro)> {
    let rest = rest.trim_start();
    let end = rest
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .unwrap_or(rest.len());
    let (name, after) = rest.split_at(end);
    if name.is_empty() || name.starts_with(|c: char| c.is_ascii_digit()) {
        return None;
    }
    // A function-like macro has its `(` right after the name.
    let (params, body) = match after.strip_prefix('(') {
        Some(list) => {
            let close = list.find(')')?;
            let params: Vec<String> = list[..close]
                .split(',')
                .map(|param| param.trim().to_owned())
                .filter(|param| !param.is_empty())
                .collect();
            let valid = params
                .iter()
                .all(|param| param.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'));
            if !valid {
                return None;
            }
            (Some(params), &list[close + 1..])
        }
        None => (None, after),
    };
    let body = lex(body.trim());
    if body.iter().any(|token| token.is_punct('#')) {
        return None;
    }
    Some((name.to_owned(), Macro { params, body }))
}

/// `tokens` with every macro expanded; a macro is not expanded again
/// inside its own expansion (C's rescanning rule). `budget` is the number
/// of tokens the expansion may still produce.
fn expand(
    tokens: &[Token],
    defines: &HashMap<String, Macro>,
    budget: &mut usize,
) -> Result<Vec<Token>, GlslError> {
    expand_hiding(tokens, defines, 0, &HashSet::new(), budget)
}

/// [`expand`] at nesting `depth`, with the macros being expanded around
/// this point in `hidden`.
fn expand_hiding(
    tokens: &[Token],
    defines: &HashMap<String, Macro>,
    depth: usize,
    hidden: &HashSet<String>,
    budget: &mut usize,
) -> Result<Vec<Token>, GlslError> {
    if depth > MAX_EXPANSION_DEPTH {
        return Err(GlslError::Unsupported("recursive macro"));
    }
    *budget = budget
        .checked_sub(tokens.len())
        .ok_or(GlslError::Unsupported("macro expansion too large"))?;
    let mut out = Vec::with_capacity(tokens.len());
    let mut i = 0;
    while i < tokens.len() {
        let token = &tokens[i];
        let found = (token.kind == Kind::Ident && !hidden.contains(&token.text))
            .then(|| defines.get(&token.text))
            .flatten();
        let Some(found) = found else {
            out.push(token.clone());
            i += 1;
            continue;
        };
        let mut inner = hidden.clone();
        inner.insert(token.text.clone());
        match &found.params {
            None => {
                out.extend(expand_hiding(
                    &found.body,
                    defines,
                    depth + 1,
                    &inner,
                    budget,
                )?);
                i += 1;
            }
            Some(params) => {
                let Some((args, after)) = call_arguments(tokens, i + 1) else {
                    // A function-like macro name without `(` stays a name.
                    out.push(token.clone());
                    i += 1;
                    continue;
                };
                let args = if params.is_empty() && args.len() == 1 && is_blank(&args[0]) {
                    Vec::new()
                } else {
                    args
                };
                if args.len() != params.len() {
                    return Err(GlslError::Unsupported("macro called with wrong arguments"));
                }
                // Arguments expand fully before substitution.
                let mut expanded_args = Vec::with_capacity(args.len());
                for arg in &args {
                    expanded_args.push(expand_hiding(arg, defines, depth + 1, hidden, budget)?);
                }
                let mut body = Vec::with_capacity(found.body.len());
                for part in &found.body {
                    let param = (part.kind == Kind::Ident)
                        .then(|| params.iter().position(|p| *p == part.text))
                        .flatten();
                    match param {
                        Some(at) => body.extend(expanded_args[at].iter().cloned()),
                        None => body.push(part.clone()),
                    }
                }
                out.extend(expand_hiding(&body, defines, depth + 1, &inner, budget)?);
                i = after;
            }
        }
    }
    Ok(out)
}

fn is_blank(tokens: &[Token]) -> bool {
    tokens.iter().all(Token::is_trivia)
}

/// The comma-separated arguments of a call whose `(` is the next
/// significant token at or after `from`, and the index after its `)`.
fn call_arguments(tokens: &[Token], from: usize) -> Option<(Vec<Vec<Token>>, usize)> {
    let open = (from..tokens.len()).find(|&j| !tokens[j].is_trivia())?;
    if !tokens[open].is_punct('(') {
        return None;
    }
    let mut args = vec![Vec::new()];
    let mut depth = 0usize;
    for (j, token) in tokens.iter().enumerate().skip(open + 1) {
        if token.is_punct('(') {
            depth += 1;
        } else if token.is_punct(')') {
            if depth == 0 {
                return Some((args, j + 1));
            }
            depth -= 1;
        } else if depth == 0 && token.is_punct(',') {
            args.push(Vec::new());
            continue;
        }
        args.last_mut()
            .expect("one argument at least")
            .push(token.clone());
    }
    None
}

/// The value of an `#if` / `#elif` condition, or `None` when it is not a
/// constant integer expression.
fn evaluate(condition: &str, defines: &HashMap<String, Macro>, budget: &mut usize) -> Option<bool> {
    // `defined NAME` / `defined(NAME)` resolve before macro expansion.
    let tokens = lex(condition);
    let mut resolved = Vec::with_capacity(tokens.len());
    let mut i = 0;
    while i < tokens.len() {
        if tokens[i].is_ident("defined") {
            let mut j = next_word(&tokens, i + 1)?;
            let parenthesised = tokens[j].is_punct('(');
            if parenthesised {
                j = next_word(&tokens, j + 1)?;
            }
            if tokens[j].kind != Kind::Ident {
                return None;
            }
            let value = if defines.contains_key(&tokens[j].text) {
                "1"
            } else {
                "0"
            };
            resolved.push(Token::new(Kind::Number, value));
            i = j + 1;
            if parenthesised {
                let close = next_word(&tokens, i)?;
                if !tokens[close].is_punct(')') {
                    return None;
                }
                i = close + 1;
            }
            continue;
        }
        resolved.push(tokens[i].clone());
        i += 1;
    }
    let expanded = expand(&resolved, defines, budget).ok()?;
    let mut parser = Expr::new(&expanded);
    let value = parser.ternary()?;
    parser.at_end().then_some(value != 0)
}

fn next_word(tokens: &[Token], from: usize) -> Option<usize> {
    (from..tokens.len()).find(|&j| !tokens[j].is_trivia())
}

/// A precedence-climbing evaluator over the significant tokens of an
/// `#if` expression. Unknown identifiers read as 0, as in C.
struct Expr {
    words: Vec<String>,
    at: usize,
}

impl Expr {
    fn new(tokens: &[Token]) -> Self {
        let mut words: Vec<String> = Vec::new();
        for token in tokens.iter().filter(|token| !token.is_trivia()) {
            // The lexer splits `&&`, `<=`, … into single characters; join
            // the two-character operators back.
            if token.kind == Kind::Punct {
                if let Some(last) = words.last_mut() {
                    let joined = format!("{last}{}", token.text);
                    if matches!(
                        joined.as_str(),
                        "&&" | "||" | "==" | "!=" | "<=" | ">=" | "<<" | ">>"
                    ) {
                        *last = joined;
                        continue;
                    }
                }
            }
            words.push(token.text.clone());
        }
        Self { words, at: 0 }
    }

    fn peek(&self) -> Option<&str> {
        self.words.get(self.at).map(String::as_str)
    }

    fn at_end(&self) -> bool {
        self.at == self.words.len()
    }

    fn eat(&mut self, word: &str) -> bool {
        let found = self.peek() == Some(word);
        if found {
            self.at += 1;
        }
        found
    }

    fn ternary(&mut self) -> Option<i64> {
        let condition = self.binary(0)?;
        if !self.eat("?") {
            return Some(condition);
        }
        let yes = self.ternary()?;
        if !self.eat(":") {
            return None;
        }
        let no = self.ternary()?;
        Some(if condition != 0 { yes } else { no })
    }

    fn binary(&mut self, min: u8) -> Option<i64> {
        let mut left = self.unary()?;
        while let Some((precedence, op)) = self.peek().and_then(binary_op) {
            if precedence < min {
                break;
            }
            self.at += 1;
            let right = self.binary(precedence + 1)?;
            left = apply(op, left, right)?;
        }
        Some(left)
    }

    fn unary(&mut self) -> Option<i64> {
        let word = self.peek()?.to_owned();
        self.at += 1;
        match word.as_str() {
            "!" => Some(i64::from(self.unary()? == 0)),
            "-" => self.unary()?.checked_neg(),
            "+" => self.unary(),
            "~" => Some(!self.unary()?),
            "(" => {
                let value = self.ternary()?;
                self.eat(")").then_some(value)
            }
            number if number.starts_with(|c: char| c.is_ascii_digit()) => parse_int(number),
            name if name.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_') => Some(0),
            _ => None,
        }
    }
}

fn binary_op(word: &str) -> Option<(u8, &'static str)> {
    Some(match word {
        "||" => (1, "||"),
        "&&" => (2, "&&"),
        "|" => (3, "|"),
        "^" => (4, "^"),
        "&" => (5, "&"),
        "==" => (6, "=="),
        "!=" => (6, "!="),
        "<" => (7, "<"),
        ">" => (7, ">"),
        "<=" => (7, "<="),
        ">=" => (7, ">="),
        "<<" => (8, "<<"),
        ">>" => (8, ">>"),
        "+" => (9, "+"),
        "-" => (9, "-"),
        "*" => (10, "*"),
        "/" => (10, "/"),
        "%" => (10, "%"),
        _ => return None,
    })
}

fn apply(op: &str, a: i64, b: i64) -> Option<i64> {
    Some(match op {
        "||" => i64::from(a != 0 || b != 0),
        "&&" => i64::from(a != 0 && b != 0),
        "|" => a | b,
        "^" => a ^ b,
        "&" => a & b,
        "==" => i64::from(a == b),
        "!=" => i64::from(a != b),
        "<" => i64::from(a < b),
        ">" => i64::from(a > b),
        "<=" => i64::from(a <= b),
        ">=" => i64::from(a >= b),
        "<<" => a.checked_shl(u32::try_from(b).ok()?)?,
        ">>" => a.checked_shr(u32::try_from(b).ok()?)?,
        "+" => a.checked_add(b)?,
        "-" => a.checked_sub(b)?,
        "*" => a.checked_mul(b)?,
        "/" => a.checked_div(b)?,
        "%" => a.checked_rem(b)?,
        _ => return None,
    })
}

/// A decimal, octal (`010`) or hex (`0x1F`) integer literal.
fn parse_int(word: &str) -> Option<i64> {
    if let Some(hex) = word.strip_prefix("0x").or_else(|| word.strip_prefix("0X")) {
        return i64::from_str_radix(hex, 16).ok();
    }
    if word.len() > 1 && word.starts_with('0') && word.chars().all(|c| c.is_ascii_digit()) {
        return i64::from_str_radix(&word[1..], 8).ok();
    }
    word.parse().ok()
}

#[cfg(test)]
#[path = "glsl_preprocess_tests.rs"]
mod tests;
