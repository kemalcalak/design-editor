//! pen.dev `imports` → linked component libraries (#15).
//!
//! A pen.dev document declares the files it borrows components from as
//! `imports: { alias: "./kit.pen" }`, and points at their components and
//! variables with the alias as a prefix (`ref: "kit:abc"`, `"$kit:brand"`).
//! The canonical document keeps library masters with their own ids, so the
//! import records each entry as a link and drops the prefixes.

use serde_json::{Map, Value};

/// One `imports` entry of a pen.dev document.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PenDevImport {
    /// The short alias (the `imports` key).
    pub alias: String,
    /// The imported file, relative to the pen.dev document.
    pub path: String,
}

/// Remove `imports` from `doc` and return its string entries in key order.
pub(super) fn take_imports(doc: &mut Map<String, Value>) -> Vec<PenDevImport> {
    let Some(Value::Object(imports)) = doc.remove("imports") else {
        return Vec::new();
    };
    imports
        .into_iter()
        .filter_map(|(alias, path)| match path {
            Value::String(path) if !alias.is_empty() && !path.trim().is_empty() => {
                Some(PenDevImport { alias, path })
            }
            _ => None,
        })
        .collect()
}

/// Drop `alias:` from `ref` targets and `$alias:` from variable bindings.
pub(super) fn strip_alias_prefixes(value: &mut Value, imports: &[PenDevImport]) {
    if imports.is_empty() {
        return;
    }
    match value {
        Value::Object(map) => {
            for (key, child) in map.iter_mut() {
                match child {
                    Value::String(text) if key == "ref" => {
                        if let Some(rest) = strip_alias(text, imports) {
                            *text = rest;
                        }
                    }
                    other => strip_alias_prefixes(other, imports),
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                strip_alias_prefixes(item, imports);
            }
        }
        // Only a whole binding (`$kit:brand`), never prose that happens
        // to start with a dollar sign.
        Value::String(text) if !text.contains(char::is_whitespace) => {
            if let Some(name) = text.strip_prefix('$') {
                if let Some(rest) = strip_alias(name, imports) {
                    *text = format!("${rest}");
                }
            }
        }
        _ => {}
    }
}

fn strip_alias(text: &str, imports: &[PenDevImport]) -> Option<String> {
    let (alias, rest) = text.split_once(':')?;
    imports
        .iter()
        .any(|import| import.alias == alias)
        .then(|| rest.to_string())
}

#[cfg(test)]
mod tests {
    use super::super::{convert, no_assets};
    use super::*;
    use serde_json::json;

    #[test]
    fn imports_become_links_and_prefixes_are_dropped() {
        let mut doc = json!({
            "version": "2.21",
            "imports": { "kit": "./kit.pen", "bad": 3 },
            "children": [
                { "type": "ref", "id": "i1", "ref": "kit:btn", "x": 0, "y": 0 },
                { "type": "rectangle", "id": "r", "fill": "$kit:brand", "width": 4, "height": 4 },
                { "type": "ref", "id": "i2", "ref": "local", "x": 0, "y": 0 }
            ]
        });
        let report = convert(&mut doc, &no_assets);
        assert_eq!(
            report.imports,
            vec![PenDevImport {
                alias: "kit".into(),
                path: "./kit.pen".into()
            }]
        );
        assert!(doc.get("imports").is_none());
        let text = doc.to_string();
        assert!(text.contains(r#""ref":"btn""#), "{text}");
        assert!(text.contains("$brand"), "{text}");
        assert!(text.contains(r#""ref":"local""#), "{text}");
        assert!(!report.unsupported.contains_key("imported .pen libraries"));
    }
}
