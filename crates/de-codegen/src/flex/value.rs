//! Scalar → CSS value helpers shared by the flex emitters.
//!
//! Design variables stay live in the export: a `$name` reference becomes
//! `var(--name)`, so the `:root` block the document emits drives every use.
//! Number variables are written unitless there (the `CssVariables` table),
//! so a length reference is `calc(var(--name) * 1px)`.

use crate::{css_custom_property, fmt_num, parse_color};
use jian_ops_schema::node::NumberOrExpression;
use jian_ops_schema::sizing::SizingBehavior;

/// A `$name` reference's variable name, when `s` is one.
pub(crate) fn variable_ref(s: &str) -> Option<&str> {
    let name = s.trim().strip_prefix('$')?;
    (!name.is_empty()).then_some(name)
}

/// `var(--name)` for a design-variable name.
pub(crate) fn var_css(name: &str) -> String {
    format!("var({})", css_custom_property(name))
}

/// A length in px, trimmed like the other generators print numbers.
pub(crate) fn px(n: f64) -> String {
    if n == 0.0 {
        "0".into()
    } else {
        format!("{}px", fmt_num(round2(n)))
    }
}

/// Round to two decimals: well under a pixel, and keeps generated CSS free of
/// float noise such as `33.333333333333336px`.
pub(crate) fn round2(n: f64) -> f64 {
    (n * 100.0).round() / 100.0
}

/// A ratio in `0..=1` with up to four decimals.
pub(crate) fn ratio(n: f64) -> String {
    let n = n.clamp(0.0, 1.0);
    let s = format!("{n:.4}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() {
        "0".into()
    } else {
        s.into()
    }
}

/// A length that may be a variable reference.
pub(crate) fn length_expr(expr: &str) -> Option<String> {
    variable_ref(expr).map(|name| format!("calc({} * 1px)", var_css(name)))
}

/// `gap`-style number-or-expression as a length.
pub(crate) fn number_or_expr_px(value: &NumberOrExpression) -> Option<String> {
    match value {
        NumberOrExpression::Number(n) => Some(px(*n)),
        NumberOrExpression::Expression(e) => length_expr(e),
    }
}

/// A literal number-or-expression as a unitless value (opacity).
pub(crate) fn number_or_expr_unitless(value: &NumberOrExpression) -> Option<String> {
    match value {
        NumberOrExpression::Number(n) => Some(ratio(*n)),
        NumberOrExpression::Expression(e) => variable_ref(e).map(var_css),
    }
}

/// A fixed size: a literal number or a variable reference. Keywords are the
/// caller's job (they depend on the parent's axis).
pub(crate) fn fixed_size(value: &SizingBehavior) -> Option<String> {
    match value {
        SizingBehavior::Number(n) => Some(px(*n)),
        SizingBehavior::Expression(e) => length_expr(e),
        SizingBehavior::Keyword(_) => None,
    }
}

/// A colour string as CSS. Hex becomes `rgb()` / `rgba()`, a `$name`
/// reference `var(--name)`; anything else (a named colour) passes through.
pub(crate) fn color(s: &str) -> String {
    color_alpha(s, 1.0)
}

/// A colour scaled by an extra alpha (a fill layer's opacity). A variable
/// keeps its reference through `color-mix`, so a theme switch still applies.
pub(crate) fn color_alpha(s: &str, alpha: f64) -> String {
    let alpha = alpha.clamp(0.0, 1.0);
    if let Some(name) = variable_ref(s) {
        if alpha >= 0.999 {
            return var_css(name);
        }
        return format!(
            "color-mix(in srgb, {} {}%, transparent)",
            var_css(name),
            fmt_num(round2(alpha * 100.0))
        );
    }
    match parse_color(s) {
        Some((r, g, b, a)) => {
            let a = f64::from(a) * alpha;
            let to_u8 = |c: f32| (c.clamp(0.0, 1.0) * 255.0).round() as u8;
            if a >= 0.999 {
                format!("rgb({},{},{})", to_u8(r), to_u8(g), to_u8(b))
            } else {
                format!("rgba({},{},{},{})", to_u8(r), to_u8(g), to_u8(b), ratio(a))
            }
        }
        // A named colour passes through; anything else could break out of
        // the declaration (`;`, `}`, `</style>`) and is dropped.
        None => {
            let t = s.trim();
            if !t.is_empty() && t.chars().all(|c| c.is_ascii_alphabetic()) {
                t.to_string()
            } else {
                "transparent".into()
            }
        }
    }
}

/// A CSS-safe slug: lowercase ASCII letters, digits and single dashes.
pub(crate) fn slug(s: &str) -> String {
    let mut out = String::new();
    let mut dash = false;
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            dash = false;
        } else if !dash && !out.is_empty() {
            out.push('-');
            dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out
}

/// A font family as a CSS `font-family` value: the authored family quoted,
/// with a generic fallback.
pub(crate) fn font_family(family: &str) -> String {
    // Only characters that cannot end the string, the declaration or the
    // `<style>` element survive.
    let family: String = family
        .trim()
        .chars()
        .filter(|c| !matches!(c, '"' | '\\' | '<' | '>' | ';' | '{' | '}') && !c.is_control())
        .collect();
    let generic = if family.to_ascii_lowercase().contains("mono") {
        "monospace"
    } else if family.to_ascii_lowercase().contains("serif")
        && !family.to_ascii_lowercase().contains("sans")
    {
        "serif"
    } else {
        "sans-serif"
    };
    format!("\"{family}\", {generic}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colours_and_variables() {
        assert_eq!(color("#ff0000"), "rgb(255,0,0)");
        assert_eq!(color("#ff000080"), "rgba(255,0,0,0.502)");
        assert_eq!(color("$brand"), "var(--brand)");
        assert_eq!(
            color_alpha("$brand", 0.5),
            "color-mix(in srgb, var(--brand) 50%, transparent)"
        );
        assert_eq!(color_alpha("#000000", 0.25), "rgba(0,0,0,0.25)");
        assert_eq!(color("tomato"), "tomato");
        assert_eq!(color("red;}</style><script>"), "transparent");
    }

    #[test]
    fn lengths() {
        assert_eq!(px(12.0), "12px");
        assert_eq!(px(0.0), "0");
        assert_eq!(px(1.0 / 3.0), "0.33px");
        assert_eq!(
            length_expr("$space-4").as_deref(),
            Some("calc(var(--space-4) * 1px)")
        );
        assert_eq!(length_expr("12"), None);
    }

    #[test]
    fn slugs_and_fonts() {
        assert_eq!(slug("Hero Card / Title"), "hero-card-title");
        assert_eq!(slug("  "), "");
        assert_eq!(font_family("Inter"), "\"Inter\", sans-serif");
        assert_eq!(
            font_family("JetBrains Mono"),
            "\"JetBrains Mono\", monospace"
        );
        assert_eq!(font_family("Noto Serif"), "\"Noto Serif\", serif");
        assert_eq!(font_family("A</style>{x}"), "\"A/stylex\", sans-serif");
    }
}
