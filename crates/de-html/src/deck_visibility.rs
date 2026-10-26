//! Slides a deck viewer hides until its script shows them.
//!
//! A player keeps every slide but the current one at `display: none` (or
//! invisible) and adds a state class such as `.on` / `.active` to the one on
//! screen. The importer runs no script, so it does what the script would:
//! it adds the first state class the stylesheet answers with a visible
//! slide, and falls back to an inline style when none does.

use crate::css::cascade::{compute_style_for_viewport, ComputedStyle, StyleRule};
use crate::dom::DomElement;
use crate::HtmlImportOptions;

/// The class names deck players use for "this slide is on screen".
const STATE_CLASSES: [&str; 10] = [
    "on",
    "active",
    "current",
    "present",
    "visible",
    "show",
    "shown",
    "is-active",
    "is-current",
    "selected",
];

/// Hidden by viewer state rather than removed from the document.
pub(crate) fn is_hidden(style: &ComputedStyle) -> bool {
    style.get("display") == Some("none")
        || matches!(style.get("visibility"), Some("hidden" | "collapse"))
        || transparent(style)
}

fn transparent(style: &ComputedStyle) -> bool {
    style
        .get("opacity")
        .and_then(|value| value.trim().parse::<f64>().ok())
        .is_some_and(|opacity| opacity <= 0.0)
}

/// Make `element` render the way the player shows it on screen.
/// `ancestors` is every element above it, outermost first, already shown.
pub(crate) fn show(
    ancestors: &[&DomElement],
    element: &mut DomElement,
    parent_style: Option<&ComputedStyle>,
    rules: &[StyleRule],
    opts: &HtmlImportOptions,
) {
    element
        .attrs
        .retain(|(name, value)| name != "hidden" && !(name == "aria-hidden" && value == "true"));
    let style_of = |element: &DomElement| {
        let mut path = ancestors.to_vec();
        path.push(element);
        compute_style_for_viewport(
            &path,
            rules,
            parent_style,
            opts.base_font_size,
            opts.viewport_width,
            opts.viewport_height(),
        )
    };
    if !is_hidden(&style_of(element)) {
        return;
    }
    let classes: Vec<String> = element.classes().into_iter().map(str::to_string).collect();
    for state in STATE_CLASSES {
        if classes.iter().any(|class| class == state) {
            continue;
        }
        let mut probe = element.clone();
        set_classes(
            &mut probe,
            classes.iter().map(String::as_str).chain([state]),
        );
        if !is_hidden(&style_of(&probe)) {
            *element = probe;
            return;
        }
    }
    let style = style_of(element);
    let mut inline = String::new();
    if style.get("display") == Some("none") {
        inline.push_str("display:block;");
    }
    if matches!(style.get("visibility"), Some("hidden" | "collapse")) {
        inline.push_str("visibility:visible;");
    }
    if transparent(&style) {
        inline.push_str("opacity:1;");
    }
    match element.attrs.iter_mut().find(|(name, _)| name == "style") {
        Some((_, value)) => {
            value.push(';');
            value.push_str(&inline);
        }
        None => element.attrs.push(("style".to_string(), inline)),
    }
}

fn set_classes<'a>(element: &mut DomElement, classes: impl Iterator<Item = &'a str>) {
    let value = classes.collect::<Vec<_>>().join(" ");
    match element.attrs.iter_mut().find(|(name, _)| name == "class") {
        Some((_, existing)) => *existing = value,
        None => element.attrs.push(("class".to_string(), value)),
    }
}
