//! Slides a deck viewer hides until its script shows them.
//!
//! A player keeps every slide but the current one at `display: none` (or
//! invisible) and adds a state class such as `.on` / `.active` to the one on
//! screen. The importer runs no script, so it does what the script would:
//! it adds the first state class the stylesheet answers with a visible
//! slide, and falls back to an inline style when none does.

use crate::css::cascade::ComputedStyle;
use crate::dom::DomElement;

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

/// The element is not on screen: hidden by its style or by an attribute.
/// [`revealed_attrs`] undoes exactly these.
pub(crate) fn needs_showing(element: &DomElement, style: &ComputedStyle) -> bool {
    is_hidden(style) || element.attrs.iter().any(|(name, value)| hides(name, value))
}

/// `hidden`, or `aria-hidden="true"`.
fn hides(name: &str, value: &str) -> bool {
    name == "hidden" || (name == "aria-hidden" && value == "true")
}

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

/// An element's attributes, as the DOM keeps them.
pub(crate) type Attrs = [(String, String)];

/// The attributes that make an element render the way the player shows it
/// on screen. `style_of` computes the element's style with a candidate set
/// of attributes, in place in the document: sibling selectors such as
/// `.slide + .slide { display: none }` only match there.
pub(crate) fn revealed_attrs(
    attrs: &Attrs,
    style_of: &mut dyn FnMut(&Attrs) -> ComputedStyle,
) -> Vec<(String, String)> {
    let base: Vec<(String, String)> = attrs
        .iter()
        .filter(|(name, value)| !hides(name, value))
        .cloned()
        .collect();
    let style = style_of(&base);
    if !is_hidden(&style) {
        return base;
    }
    let classes: Vec<&str> = base
        .iter()
        .find(|(name, _)| name == "class")
        .map(|(_, value)| value.split_whitespace().collect())
        .unwrap_or_default();
    for state in STATE_CLASSES {
        if classes.contains(&state) {
            continue;
        }
        let candidate = with_attr(
            &base,
            "class",
            &[classes.join(" "), state.to_string()].join(" "),
        );
        if !is_hidden(&style_of(&candidate)) {
            return candidate;
        }
    }
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
    let existing = base
        .iter()
        .find(|(name, _)| name == "style")
        .map(|(_, value)| format!("{value};"))
        .unwrap_or_default();
    with_attr(&base, "style", &format!("{existing}{inline}"))
}

/// `attrs` with `name` set to `value` (replaced, or appended).
fn with_attr(attrs: &Attrs, name: &str, value: &str) -> Vec<(String, String)> {
    let mut attrs = attrs.to_vec();
    match attrs.iter_mut().find(|(existing, _)| existing == name) {
        Some((_, existing)) => *existing = value.trim().to_string(),
        None => attrs.push((name.to_string(), value.trim().to_string())),
    }
    attrs
}
