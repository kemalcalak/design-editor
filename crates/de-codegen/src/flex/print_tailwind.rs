//! HTML + Tailwind v4 printer: declarations become utility classes.
//!
//! Common declarations map to their named utilities (`flex`, `flex-col`,
//! `items-center`, `w-full`, `shrink-0`, …); sizes, colours and spacing use
//! arbitrary values (`w-[320px]`, `bg-[var(--brand)]`), and anything without
//! a utility becomes an arbitrary property (`[outline-offset:-1px]`). Font
//! stacks are not arbitrary values: each family gets a `.font-<slug>` class in
//! `@layer base`, as pen.dev's Tailwind guide asks.

use super::ir::{Content, Decl, Decls, Element, Inline};
use super::value::slug;
use crate::html_escape;
use std::collections::BTreeMap;

pub(crate) struct Printed {
    pub markup: String,
    /// `.font-<slug>` → font-family value, for `@layer base`.
    pub font_classes: BTreeMap<String, String>,
}

pub(crate) fn print(elements: &[Element], depth: usize) -> Printed {
    let mut p = Printer::default();
    for el in elements {
        p.element(el, depth);
    }
    Printed {
        markup: p.markup,
        font_classes: p.fonts,
    }
}

#[derive(Default)]
struct Printer {
    markup: String,
    fonts: BTreeMap<String, String>,
}

/// An arbitrary value: whitespace becomes `_`, a literal `_` is escaped.
fn arb(value: &str) -> String {
    value
        .replace('_', "\\_")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("_")
}

fn sized(prefix: &str, value: &str) -> String {
    match value {
        "100%" => format!("{prefix}-full"),
        "0" => format!("{prefix}-0"),
        v => format!("{prefix}-[{}]", arb(v)),
    }
}

fn arbitrary_property(d: &Decl) -> String {
    format!("[{}:{}]", d.prop, arb(&d.value))
}

impl Printer {
    fn utility(&mut self, d: &Decl) -> Vec<String> {
        let v = d.value.as_str();
        let one = |s: &str| vec![s.to_string()];
        match (d.prop, v) {
            ("display", "flex" | "grid" | "block" | "inline-block") => one(v),
            ("flex-direction", "column") => one("flex-col"),
            ("justify-content", "center") => one("justify-center"),
            ("justify-content", "flex-end") => one("justify-end"),
            ("justify-content", "space-between") => one("justify-between"),
            ("justify-content", "space-around") => one("justify-around"),
            ("align-items", "flex-start" | "start") => one("items-start"),
            ("align-items", "center") => one("items-center"),
            ("align-items", "flex-end" | "end") => one("items-end"),
            ("justify-items", "start" | "center" | "end") => vec![format!("justify-items-{v}")],
            ("align-self", "stretch") => one("self-stretch"),
            ("flex", "1 1 0px") => one("flex-1"),
            ("flex-grow", "1") => one("grow"),
            ("flex-shrink", "0") => one("shrink-0"),
            ("position", "relative" | "absolute") => one(v),
            ("overflow", "hidden" | "visible") => vec![format!("overflow-{v}")],
            ("left", _) => vec![sized("left", v)],
            ("top", _) => vec![sized("top", v)],
            ("z-index", _) => vec![format!("z-[{v}]")],
            ("width", _) => vec![sized("w", v)],
            ("height", _) => vec![sized("h", v)],
            ("min-width", _) => vec![sized("min-w", v)],
            ("max-width", _) => vec![sized("max-w", v)],
            ("min-height", _) => vec![sized("min-h", v)],
            ("max-height", _) => vec![sized("max-h", v)],
            ("gap", _) => vec![sized("gap", v)],
            ("padding", _) => {
                let parts: Vec<&str> = v.split_whitespace().collect();
                match parts.as_slice() {
                    [a] => vec![sized("p", a)],
                    [y, x] => vec![sized("py", y), sized("px", x)],
                    [t, r, b, l] => vec![
                        sized("pt", t),
                        sized("pr", r),
                        sized("pb", b),
                        sized("pl", l),
                    ],
                    _ => vec![arbitrary_property(d)],
                }
            }
            ("border-radius", "50%") => one("rounded-full"),
            ("border-radius", _) if !v.contains(' ') => vec![format!("rounded-[{}]", arb(v))],
            ("background", _) if is_plain_color(v) => vec![format!("bg-[{}]", arb(v))],
            ("color", _) => vec![format!("text-[color:{}]", arb(v))],
            ("font-size", _) => vec![format!("text-[length:{}]", arb(v))],
            ("font-weight", _) => vec![format!("font-[{v}]")],
            ("font-style", "italic") => one("italic"),
            ("line-height", _) => vec![format!("leading-[{}]", arb(v))],
            ("letter-spacing", _) => vec![format!("tracking-[{}]", arb(v))],
            ("text-align", "center" | "right" | "justify") => vec![format!("text-{v}")],
            ("white-space", "pre") => one("whitespace-pre"),
            ("white-space", "pre-wrap") => one("whitespace-pre-wrap"),
            ("text-decoration", "none") => one("no-underline"),
            ("text-decoration", "underline" | "line-through") => one(v),
            ("object-fit", "cover" | "contain" | "fill" | "none") => {
                vec![format!("object-{v}")]
            }
            ("opacity", _) => vec![format!("opacity-[{}]", arb(v))],
            ("font-family", _) => {
                let family = v.split(',').next().unwrap_or(v).trim().trim_matches('"');
                let class = format!("font-{}", slug(family));
                self.fonts.insert(class.clone(), v.to_string());
                vec![class]
            }
            _ => vec![arbitrary_property(d)],
        }
    }

    fn classes(&mut self, decls: &Decls) -> Option<String> {
        if decls.is_empty() {
            return None;
        }
        let classes: Vec<String> = decls.iter().flat_map(|d| self.utility(d)).collect();
        Some(classes.join(" "))
    }

    fn open_tag(&mut self, el: &Element) -> String {
        let mut tag = format!("<{}", el.tag);
        if let Some(class) = self.classes(&el.decls) {
            tag.push_str(&format!(" class=\"{}\"", html_escape(&class)));
        }
        for (k, v) in &el.attrs {
            tag.push_str(&format!(" {}=\"{}\"", k, html_escape(v)));
        }
        tag.push('>');
        tag
    }

    fn element(&mut self, el: &Element, depth: usize) {
        let indent = "  ".repeat(depth);
        if el.tag.is_empty() {
            if let Content::Raw(raw) = &el.content {
                self.markup.push_str(&format!("{indent}{raw}\n"));
            }
            return;
        }
        let open = self.open_tag(el);
        if el.is_void() {
            self.markup.push_str(&format!("{indent}{open}\n"));
            return;
        }
        match &el.content {
            Content::Empty => {
                self.markup
                    .push_str(&format!("{indent}{open}</{}>\n", el.tag));
            }
            Content::Raw(raw) => {
                self.markup
                    .push_str(&format!("{indent}{open}{raw}</{}>\n", el.tag));
            }
            Content::Text(runs) => {
                let body = self.runs(runs);
                self.markup
                    .push_str(&format!("{indent}{open}{body}</{}>\n", el.tag));
            }
            Content::Children(children) => {
                self.markup.push_str(&format!("{indent}{open}\n"));
                for child in children {
                    self.element(child, depth + 1);
                }
                self.markup.push_str(&format!("{indent}</{}>\n", el.tag));
            }
        }
    }

    fn runs(&mut self, runs: &[Inline]) -> String {
        let mut out = String::new();
        for run in runs {
            let text = html_escape(&run.text);
            let class_attr = self
                .classes(&run.decls)
                .map(|c| format!(" class=\"{}\"", html_escape(&c)))
                .unwrap_or_default();
            match (&run.href, class_attr.is_empty()) {
                (Some(href), _) => out.push_str(&format!(
                    "<a href=\"{}\"{class_attr}>{text}</a>",
                    html_escape(href)
                )),
                (None, false) => out.push_str(&format!("<span{class_attr}>{text}</span>")),
                (None, true) => out.push_str(&text),
            }
        }
        out
    }
}

/// A single colour value (no layers, no images) can use `bg-[…]`.
fn is_plain_color(v: &str) -> bool {
    !v.contains("gradient") && !v.contains("url(") && (!v.contains(", ") || v.starts_with("color-mix("))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn classes_of(pairs: &[(&'static str, &str)]) -> String {
        let mut d = Decls::default();
        for (p, v) in pairs {
            d.set(p, *v);
        }
        Printer::default().classes(&d).unwrap()
    }

    #[test]
    fn layout_maps_to_named_utilities() {
        assert_eq!(
            classes_of(&[
                ("display", "flex"),
                ("flex-direction", "column"),
                ("align-items", "flex-start"),
                ("gap", "12px"),
                ("padding", "8px 16px"),
            ]),
            "flex flex-col items-start gap-[12px] py-[8px] px-[16px]"
        );
        assert_eq!(
            classes_of(&[("flex", "1 1 0px"), ("min-width", "0"), ("width", "100%")]),
            "flex-1 min-w-0 w-full"
        );
    }

    #[test]
    fn values_without_utilities_are_arbitrary() {
        assert_eq!(
            classes_of(&[("outline", "1px solid rgb(0,0,0)"), ("outline-offset", "-0.5px")]),
            "[outline:1px_solid_rgb(0,0,0)] [outline-offset:-0.5px]"
        );
        assert_eq!(
            classes_of(&[("background", "linear-gradient(90deg, rgb(255,0,0) 0%, rgb(0,0,255) 100%)")]),
            "[background:linear-gradient(90deg,_rgb(255,0,0)_0%,_rgb(0,0,255)_100%)]"
        );
        assert_eq!(classes_of(&[("color", "var(--ink)")]), "text-[color:var(--ink)]");
        assert_eq!(classes_of(&[("background", "var(--bg)")]), "bg-[var(--bg)]");
    }
}
