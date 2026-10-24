//! Text nodes: typography, wrapping, styled runs and links.
//!
//! `textGrowth` decides the box like it does on the canvas: `auto` hugs one
//! line (any authored number is ignored), `fixed-width` wraps at the width,
//! `fixed-width-height` also pins the height, and an absent `textGrowth`
//! keeps authored numbers on one line. The canvas's default line height is
//! 1.2, so that is written when none is authored.
//!
//! A pen.dev `href` arrives as an `open_url` tap action with a string-literal
//! url (#167); such a text exports as `<a href>`. Styled segments with their
//! own `href` become inline links.

use super::assets::Assets;
use super::ir::{Decls, Inline};
use super::paint::apply_text_fill;
use super::value::{color, font_family, px, round2};
use crate::fmt_num;
use de_editor_core::pen_node_ext::PenNodeExt;
use jian_ops_schema::node::{
    FontWeight, PenNode, TextAlign, TextAlignVertical, TextContent, TextFontStyle, TextGrowth,
    TextNode,
};
use jian_ops_schema::sizing::SizingBehavior;
use jian_ops_schema::style::FontStyleKind;

/// CSS numeric weight for an authored weight (`700`, `"700"`, `"bold"`).
pub(crate) fn weight_css(w: &FontWeight) -> Option<String> {
    match w {
        FontWeight::Number(n) => Some(n.to_string()),
        FontWeight::Keyword(k) => {
            let k = k.trim().to_ascii_lowercase();
            if let Ok(n) = k.parse::<u32>() {
                return Some(n.to_string());
            }
            let n = match k.replace(['-', '_', ' '], "").as_str() {
                "thin" | "hairline" => 100,
                "extralight" | "ultralight" => 200,
                "light" => 300,
                "normal" | "regular" => 400,
                "medium" => 500,
                "semibold" | "demibold" => 600,
                "bold" => 700,
                "extrabold" | "ultrabold" => 800,
                "black" | "heavy" => 900,
                _ => return None,
            };
            Some(n.to_string())
        }
    }
}

/// The text's width / height props after `textGrowth` (auto ignores numbers).
pub(crate) fn text_sizing(
    t: &TextNode,
) -> (Option<&SizingBehavior>, Option<&SizingBehavior>) {
    let (w, h) = (t.width.as_ref(), t.height.as_ref());
    fn unpin(s: Option<&SizingBehavior>) -> Option<&SizingBehavior> {
        s.filter(|s| matches!(s, SizingBehavior::Keyword(_)))
    }
    match t.text_growth {
        Some(TextGrowth::Auto) => (unpin(w), unpin(h)),
        Some(TextGrowth::FixedWidth) => (w, unpin(h)),
        Some(TextGrowth::FixedWidthHeight) | None => (w, h),
    }
}

/// Typography, wrapping and paint declarations of a text element.
pub(crate) fn apply_text(decls: &mut Decls, t: &TextNode, assets: &mut Assets, hint: &str) {
    if let Some(f) = t.font_family.as_deref().filter(|f| !f.trim().is_empty()) {
        decls.set("font-family", font_family(f));
    }
    if let Some(s) = t.font_size.filter(|s| *s > 0.0) {
        decls.set("font-size", px(s));
    }
    if let Some(w) = t.font_weight.as_ref().and_then(weight_css) {
        if w != "400" {
            decls.set("font-weight", w);
        }
    }
    if t.font_style == Some(TextFontStyle::Italic) {
        decls.set("font-style", "italic");
    }
    if let Some(ls) = t.letter_spacing.filter(|v| *v != 0.0) {
        decls.set("letter-spacing", px(ls));
    }
    let line_height = t.layout_line_height_multiplier().unwrap_or(1.2);
    decls.set("line-height", fmt_num(round2(line_height)));
    match t.text_align {
        Some(TextAlign::Center) => decls.set("text-align", "center"),
        Some(TextAlign::Right) => decls.set("text-align", "right"),
        Some(TextAlign::Justify) => decls.set("text-align", "justify"),
        Some(TextAlign::Left) | None => {}
    }
    let mut decoration = Vec::new();
    if t.underline == Some(true) {
        decoration.push("underline");
    }
    if t.strikethrough == Some(true) {
        decoration.push("line-through");
    }
    if !decoration.is_empty() {
        decls.set("text-decoration", decoration.join(" "));
    } else if node_link(t).is_some() {
        // A link keeps the design's look: no browser underline.
        decls.set("text-decoration", "none");
    }
    let wraps = matches!(
        t.text_growth,
        Some(TextGrowth::FixedWidth | TextGrowth::FixedWidthHeight)
    );
    decls.set("white-space", if wraps { "pre-wrap" } else { "pre" });
    if wraps {
        decls.set("overflow-wrap", "break-word");
    }
    if t.text_growth == Some(TextGrowth::FixedWidthHeight) {
        let justify = match t.text_align_vertical {
            Some(TextAlignVertical::Middle) => Some("center"),
            Some(TextAlignVertical::Bottom) => Some("flex-end"),
            Some(TextAlignVertical::Top) | None => None,
        };
        if let Some(j) = justify {
            decls.set("display", "flex");
            decls.set("flex-direction", "column");
            decls.set("justify-content", j);
        }
    }
    let fills = t.fill.as_deref().unwrap_or(&[]);
    apply_text_fill(decls, fills, assets, hint);
}

/// True when the text element must wrap its runs in one inner span (a flex
/// column would otherwise lay every run out as its own line).
pub(crate) fn needs_inner_wrapper(decls: &Decls) -> bool {
    decls.get("display") == Some("flex")
}

/// The text's runs. Plain content is one unstyled run.
pub(crate) fn inlines(t: &TextNode) -> Vec<Inline> {
    match &t.content {
        TextContent::Plain(s) => vec![Inline {
            text: s.clone(),
            decls: Decls::default(),
            href: None,
        }],
        TextContent::Styled(segs) => segs
            .iter()
            .map(|s| {
                let mut d = Decls::default();
                if let Some(f) = s.font_family.as_deref().filter(|f| !f.trim().is_empty()) {
                    d.set("font-family", font_family(f));
                }
                if let Some(size) = s.font_size.filter(|v| *v > 0.0) {
                    d.set("font-size", px(f64::from(size)));
                }
                if let Some(w) = s.font_weight {
                    d.set("font-weight", w.to_string());
                }
                if s.font_style == Some(FontStyleKind::Italic) {
                    d.set("font-style", "italic");
                }
                if let Some(c) = s.fill.as_deref().filter(|c| !c.trim().is_empty()) {
                    d.set("color", color(c));
                }
                let mut deco = Vec::new();
                if s.underline == Some(true) {
                    deco.push("underline");
                }
                if s.strikethrough == Some(true) {
                    deco.push("line-through");
                }
                if !deco.is_empty() {
                    d.set("text-decoration", deco.join(" "));
                }
                Inline {
                    text: s.text.clone(),
                    decls: d,
                    href: s
                        .href
                        .as_deref()
                        .map(str::trim)
                        .filter(|h| !h.is_empty())
                        .map(str::to_string),
                }
            })
            .collect(),
    }
}

/// The URL a text links to: its first `onTap` `open_url` action whose url is
/// a single-quoted string literal (how the pen.dev import writes `href`).
pub(crate) fn node_link(t: &TextNode) -> Option<String> {
    link_in(t.events.as_ref()?.on_tap.as_deref()?)
}

/// Same, for any node.
pub(crate) fn link_of(node: &PenNode) -> Option<String> {
    link_in(node.on_tap()?)
}

fn link_in(actions: &[jian_ops_schema::events::Action]) -> Option<String> {
    actions.iter().find_map(|action| {
        let (name, body) = action.single()?;
        if name != "open_url" {
            return None;
        }
        string_literal(body.get("url")?.as_str()?)
    })
}

/// Unquote a `'…'` expression literal (`\\` and `\'` are its escapes).
fn string_literal(expr: &str) -> Option<String> {
    let inner = expr.trim().strip_prefix('\'')?.strip_suffix('\'')?;
    let mut out = String::new();
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => out.push(chars.next()?),
            '\'' => return None,
            c => out.push(c),
        }
    }
    (!out.trim().is_empty()).then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal_urls_unescape() {
        assert_eq!(
            string_literal("'https://example.com/it\\'s'").as_deref(),
            Some("https://example.com/it's")
        );
        assert_eq!(string_literal("$url"), None);
        assert_eq!(string_literal("'a' + b"), None);
        assert_eq!(string_literal("''"), None);
    }

    #[test]
    fn keyword_weights() {
        let w = |s: &str| weight_css(&FontWeight::Keyword(s.into()));
        assert_eq!(w("bold").as_deref(), Some("700"));
        assert_eq!(w("Semi Bold").as_deref(), Some("600"));
        assert_eq!(w("650").as_deref(), Some("650"));
        assert_eq!(w("weird"), None);
    }
}
