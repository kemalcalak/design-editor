//! Paragraph and run properties, and the inheritance chain they resolve
//! through.
//!
//! A run's look is the first value found walking: the run's own `rPr`,
//! the paragraph's `pPr`, the shape's `lstStyle`, the layout and master
//! placeholders' `lstStyle`, the master's `txStyles` (for placeholders),
//! the shape style's `fontRef`, the presentation's `defaultTextStyle`, and
//! finally PowerPoint's built-in 18 pt. Each list contributes its
//! `lvlNpPr` for the paragraph's level, then its `defPPr`.

use roxmltree::Node;

use crate::color::{ColorContext, Rgba};
use crate::xml;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct RunStyle {
    pub size_pt: Option<f64>,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub underline: Option<bool>,
    pub strike: Option<bool>,
    pub color: Option<Rgba>,
    /// Raw typeface, theme references (`+mn-lt`) still unresolved.
    pub typeface: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Bullet {
    None,
    Char(String),
    AutoNum { scheme: String, start: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LineSpacing {
    /// Fraction of single spacing (1.0 = 100%).
    Percent(f64),
    Points(f64),
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ParaStyle {
    pub align: Option<String>,
    pub bullet: Option<Bullet>,
    pub bullet_font: Option<String>,
    pub line_spacing: Option<LineSpacing>,
    pub run: RunStyle,
}

macro_rules! fill_from {
    ($dst:expr, $src:expr, $($field:ident),*) => {
        $( if $dst.$field.is_none() { $dst.$field = $src.$field.clone(); } )*
    };
}

impl RunStyle {
    /// Fill every unset field from `lower`.
    pub fn inherit(&mut self, lower: &RunStyle) {
        fill_from!(self, lower, size_pt, bold, italic, underline, strike, color, typeface);
    }
}

impl ParaStyle {
    pub fn inherit(&mut self, lower: &ParaStyle) {
        fill_from!(self, lower, align, bullet, bullet_font, line_spacing);
        self.run.inherit(&lower.run);
    }
}

pub fn read_run(rpr: Node<'_, '_>, colors: &ColorContext<'_>) -> RunStyle {
    let mut run = RunStyle {
        size_pt: xml::attr_f64(rpr, "sz").map(|sz| sz / 100.0),
        bold: xml::attr_bool(rpr, "b"),
        italic: xml::attr_bool(rpr, "i"),
        underline: rpr.attribute("u").map(|u| u != "none"),
        strike: rpr.attribute("strike").map(|s| s != "noStrike"),
        color: None,
        typeface: xml::child(rpr, "latin")
            .and_then(|latin| latin.attribute("typeface"))
            .map(str::to_string),
    };
    if let Some(fill) = xml::child(rpr, "solidFill") {
        run.color = colors.color_in(fill);
    } else if let Some(fill) = xml::child(rpr, "gradFill") {
        // A gradient run has no text-segment spelling; its first stop is
        // the colour the eye reads.
        run.color = xml::path(fill, &["gsLst", "gs"]).and_then(|gs| colors.color_in(gs));
    }
    run
}

/// A `pPr` or a list style's `lvlNpPr` / `defPPr`.
pub fn read_paragraph(ppr: Node<'_, '_>, colors: &ColorContext<'_>) -> ParaStyle {
    let mut para = ParaStyle {
        align: ppr.attribute("algn").map(str::to_string),
        ..ParaStyle::default()
    };
    for el in xml::elements(ppr) {
        match el.tag_name().name() {
            "buNone" => para.bullet = Some(Bullet::None),
            "buChar" => {
                para.bullet = Some(Bullet::Char(
                    el.attribute("char").unwrap_or("•").to_string(),
                ))
            }
            "buAutoNum" => {
                para.bullet = Some(Bullet::AutoNum {
                    scheme: el.attribute("type").unwrap_or("arabicPeriod").to_string(),
                    start: xml::attr_i64(el, "startAt").unwrap_or(1).max(1) as u32,
                })
            }
            "buFont" => para.bullet_font = el.attribute("typeface").map(str::to_string),
            "lnSpc" => {
                para.line_spacing = xml::child(el, "spcPct")
                    .and_then(|p| xml::attr_f64(p, "val"))
                    .map(|v| LineSpacing::Percent(v / 100_000.0))
                    .or_else(|| {
                        xml::child(el, "spcPts")
                            .and_then(|p| xml::attr_f64(p, "val"))
                            .map(|v| LineSpacing::Points(v / 100.0))
                    })
            }
            "defRPr" => para.run = read_run(el, colors),
            _ => {}
        }
    }
    para
}

/// The ordered list styles one text body inherits through.
#[derive(Default)]
pub struct StyleChain<'a, 'i> {
    lists: Vec<Node<'a, 'i>>,
    /// Inserted between the placeholder lists and the presentation
    /// default: the shape style's `fontRef` colour and font.
    font_ref: RunStyle,
    font_ref_at: usize,
}

impl<'a, 'i> StyleChain<'a, 'i> {
    pub fn push(&mut self, list: Option<Node<'a, 'i>>) {
        if let Some(list) = list {
            self.lists.push(list);
        }
    }

    /// Record the `fontRef` and mark where in the chain it applies.
    pub fn set_font_ref(&mut self, run: RunStyle) {
        self.font_ref = run;
        self.font_ref_at = self.lists.len();
    }

    /// Resolved properties for a paragraph at `level` (0-based).
    pub fn level(&self, level: usize, colors: &ColorContext<'_>) -> ParaStyle {
        let name = format!("lvl{}pPr", level.min(8) + 1);
        let mut out = ParaStyle::default();
        for (index, list) in self.lists.iter().enumerate() {
            if index == self.font_ref_at {
                out.run.inherit(&self.font_ref);
            }
            if let Some(lvl) = xml::child(*list, &name) {
                out.inherit(&read_paragraph(lvl, colors));
            }
            if let Some(def) = xml::child(*list, "defPPr") {
                out.inherit(&read_paragraph(def, colors));
            }
        }
        if self.font_ref_at >= self.lists.len() {
            out.run.inherit(&self.font_ref);
        }
        out
    }
}

/// The prefix a bullet paints, numbered bullets counted by the caller.
pub fn bullet_prefix(bullet: &Bullet, font: Option<&str>, number: u32) -> Option<String> {
    match bullet {
        Bullet::None => None,
        Bullet::Char(ch) => Some(symbol_char(ch, font)),
        Bullet::AutoNum { scheme, .. } => Some(auto_number(scheme, number)),
    }
}

/// Symbol fonts put their glyphs on Latin code points; map the bullets
/// decks use to the Unicode characters they look like.
fn symbol_char(ch: &str, font: Option<&str>) -> String {
    let symbol_font = font.is_some_and(|f| {
        let f = f.to_ascii_lowercase();
        f.starts_with("wingdings") || f == "symbol" || f == "webdings"
    });
    if !symbol_font {
        return ch.to_string();
    }
    match ch {
        "§" | "n" => "▪",
        "Ø" | "Ü" => "➢",
        "ü" => "✓",
        "q" | "o" => "❑",
        "v" => "❖",
        "l" | "·" => "•",
        "Ú" | "à" => "➔",
        _ => "•",
    }
    .to_string()
}

fn auto_number(scheme: &str, n: u32) -> String {
    let body = if scheme.starts_with("alphaLc") {
        alpha(n, false)
    } else if scheme.starts_with("alphaUc") {
        alpha(n, true)
    } else if scheme.starts_with("romanLc") {
        roman(n).to_lowercase()
    } else if scheme.starts_with("romanUc") {
        roman(n)
    } else {
        n.to_string()
    };
    if scheme.ends_with("ParenBoth") {
        format!("({body})")
    } else if scheme.ends_with("ParenR") {
        format!("{body})")
    } else if scheme.ends_with("Plain") {
        body
    } else {
        format!("{body}.")
    }
}

fn alpha(n: u32, upper: bool) -> String {
    let base = if upper { b'A' } else { b'a' };
    let index = ((n.max(1) - 1) % 26) as u8;
    let repeat = ((n.max(1) - 1) / 26 + 1) as usize;
    std::iter::repeat_n((base + index) as char, repeat).collect()
}

fn roman(mut n: u32) -> String {
    const TABLE: [(u32, &str); 13] = [
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ];
    let mut out = String::new();
    for (value, digits) in TABLE {
        while n >= value {
            out.push_str(digits);
            n -= value;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbering_schemes() {
        assert_eq!(auto_number("arabicPeriod", 3), "3.");
        assert_eq!(auto_number("alphaLcParenR", 2), "b)");
        assert_eq!(auto_number("romanUcPeriod", 4), "IV.");
        assert_eq!(auto_number("arabicParenBoth", 1), "(1)");
    }

    #[test]
    fn wingdings_bullets_become_unicode() {
        assert_eq!(symbol_char("§", Some("Wingdings")), "▪");
        assert_eq!(symbol_char("–", Some("Arial")), "–");
    }
}
