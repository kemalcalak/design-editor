//! Pixel word-wrap for the Design-MD panel's body (#451).
//!
//! The body used to wrap by a character budget (`CHAR_W`, 6 px a
//! character), so a line of wide glyphs ran past the panel and a line of
//! narrow ones broke early, and CJK paragraphs — one "word" with no spaces —
//! never broke at all. It now wraps at the width the runs are painted at:
//! every run is measured in the chrome face at the body size through
//! [`LabelWidths`], the same widths [`paint_runs`] advances by, so a line
//! that wraps to fit is a line that paints inside the panel.
//!
//! The body is styled runs, not plain text: a `` `code` `` span paints on a
//! chip [`CODE_PAD_W`] wider than its text, a `#HEX` token behind a
//! [`COLOR_SWATCH_W`] swatch, and both stay whole. So words are placed here,
//! with the rules of jian's `wrap_runs` (plain and bold runs break at spaces,
//! words join with one space), and a word wider than the whole line — a CJK
//! or Thai phrase, a long URL — is broken by [`LabelWidths::wrap`], the one
//! chrome wrapper that breaks between CJK characters and Thai clusters.
//!
//! [`paint_runs`]: super::design_md_panel

use super::design_md_markdown::MdRun;
use super::label_widths::LabelWidths;
use super::text_metrics::CHROME_FONT_FAMILY;

/// Width a code span adds to its text: its chip's 2 px pad either side.
pub(super) const CODE_PAD_W: f32 = 4.0;
/// Width a colour token adds to its hex: the 10 px swatch and its 4 px gap.
pub(super) const COLOR_SWATCH_W: f32 = 14.0;
/// Every body run is painted at the regular weight (bold is a colour, not a
/// weight, in this panel).
const REGULAR: u16 = 400;

/// Painted width of `text` in the body face at `size`.
pub(super) fn text_width(widths: &mut LabelWidths<'_>, text: &str, size: f32) -> f32 {
    widths.styled(text, size, CHROME_FONT_FAMILY, REGULAR)
}

/// How far painting `run` at `size` advances the pen.
pub(super) fn run_width(widths: &mut LabelWidths<'_>, run: &MdRun, size: f32) -> f32 {
    match run {
        MdRun::Plain(text) | MdRun::Bold(text) => text_width(widths, text, size),
        MdRun::Code(text) => text_width(widths, text, size) + CODE_PAD_W,
        MdRun::Color(hex) => COLOR_SWATCH_W + text_width(widths, hex, size),
    }
}

/// Wrap `runs` into lines no wider than `max_w` painted at `size`.
pub(super) fn wrap_md_runs(
    runs: &[MdRun],
    max_w: f32,
    size: f32,
    widths: &mut LabelWidths<'_>,
) -> Vec<Vec<MdRun>> {
    let mut wrap = Wrap {
        widths,
        size,
        max_w,
        space: 0.0,
        lines: Vec::new(),
        line: Vec::new(),
        x: 0.0,
    };
    wrap.space = text_width(wrap.widths, " ", size);
    for run in runs {
        match run {
            MdRun::Plain(text) | MdRun::Bold(text) => {
                let mut had_word = false;
                for word in text.split_whitespace() {
                    had_word = true;
                    wrap.place_word(word, run);
                }
                // A whitespace-only run between two others still separates
                // them, as long as the space fits.
                if !had_word && !text.is_empty() && wrap.x > 0.0 && wrap.fits(wrap.space) {
                    append_styled(&mut wrap.line, " ", run);
                    wrap.x += wrap.space;
                }
            }
            MdRun::Code(_) | MdRun::Color(_) => {
                let w = run_width(wrap.widths, run, size);
                if wrap.x > 0.0 && !wrap.fits(w) {
                    wrap.flush();
                }
                wrap.line.push(run.clone());
                wrap.x += w;
            }
        }
    }
    wrap.flush();
    if wrap.lines.is_empty() {
        wrap.lines.push(Vec::new());
    }
    wrap.lines
}

/// The line being filled and the lines already full.
struct Wrap<'w, 'b> {
    widths: &'w mut LabelWidths<'b>,
    size: f32,
    max_w: f32,
    /// Width of the space that joins two words.
    space: f32,
    lines: Vec<Vec<MdRun>>,
    line: Vec<MdRun>,
    /// Painted width of `line` so far.
    x: f32,
}

impl Wrap<'_, '_> {
    /// Whether `w` more fits on the current line.
    fn fits(&self, w: f32) -> bool {
        self.x + w <= self.max_w
    }

    /// End the current line, if it holds anything.
    fn flush(&mut self) {
        if !self.line.is_empty() {
            self.lines.push(std::mem::take(&mut self.line));
        }
        self.x = 0.0;
    }

    /// Append `text` in `style`, preceded by a space when the line is not
    /// empty; the caller has checked it fits.
    fn append(&mut self, text: &str, style: &MdRun, w: f32) {
        if self.x > 0.0 {
            append_styled(&mut self.line, " ", style);
            self.x += self.space;
        }
        append_styled(&mut self.line, text, style);
        self.x += w;
    }

    /// Place one whitespace-free `word` of a plain or bold run.
    fn place_word(&mut self, word: &str, style: &MdRun) {
        let w = text_width(self.widths, word, self.size);
        let sep = if self.x > 0.0 { self.space } else { 0.0 };
        if self.fits(sep + w) {
            self.append(word, style, w);
            return;
        }
        if w <= self.max_w {
            self.flush();
            self.append(word, style, w);
            return;
        }
        // Wider than a whole line, so it is broken wherever it lands: fill
        // what is left of this line first, then break the rest at full width.
        let mut rest = word;
        let room = self.max_w - self.x - sep;
        if self.x > 0.0 && room >= self.size * 2.0 {
            let head = self.break_head(rest, room);
            if !head.is_empty() {
                let head_w = text_width(self.widths, head, self.size);
                self.append(head, style, head_w);
                rest = &rest[head.len()..];
            }
        }
        self.flush();
        let pieces = self
            .widths
            .wrap(rest, self.max_w, self.size, CHROME_FONT_FAMILY, REGULAR);
        for (index, piece) in pieces.iter().enumerate() {
            if index > 0 {
                self.flush();
            }
            let piece_w = text_width(self.widths, piece, self.size);
            self.append(piece, style, piece_w);
        }
    }

    /// The longest head of `word` that [`LabelWidths::wrap`] fits in
    /// `room`, as a prefix of `word` (empty when it does not break there).
    fn break_head<'t>(&mut self, word: &'t str, room: f32) -> &'t str {
        let lines = self
            .widths
            .wrap(word, room, self.size, CHROME_FONT_FAMILY, REGULAR);
        match lines.first() {
            Some(head) if lines.len() > 1 && word.starts_with(head.as_str()) => &word[..head.len()],
            _ => "",
        }
    }
}

/// Append `text` to `line` in the style of `style`, merging into a trailing
/// run of the same style.
fn append_styled(line: &mut Vec<MdRun>, text: &str, style: &MdRun) {
    match (line.last_mut(), style) {
        (Some(MdRun::Plain(s)), MdRun::Plain(_)) | (Some(MdRun::Bold(s)), MdRun::Bold(_)) => {
            s.push_str(text)
        }
        (_, MdRun::Bold(_)) => line.push(MdRun::Bold(text.to_string())),
        _ => line.push(MdRun::Plain(text.to_string())),
    }
}

#[cfg(test)]
#[path = "design_md_wrap_tests.rs"]
mod tests;
