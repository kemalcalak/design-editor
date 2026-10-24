//! `$variable` references for the pen.dev conversion.
//!
//! pen.dev lets almost any property name a design variable
//! (`"fontSize": "$size-body"`). The canonical schema keeps a reference only
//! where the render-time resolver (`de_editor_core::variables_resolve`) reads
//! one: paint colours, a number variable in node `opacity`, `gap` or a
//! whole-string `padding`, in a text `fontWeight`, and a string variable in
//! text `content`. Every other reference is replaced
//! here by the variable's value for the node's theme (the document default,
//! overlaid by the `theme` of each ancestor and of the node itself) and
//! counted in the report, so one field can never fail the whole file.

use std::collections::BTreeMap;

use serde_json::{Map, Value};

use super::PenDevReport;

/// Theme axis → active value, e.g. `{ "device": "mobile" }`.
pub(super) type Theme = BTreeMap<String, String>;

/// Node keys the conversion never reads as variable references: identity
/// and structure (walked separately).
pub(super) const SKIPPED_KEYS: &[&str] = &[
    "type",
    "id",
    "name",
    "ref",
    "theme",
    "children",
    "descendants",
];

/// Keys the canonical schema types as numbers; a reference that resolves to
/// anything else there is dropped instead of failing the load.
const NUMBER_KEYS: &[&str] = &[
    "x",
    "y",
    "rotation",
    "opacity",
    "fontSize",
    "letterSpacing",
    "lineHeight",
    "cornerRadius",
    "padding",
    "gap",
    "strokeWidth",
    "blur",
    "radius",
    "spread",
    "angle",
    "offset",
    "innerRadius",
    "startAngle",
    "sweepAngle",
    "polygonCount",
    "minWidth",
    "maxWidth",
    "minHeight",
    "maxHeight",
];

/// pen.dev paint keys: colour references stay bound inside them, and an
/// unknown name is left for the canonical resolver's built-in palette.
const PAINT_KEYS: &[&str] = &["fill", "stroke", "effect"];

/// Keys the canonical schema keeps as free strings (sizes take keywords and
/// expressions too): an unknown name there loads as written.
const TEXT_KEYS: &[&str] = &[
    "fontFamily",
    "icon",
    "iconFontName",
    "library",
    "iconFontFamily",
    "width",
    "height",
];

/// The document's `variables` table and default theme.
pub(super) struct Variables {
    table: Map<String, Value>,
    default_theme: Theme,
}

impl Variables {
    pub(super) fn from_document(doc: &Map<String, Value>) -> Self {
        let table = doc
            .get("variables")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        // The first value of every axis is the default theme.
        let default_theme = doc
            .get("themes")
            .and_then(Value::as_object)
            .map(|themes| {
                themes
                    .iter()
                    .filter_map(|(axis, values)| {
                        let first = values.as_array()?.first()?.as_str()?;
                        Some((axis.clone(), first.to_owned()))
                    })
                    .collect()
            })
            .unwrap_or_default();
        Self {
            table,
            default_theme,
        }
    }

    pub(super) fn default_theme(&self) -> &Theme {
        &self.default_theme
    }

    fn kind(&self, name: &str) -> Option<&str> {
        self.table.get(name)?.get("type")?.as_str()
    }

    /// The variable's value under `theme`, matched with the canonical
    /// resolver's rule (`variables_resolve::resolve_themed_value`): the most
    /// specific entry whose axes all hold the active values, else the first.
    pub(super) fn value(&self, name: &str, theme: &Theme) -> Option<Value> {
        let value = self.table.get(name)?.get("value")?;
        let Value::Array(entries) = value else {
            return Some(value.clone());
        };
        let mut best: Option<(usize, &Value)> = None;
        for entry in entries {
            let Some(axes) = entry
                .get("theme")
                .and_then(Value::as_object)
                .filter(|axes| !axes.is_empty())
            else {
                continue;
            };
            let matches = axes
                .iter()
                .all(|(axis, wanted)| theme.get(axis).map(String::as_str) == wanted.as_str());
            if matches && best.is_none_or(|(len, _)| axes.len() > len) {
                best = Some((axes.len(), entry));
            }
        }
        let entry = best.map(|(_, entry)| entry).or_else(|| entries.first())?;
        entry.get("value").cloned()
    }

    /// Whether `key: value` stays a live reference in the canonical
    /// document: what `variables_resolve::resolve_node_for_canvas` reads in
    /// place (numeric `opacity` / `gap` / whole-string `padding`, a numeric
    /// `fontWeight` on text, string `content`).
    pub(super) fn binds_in_place(&self, key: &str, value: &Value, is_text: bool) -> bool {
        match key {
            "opacity" | "gap" | "padding" => self.binds(value, "number"),
            "fontWeight" => is_text && self.binds(value, "number"),
            "content" => self.binds(value, "string"),
            _ => false,
        }
    }

    /// Whether `value` is a reference the canonical resolver reads in place
    /// as a variable of `kind`.
    fn binds(&self, value: &Value, kind: &str) -> bool {
        value
            .as_str()
            .and_then(reference)
            .is_some_and(|name| self.kind(name).is_none_or(|own| own == kind))
    }
}

/// `theme` with a node's own `theme` axes layered on top, or `None` when the
/// node has none and its parent's theme applies unchanged.
pub(super) fn scoped_theme(map: &Map<String, Value>, theme: &Theme) -> Option<Theme> {
    let own = map
        .get("theme")?
        .as_object()
        .filter(|own| !own.is_empty())?;
    let mut scoped = theme.clone();
    for (axis, value) in own {
        if let Some(value) = value.as_str() {
            scoped.insert(axis.clone(), value.to_owned());
        }
    }
    Some(scoped)
}

/// Replace the references in one node (or instance patch) that the canonical
/// schema cannot hold. Children and `descendants` are left to the node walk.
pub(super) fn resolve_node(
    map: &mut Map<String, Value>,
    variables: &Variables,
    theme: &Theme,
    report: &mut PenDevReport,
) {
    let is_text = map.get("type").and_then(Value::as_str) == Some("text");
    let mut dropped = Vec::new();
    for (key, value) in map.iter_mut() {
        let key = key.as_str();
        // What `variables_resolve::resolve_node_for_canvas` reads in place:
        // numeric `opacity` / `gap` / whole-string `padding`, a numeric
        // `fontWeight` on text, string `content`.
        if SKIPPED_KEYS.contains(&key) || variables.binds_in_place(key, value, is_text) {
            continue;
        }
        let slot = Slot {
            key,
            kind: SlotKind::of(key, SlotKind::Strict),
        };
        if !resolve_value(value, slot, variables, theme, report) {
            dropped.push(key.to_owned());
        }
    }
    for key in dropped {
        map.remove(&key);
    }
    // A number or boolean variable bound to text content shows as text.
    if let Some(content) = map
        .get_mut("content")
        .filter(|content| content.is_number() || content.is_boolean())
    {
        *content = Value::String(content.to_string());
    }
}

/// What the canonical schema accepts in a slot.
#[derive(Clone, Copy, PartialEq, Eq)]
enum SlotKind {
    /// A number only.
    Number,
    /// Paint: colours (bound) plus paint options.
    Paint,
    /// A free string.
    Text,
    /// A typed value (enum, bool, …) an unknown name would fail.
    Strict,
}

impl SlotKind {
    /// The kind of `key`, inside a value of kind `parent`.
    fn of(key: &str, parent: SlotKind) -> SlotKind {
        if parent == SlotKind::Number || NUMBER_KEYS.contains(&key) {
            SlotKind::Number
        } else if PAINT_KEYS.contains(&key) {
            SlotKind::Paint
        } else if TEXT_KEYS.contains(&key) {
            SlotKind::Text
        } else if parent == SlotKind::Paint {
            SlotKind::Paint
        } else {
            SlotKind::Strict
        }
    }
}

/// Where a value sits: the top-level node key (for the report label) and
/// what the schema accepts there.
#[derive(Clone, Copy)]
struct Slot<'a> {
    key: &'a str,
    kind: SlotKind,
}

/// Resolve the references inside `value`. Returns `false` when `value` is
/// itself a reference that cannot be resolved, so the caller drops it.
fn resolve_value(
    value: &mut Value,
    slot: Slot<'_>,
    variables: &Variables,
    theme: &Theme,
    report: &mut PenDevReport,
) -> bool {
    match value {
        Value::String(text) => {
            let Some(name) = reference(text) else {
                return true;
            };
            let kind = variables.kind(name);
            if slot.kind == SlotKind::Paint && kind.is_none_or(|kind| kind == "color") {
                return true;
            }
            let resolved = variables.value(name, theme).filter(|resolved| {
                // A value naming another variable is a cycle guard miss.
                let chained = resolved.as_str().and_then(reference).is_some();
                !chained && (slot.kind != SlotKind::Number || resolved.is_number())
            });
            if let Some(resolved) = resolved {
                *value = resolved;
                report.note(resolved_label(slot.key));
                return true;
            }
            report.note("unresolved variable");
            slot.kind == SlotKind::Text
        }
        Value::Array(items) => {
            for item in items.iter_mut() {
                if !resolve_value(item, slot, variables, theme, report) {
                    // Keep the array's shape (padding / radius sides).
                    *item = Value::from(0);
                }
            }
            true
        }
        Value::Object(obj) => {
            // Mesh paints are dropped with their references; shader paints
            // resolve their uniforms in `shader.rs`.
            if matches!(
                obj.get("type").and_then(Value::as_str),
                Some("shader" | "mesh_gradient")
            ) {
                return true;
            }
            let mut dropped = Vec::new();
            for (key, item) in obj.iter_mut() {
                let inner = Slot {
                    kind: SlotKind::of(key, slot.kind),
                    ..slot
                };
                if !resolve_value(item, inner, variables, theme, report) {
                    dropped.push(key.clone());
                }
            }
            for key in dropped {
                obj.remove(&key);
            }
            true
        }
        _ => true,
    }
}

pub(super) fn reference(text: &str) -> Option<&str> {
    text.strip_prefix('$').filter(|name| !name.is_empty())
}

/// Report entry for a reference replaced by its value.
fn resolved_label(key: &str) -> &'static str {
    match key {
        "fontSize" => "font size variable (fixed value)",
        "fontFamily" => "font family variable (fixed value)",
        "fontWeight" => "font weight variable (fixed value)",
        "letterSpacing" | "lineHeight" => "text spacing variable (fixed value)",
        "cornerRadius" => "corner radius variable (fixed value)",
        "padding" | "gap" => "spacing variable (fixed value)",
        "width" | "height" => "size variable (fixed value)",
        "strokeWidth" => "stroke width variable (fixed value)",
        "icon" | "iconFontName" | "library" | "iconFontFamily" => "icon variable (fixed value)",
        "fill" | "stroke" | "effect" => "paint variable (fixed value)",
        "enabled" => "visibility variable (fixed value)",
        "content" => "text variable (fixed value)",
        _ => "variable (fixed value)",
    }
}
