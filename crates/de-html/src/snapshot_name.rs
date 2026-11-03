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
/// `radix-:r2:`, `headlessui-menu-button-3`, `mat-select-2`, `ember123`).
fn meaningful_id(id: &str) -> bool {
    id.chars().count() <= MAX_CHARS
        && !id.contains(':')
        && !GENERATED_ID_PREFIXES
            .iter()
            .any(|prefix| id.starts_with(prefix))
        && id.chars().next().is_some_and(char::is_alphabetic)
        && !id.split(['-', '_']).any(looks_hashed)
}

/// A class that names the element rather than styling it.
fn meaningful_class(class: &str) -> bool {
    let mut segments = class.split(['-', '_']);
    // `c-PJLV`, `e-1x2y`: one-letter scopes CSS-in-JS libraries emit.
    let scoped = segments.next().is_some_and(|first| first.len() == 1);
    class.chars().next().is_some_and(char::is_alphabetic)
        && class
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        && !scoped
        && !UTILITY_WORDS.contains(&class)
        && !UTILITY_PREFIXES.iter().any(|prefix| {
            class
                .strip_prefix(prefix)
                .is_some_and(|rest| rest.starts_with('-'))
        })
        && !VALUE_PREFIXES.iter().any(|prefix| {
            class
                .strip_prefix(prefix)
                .and_then(|rest| rest.strip_prefix('-'))
                .is_some_and(|value| {
                    value.starts_with(|c: char| c.is_ascii_digit())
                        || UTILITY_VALUES.contains(&value.split('-').next().unwrap_or(value))
                })
        })
        && !GENERATED_CLASS_PREFIXES
            .iter()
            .any(|prefix| class.starts_with(prefix))
        && !class.split(['-', '_']).any(looks_hashed)
}

/// A generated hash segment: letters and digits mixed at hash length
/// (`x1y2z`, `a8f3k`), or a case pattern no word has (`kQxYzA`, `GQzVa`,
/// styled-components and CSS-module suffixes). A real name of that shape
/// (`step10`) is lost to the tag, which costs less than a hash as a name;
/// camel case (`sidebarNav`) and acronyms (`FAQSection`) stay names.
fn looks_hashed(segment: &str) -> bool {
    let mixed = segment.len() >= 5
        && segment.chars().any(|c| c.is_ascii_digit())
        && segment.chars().any(|c| c.is_ascii_alphabetic());
    let flips = segment
        .chars()
        .filter(char::is_ascii_alphabetic)
        .map(|c| c.is_ascii_uppercase())
        .collect::<Vec<_>>()
        .windows(2)
        .filter(|pair| pair[0] != pair[1])
        .count();
    mixed || (segment.len() >= 5 && flips >= 3)
}

/// "title-row" → "Title row", "sidebarNav" → "Sidebar nav", "URLInput" →
/// "Url input".
fn humanize(name: &str) -> String {
    let chars: Vec<char> = name.chars().collect();
    let mut words = String::new();
    for (at, &c) in chars.iter().enumerate() {
        if c == '-' || c == '_' {
            if !words.is_empty() && !words.ends_with(' ') {
                words.push(' ');
            }
            continue;
        }
        let previous = at.checked_sub(1).map(|before| chars[before]);
        let next = chars.get(at + 1);
        // A word starts at an upper-case letter after a lower-case one
        // (`sidebarNav`), or at the last capital of an acronym (`URLInput`).
        let starts_word = c.is_uppercase()
            && previous.is_some_and(|p| {
                p.is_lowercase()
                    || p.is_ascii_digit()
                    || (p.is_uppercase() && next.is_some_and(|n| n.is_lowercase()))
            });
        if starts_word && !words.ends_with(' ') {
            words.push(' ');
        }
        words.extend(c.to_lowercase());
    }
    let words = words.trim();
    let mut first = words.chars();
    match first.next() {
        Some(letter) => letter.to_uppercase().chain(first).collect(),
        None => name.to_string(),
    }
}

/// Id prefixes frameworks generate.
const GENERATED_ID_PREFIXES: &[&str] = &[
    "radix-",
    "headlessui-",
    "react-aria",
    "react-select-",
    "mui-",
    "mat-",
    "cdk-",
    "mantine-",
    "downshift-",
    "rc-",
    "ember",
    "__",
];

/// Utility classes that stand alone (no value after a dash): Tailwind,
/// Bootstrap.
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
    "overline",
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
    "collapse",
    "isolate",
    "grow",
    "shrink",
    "filter",
    "blur",
    "grayscale",
    "invert",
    "sepia",
    "prose",
    "dark",
    "ordinal",
    "sr-only",
    "not-sr-only",
    "tabular-nums",
    "line-through",
    "no-underline",
    "not-italic",
    "normal-case",
    "flow-root",
    "clearfix",
    "row",
    "col",
    "container-fluid",
    "visually-hidden",
    "fixed-top",
    "fixed-bottom",
    "sticky-top",
    "img-fluid",
    "img-thumbnail",
];

/// Utility families, matched as `<prefix>-<value>`.
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
    "inset",
    "flex",
    "grid",
    "items",
    "justify",
    "self",
    "place",
    "basis",
    "grow",
    "shrink",
    "overflow",
    "overscroll",
    "aspect",
    "columns",
    "break",
    "float",
    "clear",
    "decoration",
    "underline",
    "line-clamp",
    "whitespace",
    "align",
    "stroke",
    "cursor",
    "resize",
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
    "position",
    "translate-middle",
    "align-items",
    "justify-content",
];

/// Families whose word is also a real name (`top-bar`, `text-editor`,
/// `list-header`): a utility only when the rest is a value (`top-0`,
/// `text-muted-foreground`, `row-span-2`).
const VALUE_PREFIXES: &[&str] = &[
    "text", "font", "top", "right", "bottom", "left", "start", "end", "order", "object", "scroll",
    "fill", "content", "list", "box", "select", "row", "col", "table", "prose",
];

/// The values those families take: keywords, then the palette's colours.
const UTILITY_VALUES: &[&str] = &[
    "xs",
    "sm",
    "base",
    "md",
    "lg",
    "xl",
    "left",
    "center",
    "right",
    "justify",
    "start",
    "end",
    "top",
    "bottom",
    "between",
    "around",
    "evenly",
    "baseline",
    "stretch",
    "normal",
    "none",
    "auto",
    "full",
    "px",
    "span",
    "all",
    "text",
    "first",
    "last",
    "white",
    "black",
    "transparent",
    "current",
    "inherit",
    "muted",
    "primary",
    "secondary",
    "foreground",
    "background",
    "destructive",
    "accent",
    "card",
    "popover",
    "wrap",
    "nowrap",
    "balance",
    "pretty",
    "ellipsis",
    "clip",
    "sans",
    "serif",
    "mono",
    "thin",
    "extralight",
    "light",
    "medium",
    "semibold",
    "bold",
    "extrabold",
    "contain",
    "cover",
    "fill",
    "scale",
    "smooth",
    "m",
    "mx",
    "my",
    "mt",
    "mb",
    "ml",
    "mr",
    "p",
    "py",
    "pt",
    "pb",
    "pl",
    "pr",
    "disc",
    "decimal",
    "inside",
    "outside",
    "image",
    "border",
    "content",
    "decoration",
    "cell",
    "caption",
    "column",
    "fixed",
    "invert",
    "slate",
    "gray",
    "zinc",
    "neutral",
    "stone",
    "red",
    "orange",
    "amber",
    "yellow",
    "lime",
    "green",
    "emerald",
    "teal",
    "cyan",
    "sky",
    "blue",
    "indigo",
    "violet",
    "purple",
    "fuchsia",
    "pink",
    "rose",
];

/// Class names CSS-in-JS and component scoping generate.
const GENERATED_CLASS_PREFIXES: &[&str] = &[
    "css-", "sc-", "jsx-", "svelte-", "astro-", "emotion-", "tw-", "Mui", "chakra-", "mantine-",
    "ng-",
];

#[cfg(test)]
#[path = "snapshot_name_tests.rs"]
mod tests;
