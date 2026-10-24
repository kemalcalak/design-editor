//! HTML + CSS printer: every styled element gets one class, and its
//! declarations become that class's rule in a `<style>` block.

use super::ir::{Content, Decls, Element, Inline};
use crate::html_escape;
use std::collections::{BTreeSet, HashMap};

/// Markup plus the class rules it uses.
pub(crate) struct Printed {
    pub markup: String,
    pub rules: String,
}

pub(crate) fn print(elements: &[Element], depth: usize) -> Printed {
    let mut p = Printer::default();
    for el in elements {
        p.element(el, depth);
    }
    Printed {
        markup: p.markup,
        rules: p.rules,
    }
}

#[derive(Default)]
struct Printer {
    markup: String,
    rules: String,
    used: BTreeSet<String>,
    /// Next suffix to try per stem, so many same-named layers stay linear.
    next: HashMap<String, usize>,
}

impl Printer {
    fn class_for(&mut self, hint: &str, decls: &Decls) -> Option<String> {
        if decls.is_empty() {
            return None;
        }
        let stem = if hint.is_empty() { "el" } else { hint };
        // A class must not start with a digit.
        let stem = if stem.starts_with(|c: char| c.is_ascii_digit()) {
            format!("n-{stem}")
        } else {
            stem.to_string()
        };
        let mut name = stem.clone();
        let n = self.next.entry(stem.clone()).or_insert(2);
        while !self.used.insert(name.clone()) {
            name = format!("{stem}-{n}");
            *n += 1;
        }
        self.rules.push_str(&format!(".{name} {{\n"));
        for d in decls.iter() {
            self.rules
                .push_str(&format!("  {}: {};\n", d.prop, d.value));
        }
        self.rules.push_str("}\n");
        Some(name)
    }

    fn open_tag(&mut self, el: &Element, class: Option<&str>) -> String {
        let mut tag = format!("<{}", el.tag);
        if let Some(class) = class {
            tag.push_str(&format!(" class=\"{class}\""));
        }
        for (k, v) in &el.attrs {
            tag.push_str(&format!(" {}=\"{}\"", k, html_escape(v)));
        }
        tag.push('>');
        tag
    }

    fn element(&mut self, el: &Element, depth: usize) {
        let indent = "  ".repeat(depth);
        if el.is_raw() {
            if let Content::Raw(raw) = &el.content {
                self.markup.push_str(&format!("{indent}{raw}\n"));
            }
            return;
        }
        let class = self.class_for(&el.name_hint, &el.decls);
        let open = self.open_tag(el, class.as_deref());
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
                let body = self.runs(runs, &el.name_hint);
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

    fn runs(&mut self, runs: &[Inline], hint: &str) -> String {
        let mut out = String::new();
        for run in runs {
            let text = html_escape(&run.text);
            let class = self.class_for(&format!("{hint}-run"), &run.decls);
            let class_attr = class.map(|c| format!(" class=\"{c}\"")).unwrap_or_default();
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
