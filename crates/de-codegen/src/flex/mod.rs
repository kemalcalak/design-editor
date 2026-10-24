//! Flex-layout HTML export: the `html` (HTML + CSS) and `html-tailwind`
//! (HTML + Tailwind v4) targets (#170).
//!
//! Auto-layout frames become flexbox, `layout: none` frames a one-cell grid
//! stack, and only nodes the design positions absolutely get `left` / `top`
//! (see `layout.rs`). Design variables are CSS custom properties in `:root`
//! (plus one block per theme value, selected by `data-<axis>` on `<html>`),
//! and every `$name` reference in the design reads `var(--name)`.
//!
//! [`export_html`] also returns the image files it extracted, so a host can
//! write them next to the HTML; the [`Codegen`] impls keep images inline.

mod assets;
mod editor;
mod ir;
mod layout;
mod markup;
mod paint;
mod print_css;
mod print_tailwind;
mod shapes;
mod text;
mod value;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_tailwind;

pub use assets::HtmlAsset;
pub use editor::{export_editor_html, html_target_for};

use crate::codegen_targets::codegen_motion_css;
use crate::{html_escape, root_nodes, Codegen, CssVariables};
use jian_ops_schema::PenDocument;

/// Which flavour of HTML to print.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HtmlTarget {
    /// Class rules in a `<style>` block.
    Css,
    /// Tailwind v4 utility classes.
    Tailwind,
}

impl HtmlTarget {
    /// The wire token (`codegen_export`'s `framework`).
    pub fn as_wire(self) -> &'static str {
        match self {
            HtmlTarget::Css => "html",
            HtmlTarget::Tailwind => "html-tailwind",
        }
    }
}

/// Export options, pen.dev's `html-css` / `html-tailwind` switches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HtmlExportOptions {
    /// A full document (`<!DOCTYPE html>`, `<head>`, `<body>`) instead of a
    /// fragment.
    pub scaffold: bool,
    /// Layer names and ids as `data-name` / `data-id` attributes.
    pub layer_data: bool,
    /// Extract embedded images into files under this relative directory;
    /// `None` keeps them inline.
    pub asset_dir: Option<String>,
    /// `<title>` of the scaffold; the document name when `None`.
    pub title: Option<String>,
}

impl Default for HtmlExportOptions {
    fn default() -> Self {
        Self {
            scaffold: true,
            layer_data: false,
            asset_dir: None,
            title: None,
        }
    }
}

/// One export: the HTML and the files it references relatively.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HtmlExport {
    pub html: String,
    pub assets: Vec<HtmlAsset>,
}

/// Tailwind's browser build, pinned. Swap for `@import "tailwindcss"` in a
/// build setup.
const TAILWIND_CDN: &str = "https://cdn.jsdelivr.net/npm/@tailwindcss/browser@4.1.13";

/// Base rules of the HTML + CSS target. Tailwind's Preflight covers the same.
const CSS_RESET: &str = "*, *::before, *::after {\n  box-sizing: border-box;\n  margin: 0;\n}\n";

/// The design variables as `:root` custom properties (the `CssVariables`
/// table without its banner).
fn variables_css(doc: &PenDocument) -> String {
    CssVariables
        .generate(doc)
        .lines()
        .filter(|l| !l.starts_with("/*"))
        .map(|l| format!("{l}\n"))
        .collect()
}

/// `data-<axis>="<first value>"` for every theme axis, so the default theme's
/// variable block applies.
fn theme_attrs(doc: &PenDocument) -> String {
    let Some(themes) = doc.themes.as_ref() else {
        return String::new();
    };
    themes
        .iter()
        .filter_map(|(axis, values)| {
            let value = values.first()?;
            Some(format!(" data-{}=\"{}\"", axis, html_escape(value)))
        })
        .collect()
}

/// Export the document's roots (the first page, else bare children).
pub fn export_html(doc: &PenDocument, target: HtmlTarget, options: &HtmlExportOptions) -> HtmlExport {
    let resolved;
    let doc = if de_editor_core::ref_resolve::document_has_refs(doc) {
        resolved = de_editor_core::ref_resolve::resolve_refs_for_canvas(doc);
        &resolved
    } else {
        doc
    };
    let mut assets = assets::Assets::new(options.asset_dir.clone());
    let elements = markup::Walker {
        assets: &mut assets,
        layer_data: options.layer_data,
    }
    .roots(root_nodes(doc));
    let depth = usize::from(options.scaffold);
    let motion = codegen_motion_css::render(doc);
    let vars = variables_css(doc);

    let (head_style, body) = match target {
        HtmlTarget::Css => {
            let printed = print_css::print(&elements, depth);
            let mut css = String::from(CSS_RESET);
            css.push_str(&vars);
            css.push_str(&printed.rules);
            (format!("<style>\n{css}</style>\n"), printed.markup)
        }
        HtmlTarget::Tailwind => {
            let printed = print_tailwind::print(&elements, depth);
            let mut css = vars;
            if !printed.font_classes.is_empty() {
                css.push_str("@layer base {\n");
                for (class, family) in &printed.font_classes {
                    css.push_str(&format!("  .{class} {{\n    font-family: {family};\n  }}\n"));
                }
                css.push_str("}\n");
            }
            let style = if css.is_empty() {
                String::new()
            } else {
                format!("<style type=\"text/tailwindcss\">\n{css}</style>\n")
            };
            (style, printed.markup)
        }
    };
    let motion_style = if motion.is_empty() {
        String::new()
    } else {
        format!("<style>\n{motion}</style>\n")
    };

    let mut html = format!(
        "<!-- Generated by Design Editor — codegen::{} -->\n",
        target.as_wire()
    );
    if options.scaffold {
        let title = options
            .title
            .clone()
            .or_else(|| doc.name.clone())
            .unwrap_or_else(|| "Design Editor export".into());
        html.push_str(&format!(
            "<!DOCTYPE html>\n<html lang=\"en\"{}>\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<title>{}</title>\n",
            theme_attrs(doc),
            html_escape(&title)
        ));
        if target == HtmlTarget::Tailwind {
            html.push_str(&format!("<script src=\"{TAILWIND_CDN}\"></script>\n"));
        }
        html.push_str(&motion_style);
        html.push_str(&head_style);
        html.push_str("</head>\n<body>\n");
        html.push_str(&body);
        html.push_str("</body>\n</html>\n");
    } else {
        html.push_str(&motion_style);
        html.push_str(&head_style);
        html.push_str(&body);
    }
    HtmlExport {
        html,
        assets: assets.into_files(),
    }
}

/// HTML + CSS generator (`html`): flex layout, class rules, `:root`
/// variables, images inline.
pub struct Html;

impl Codegen for Html {
    fn target_label(&self) -> &'static str {
        "html"
    }

    fn generate(&self, doc: &PenDocument) -> String {
        export_html(doc, HtmlTarget::Css, &HtmlExportOptions::default()).html
    }
}

/// HTML + Tailwind v4 generator (`html-tailwind`).
pub struct HtmlTailwind;

impl Codegen for HtmlTailwind {
    fn target_label(&self) -> &'static str {
        "html-tailwind"
    }

    fn generate(&self, doc: &PenDocument) -> String {
        export_html(doc, HtmlTarget::Tailwind, &HtmlExportOptions::default()).html
    }
}
