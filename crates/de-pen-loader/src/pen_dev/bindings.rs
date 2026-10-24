//! Variable bindings for the pen.dev conversion.
//!
//! `variables::resolve_node` resolves a `$variable` on a typed field to its
//! value (the canonical field cannot hold a reference) and records the
//! binding under its pen.dev path. Once the node is converted, [`finish`]
//! moves each binding onto the canonical field path and writes
//! `variableBindings` (`PenNodeBase::variable_bindings`), which the
//! renderer resolves per theme. A binding whose field the conversion
//! changed is dropped and reported as a fixed value.

use serde_json::{Map, Value};

use super::variables::BINDINGS_KEY;
use super::PenDevReport;

/// Move the recorded bindings onto the canonical field paths, keeping those
/// whose field still holds the bound value.
pub(super) fn finish(map: &mut Map<String, Value>, report: &mut PenDevReport) {
    let Some(Value::Object(found)) = map.remove(BINDINGS_KEY) else {
        return;
    };
    let mut kept = Map::new();
    for (path, binding) in found {
        let (root, rest) = path
            .split_once('/')
            .map_or((path.as_str(), ""), |(a, b)| (a, b));
        // Where the conversion put the field now, and its final canonical
        // name (the loader's legacy pass renames icon keys later).
        let (now, canonical) = match root {
            "strokeWidth" => ("stroke/thickness", "stroke/thickness"),
            "enabled" => ("visible", "visible"),
            "icon" => ("icon", "iconFontName"),
            "library" => ("library", "iconFontFamily"),
            other => (other, other),
        };
        if root == "enabled" && !map.contains_key("visible") {
            // `enabled: true` leaves no `visible` key; the binding needs the
            // field to switch.
            map.insert("visible".to_owned(), binding["value"].clone());
        }
        let field = field_at(map, now, rest);
        if field.is_some_and(|field| same_value(field, &binding["value"])) {
            kept.insert(join(canonical, rest), binding);
        } else {
            report.note("variable binding (fixed value)");
        }
    }
    if !kept.is_empty() {
        map.insert(BINDINGS_KEY.to_owned(), Value::Object(kept));
    }
    // An instance patch has no `type`, so the loader's icon-node rename never
    // reaches it: move its icon keys to the canonical names here.
    if !map.contains_key("type") {
        for (from, to) in [("icon", "iconFontName"), ("library", "iconFontFamily")] {
            if let Some(value) = map.remove(from) {
                map.insert(to.to_owned(), value);
            }
        }
    }
}

/// The value at `root/rest` (root may itself be a `a/b` path).
fn field_at<'a>(map: &'a Map<String, Value>, root: &str, rest: &str) -> Option<&'a Value> {
    let mut segments = root
        .split('/')
        .chain(rest.split('/').filter(|s| !s.is_empty()));
    let mut value = map.get(segments.next()?)?;
    for segment in segments {
        value = match value {
            Value::Object(obj) => obj.get(segment)?,
            Value::Array(items) => items.get(segment.parse::<usize>().ok()?)?,
            _ => return None,
        };
    }
    Some(value)
}

fn join(root: &str, rest: &str) -> String {
    if rest.is_empty() {
        root.to_owned()
    } else {
        format!("{root}/{rest}")
    }
}

/// Equal values, numbers compared as floats.
fn same_value(a: &Value, b: &Value) -> bool {
    match (a.as_f64(), b.as_f64()) {
        (Some(a), Some(b)) => (a - b).abs() <= 1e-6 * a.abs().max(b.abs()).max(1.0),
        _ => a == b,
    }
}

#[cfg(test)]
#[path = "bindings_tests.rs"]
mod tests;
