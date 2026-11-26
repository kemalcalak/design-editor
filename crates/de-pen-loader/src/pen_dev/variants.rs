//! pen.dev component variants (#368).
//!
//! pen.dev groups a component's variants in a `variants` node: its
//! `reusable` object names the axes (axis → values, the first value is the
//! default) and each top-level reusable descendant carries `reusable:
//! {axis: value}` (an axis it leaves out matches any value). A `ref` to the
//! `variants` node picks one with `variant`.
//!
//! The canonical form is a frame with `componentSet` whose variant masters
//! are `reusable: true` frames with `variantValues`, which is the same
//! model: the node becomes a frame, the axes move to `componentSet`, each
//! variant's object to `variantValues`. Instances keep `ref` on the set
//! and their `variant`; opening the document points them at the master
//! they show (`de_editor_core::component_set::normalize_set_refs`).

use serde_json::{Map, Value};

/// Turn a `variants` node into a component-set frame, or a variant's
/// `reusable: {…}` into `reusable: true` plus `variantValues`.
pub(super) fn convert(map: &mut Map<String, Value>) {
    if map.get("type").and_then(Value::as_str) == Some("variants") {
        map.insert("type".to_owned(), Value::String("frame".to_owned()));
        let axes = match map.remove("reusable") {
            Some(Value::Object(axes)) => axes
                .into_iter()
                .filter_map(|(axis, values)| {
                    let values: Vec<Value> = values
                        .as_array()?
                        .iter()
                        .filter(|value| value.is_string())
                        .cloned()
                        .collect();
                    Some((axis, Value::Array(values)))
                })
                .collect(),
            _ => Map::new(),
        };
        let mut set = Map::new();
        set.insert("axes".to_owned(), Value::Object(axes));
        map.insert("componentSet".to_owned(), Value::Object(set));
        return;
    }
    if map.get("type").and_then(Value::as_str) == Some("ref") {
        return;
    }
    if let Some(Value::Object(values)) = map.get("reusable") {
        let values: Map<String, Value> = values
            .iter()
            .filter(|(_, value)| value.is_string())
            .map(|(axis, value)| (axis.clone(), value.clone()))
            .collect();
        map.insert("reusable".to_owned(), Value::Bool(true));
        map.insert("variantValues".to_owned(), Value::Object(values));
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::pen_dev::{convert, no_assets};

    /// A pen.dev Button with variant × size, one combination missing, and
    /// two instances: one naming a variant, one taking the defaults.
    fn source() -> serde_json::Value {
        json!({"version": "2.9", "children": [
            {"type": "variants", "id": "btn", "name": "Button",
             "reusable": {"variant": ["primary", "ghost"], "size": ["md", "sm"]},
             "children": [
               {"type": "frame", "id": "pm", "reusable": {"variant": "primary", "size": "md"},
                "width": 120, "height": 40,
                "children": [{"type": "text", "id": "pm-label", "content": "Go"}]},
               {"type": "frame", "id": "ps", "reusable": {"variant": "primary", "size": "sm"},
                "width": 90, "height": 32,
                "children": [{"type": "text", "id": "ps-label", "content": "Go"}]},
               {"type": "frame", "id": "gm", "reusable": {"variant": "ghost"},
                "width": 120, "height": 40,
                "children": [{"type": "text", "id": "gm-label", "content": "Go"}]}]},
            {"type": "ref", "id": "i1", "ref": "btn", "variant": {"size": "sm"}, "x": 0, "y": 100,
             "cornerRadius": 4},
            {"type": "ref", "id": "i2", "ref": "btn", "variant": {"variant": "ghost", "size": "sm"},
             "x": 200, "y": 100}
        ]})
    }

    #[test]
    fn variants_nodes_become_component_sets_without_loss() {
        let mut root = source();
        let report = convert(&mut root, &no_assets);
        assert!(report.unsupported.is_empty(), "{:?}", report.unsupported);
        let set = &root["children"][0];
        assert_eq!(set["type"], "frame");
        assert_eq!(
            set["componentSet"]["axes"],
            json!({"variant": ["primary", "ghost"], "size": ["md", "sm"]})
        );
        assert_eq!(set["children"][2]["reusable"], true);
        assert_eq!(
            set["children"][2]["variantValues"],
            json!({"variant": "ghost"})
        );
        let instance = &root["children"][1];
        assert_eq!(
            instance["variant"],
            json!({"size": "sm"}),
            "not a root override"
        );
        assert_eq!(instance["descendants"]["btn"]["cornerRadius"], 4);

        // Opened, the instances point at the masters they show.
        let doc = jian_ops_schema::load_str(&root.to_string()).unwrap().value;
        let state = de_editor_core::EditorState::from_opened_document(doc);
        let i1 = serde_json::to_value(
            de_editor_core::component_set::find_node(&state.doc, "i1").unwrap(),
        )
        .unwrap();
        assert_eq!(i1["ref"], "ps");
        assert_eq!(i1["variant"], json!({"size": "sm", "variant": "primary"}));
        assert_eq!(
            i1["descendants"]["ps"]["cornerRadius"], 4.0,
            "the root override follows"
        );
        // ghost/sm: `gm` leaves size out, so it matches any size.
        let i2 = serde_json::to_value(
            de_editor_core::component_set::find_node(&state.doc, "i2").unwrap(),
        )
        .unwrap();
        assert_eq!(i2["ref"], "gm");
    }
}
