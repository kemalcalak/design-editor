//! Default layer names: the names the editor (and the importers it reads
//! from) hand out before anyone names a layer — `Frame`, `Rectangle 3`,
//! `Group 12 copy` — in English and in every locale the editor ships.

use std::collections::HashSet;
use std::sync::OnceLock;

use de_i18n::Locale;

/// The English kind names the editor and the Figma / SVG / HTML importers
/// give new layers. Lower-cased; compared case-insensitively.
const ENGLISH_DEFAULTS: &[&str] = &[
    "frame",
    "group",
    "rectangle",
    "rect",
    "ellipse",
    "polygon",
    "star",
    "line",
    "path",
    "vector",
    "text",
    "image",
    "component",
    "instance",
    "section",
    "union",
    "subtract",
    "intersect",
    "exclude",
    "boolean",
    "layer",
    "text input",
    "text_input",
    "text area",
    "text_area",
    "number input",
    "number_input",
    "select",
    "radio group",
    "radio_group",
    "switch",
    "checkbox",
    "slider",
    "progress",
    "tabs",
    "note",
    "browser",
];

/// Catalog keys whose localized label is the kind name a user of that
/// locale sees on the toolbar, and so the default name a localized design
/// tool (or a person copying the toolbar label) gives a new layer.
const LOCALIZED_KIND_KEYS: &[&str] = &[
    "toolbar.frame",
    "toolbar.text",
    "shapes.rectangle",
    "shapes.ellipse",
    "shapes.polygon",
    "shapes.star",
    "shapes.line",
    "menu.group",
];

fn default_words() -> &'static HashSet<String> {
    static WORDS: OnceLock<HashSet<String>> = OnceLock::new();
    WORDS.get_or_init(|| {
        let mut words: HashSet<String> = ENGLISH_DEFAULTS.iter().map(|w| w.to_string()).collect();
        for locale in Locale::ALL {
            for key in LOCALIZED_KIND_KEYS {
                words.insert(de_i18n::translate(locale, key).to_lowercase());
            }
        }
        words
    })
}

/// Drop the counter and copy suffixes a default name collects:
/// `Frame 12` → `Frame`, `Group 3 copy 2` → `Group`.
fn strip_counter(name: &str) -> &str {
    let mut rest = name.trim();
    loop {
        let Some((head, last)) = rest.rsplit_once(char::is_whitespace) else {
            return rest;
        };
        let digits = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit());
        let counter = digits(last)
            || last.strip_prefix('#').is_some_and(digits)
            || last.eq_ignore_ascii_case("copy");
        if !counter {
            return rest;
        }
        rest = head.trim_end();
    }
}

/// True when `name` is a default the editor or an importer handed out
/// (`Frame`, `Rectangle 3`, `Çerçeve 2`, `矩形 1`), so it says nothing
/// about what the layer is for.
pub fn is_default_layer_name(name: &str) -> bool {
    let base = strip_counter(name);
    !base.is_empty() && default_words().contains(&base.to_lowercase())
}
