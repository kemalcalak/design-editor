//! Layer names for browser snapshot imports (#305).
//!
//! Naming every frame after its tag turned a real app's layer tree into
//! dozens of `div` rows. Each captured element is named from the most
//! meaningful thing the capture has, in order:
//!
//! 1. its `aria-label`, `title` or `alt` (an icon's name, for a vector
//!    icon);
//! 2. a landmark role or semantic tag, as a readable word ("Sidebar" for
//!    `aside`);
//! 3. for a button, link, heading or list / menu item, its text;
//! 4. a meaningful `id`, else the first class that is not a utility
//!    (Tailwind, Bootstrap spacing / display, CSS-module hashes);
//! 5. the tag.
//!
//! The extractor carries the raw attributes (`attrs`); the decision is
//! made here so it is tested in Rust.

use serde_json::{Map, Value};

/// Longest name kept from free text (labels, button text), in chars.
const MAX_CHARS: usize = 40;

/// The layer name for a captured element or media node.
pub(super) fn element_name(object: &Map<String, Value>) -> Option<String> {
    let attrs = object.get("attrs").and_then(Value::as_object);
    let attr = |key: &str| {
        attrs
            .and_then(|attrs| attrs.get(key))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
    };
    let tag = object.get("tag").and_then(Value::as_str);
    if let Some(label) = ["aria-label", "title", "alt"].into_iter().find_map(attr) {
        return Some(shorten(label));
    }
    // An icon drawn as a vector keeps its icon name, as an icon node does.
    if let Some(icon) = object
        .get("icon")
        .and_then(|icon| icon.get("names"))
        .and_then(|names| names.get(0))
        .and_then(Value::as_str)
    {
        return Some(icon.to_string());
    }
    let role = attr("role").map(str::to_ascii_lowercase);
    if let Some(word) = role.as_deref().and_then(landmark_role) {
        return Some(word.to_string());
    }
    if let Some(word) = tag.and_then(landmark_tag) {
        return Some(word.to_string());
    }
    if names_by_text(tag, role.as_deref()) {
        let mut text = String::new();
        collect_text(object, &mut text);
        if !text.trim().is_empty() {
            return Some(shorten(&text));
        }
    }
    if let Some(id) = attr("id").filter(|id| meaningful_id(id)) {
        return Some(humanize(id));
    }
    if let Some(class) = attr("class").and_then(|list| {
        list.split_whitespace()
            .find(|class| meaningful_class(class))
    }) {
        return Some(humanize(class));
    }
    tag.map(str::to_string)
}

fn landmark_role(role: &str) -> Option<&'static str> {
    Some(match role {
        "banner" => "Header",
        "navigation" => "Navigation",
        "complementary" => "Sidebar",
        "main" => "Main",
        "contentinfo" => "Footer",
        "form" => "Form",
        "search" => "Search",
        "dialog" | "alertdialog" => "Dialog",
        _ => return None,
    })
}

fn landmark_tag(tag: &str) -> Option<&'static str> {
    Some(match tag {
        "header" => "Header",
        "nav" => "Navigation",
        "aside" => "Sidebar",
        "main" => "Main",
        "footer" => "Footer",
        "form" => "Form",
        "dialog" => "Dialog",
        _ => return None,
    })
}

/// Elements a reader names by what they say: buttons, links, headings,
/// and the items of a list or menu (a sidebar's nav items).
fn names_by_text(tag: Option<&str>, role: Option<&str>) -> bool {
    matches!(
        tag,
        Some("button" | "a" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "li" | "summary")
    ) || matches!(
        role,
        Some(
            "button" | "link" | "heading" | "tab" | "menuitem" | "option" | "listitem" | "treeitem"
        )
    )
}

/// The captured text under `node`, in document order, space-separated;
/// stops once a name's worth is gathered.
fn collect_text(node: &Map<String, Value>, out: &mut String) {
    if out.chars().count() > MAX_CHARS {
        return;
    }
    if let Some(text) = node.get("text").and_then(Value::as_str) {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(text);
    }
    for child in node
        .get("children")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_object)
    {
        collect_text(child, out);
    }
}

/// Whitespace collapsed, cut to `MAX_CHARS` with "…".
fn shorten(text: &str) -> String {
    let words = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if words.chars().count() <= MAX_CHARS {
        return words;
    }
    let kept: String = words.chars().take(MAX_CHARS - 1).collect();
    format!("{}\u{2026}", kept.trim_end())
}

/// An id a person chose, not one a framework generated (`:r1:`,
/// `radix-:r2:`, `headlessui-menu-button-3`, `ember123`).
fn meaningful_id(id: &str) -> bool {
    const GENERATED: [&str; 6] = ["radix-", "headlessui-", "react-aria", "mui-", "ember", "__"];
    id.chars().count() <= MAX_CHARS
        && !id.contains(':')
        && !GENERATED.iter().any(|prefix| id.starts_with(prefix))
        && id.chars().next().is_some_and(char::is_alphabetic)
        && !id.split(['-', '_']).any(looks_hashed)
}

/// A class that names the element rather than styling it.
fn meaningful_class(class: &str) -> bool {
    class.chars().next().is_some_and(char::is_alphabetic)
        && class
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        && !UTILITY_WORDS.contains(&class)
        && !UTILITY_PREFIXES.iter().any(|prefix| {
            class
                .strip_prefix(prefix)
                .is_some_and(|rest| rest.starts_with('-'))
        })
        && !GENERATED_CLASS_PREFIXES
            .iter()
            .any(|prefix| class.starts_with(prefix))
        && !class.split(['-', '_']).any(looks_hashed)
}

/// A segment mixing letters and digits at hash length (`x1y2z`, `a8f3k`):
/// CSS-module and CSS-in-JS class suffixes.
fn looks_hashed(segment: &str) -> bool {
    segment.len() >= 5
        && segment.chars().any(|c| c.is_ascii_digit())
        && segment.chars().any(|c| c.is_ascii_alphabetic())
}

/// "title-row" → "Title row", "sidebarNav" → "Sidebar nav".
fn humanize(name: &str) -> String {
    let mut words = String::new();
    let mut previous_lower = false;
    for c in name.chars() {
        if c == '-' || c == '_' {
            if !words.ends_with(' ') && !words.is_empty() {
                words.push(' ');
            }
            previous_lower = false;
            continue;
        }
        if c.is_uppercase() && previous_lower {
            words.push(' ');
        }
        previous_lower = c.is_lowercase() || c.is_ascii_digit();
        words.extend(c.to_lowercase());
    }
    let words = words.trim();
    let mut chars = words.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => name.to_string(),
    }
}

/// Utility classes that stand alone (no value after a dash).
const UTILITY_WORDS: &[&str] = &[
    "flex",
    "grid",
    "block",
    "inline",
    "contents",
    "table",
    "hidden",
    "relative",
    "absolute",
    "fixed",
    "sticky",
    "static",
    "truncate",
    "container",
    "group",
    "peer",
    "italic",
    "underline",
    "uppercase",
    "lowercase",
    "capitalize",
    "antialiased",
    "border",
    "rounded",
    "shadow",
    "outline",
    "ring",
    "transition",
    "transform",
    "visible",
    "invisible",
    "isolate",
    "grow",
    "shrink",
    "filter",
    "prose",
    "dark",
    "sr-only",
    "not-sr-only",
    "tabular-nums",
    "line-through",
    "no-underline",
    "not-italic",
    "normal-case",
    "clearfix",
    "row",
    "col",
];

/// Utility families, matched as `<prefix>-<value>` (Tailwind, Bootstrap).
const UTILITY_PREFIXES: &[&str] = &[
    "p",
    "px",
    "py",
    "pt",
    "pr",
    "pb",
    "pl",
    "ps",
    "pe",
    "m",
    "mx",
    "my",
    "mt",
    "mr",
    "mb",
    "ml",
    "ms",
    "me",
    "w",
    "h",
    "min-w",
    "min-h",
    "max-w",
    "max-h",
    "size",
    "gap",
    "space",
    "text",
    "font",
    "leading",
    "tracking",
    "bg",
    "from",
    "via",
    "to",
    "border",
    "rounded",
    "shadow",
    "ring",
    "outline",
    "opacity",
    "z",
    "top",
    "right",
    "bottom",
    "left",
    "inset",
    "start",
    "end",
    "flex",
    "grid",
    "col",
    "row",
    "items",
    "justify",
    "content",
    "self",
    "place",
    "order",
    "basis",
    "grow",
    "shrink",
    "overflow",
    "overscroll",
    "object",
    "aspect",
    "columns",
    "break",
    "box",
    "float",
    "clear",
    "decoration",
    "underline",
    "line-clamp",
    "list",
    "whitespace",
    "align",
    "fill",
    "stroke",
    "cursor",
    "select",
    "resize",
    "scroll",
    "snap",
    "touch",
    "pointer-events",
    "transition",
    "duration",
    "ease",
    "delay",
    "animate",
    "translate",
    "rotate",
    "scale",
    "skew",
    "origin",
    "blur",
    "brightness",
    "contrast",
    "drop-shadow",
    "grayscale",
    "hue-rotate",
    "invert",
    "saturate",
    "sepia",
    "backdrop",
    "divide",
    "will-change",
    "appearance",
    "accent",
    "caret",
    "auto-cols",
    "auto-rows",
    "inline",
    "mix-blend",
    "bg-blend",
    "isolation",
    "transform",
    "d",
    "g",
    "gx",
    "gy",
    "fs",
    "fw",
    "lh",
    "rounded",
    "shadow",
    "position",
    "translate-middle",
    "align-items",
    "justify-content",
    "data",
    "aria",
    "group",
    "peer",
];

/// Class names CSS-in-JS and component scoping generate.
const GENERATED_CLASS_PREFIXES: &[&str] = &[
    "css-", "sc-", "jsx-", "svelte-", "astro-", "emotion-", "tw-", "Mui", "chakra-", "mantine-",
    "ng-",
];

#[cfg(test)]
#[path = "snapshot_name_tests.rs"]
mod tests;
