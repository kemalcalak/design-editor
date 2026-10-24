//! Typed-field variable bindings: themed resolution, edited fields,
//! instances, renames and deletes.

use serde_json::{json, Value};

use crate::ref_resolve::resolve_refs_for_canvas;
use crate::variables_resolve::{replace_variable_refs_in_tree, resolve_document_for_canvas, Theme};
use jian_ops_schema::PenDocument;

fn doc(children: Value) -> PenDocument {
    let src = json!({
        "version": "1.0",
        "themes": { "device": ["desktop", "mobile"] },
        "variables": {
            "title": { "type": "number", "value": [
                { "value": 32, "theme": { "device": "desktop" } },
                { "value": 22, "theme": { "device": "mobile" } }
            ] },
            "face": { "type": "string", "value": "Inter" },
            "gap": { "type": "number", "value": 12 }
        },
        "children": children
    });
    jian_ops_schema::load_str(&src.to_string())
        .expect("fixture parses")
        .value
}

fn heading(id: &str, size: f64) -> Value {
    json!({ "type": "text", "id": id, "content": "a", "fontSize": size, "fontFamily": "Inter",
            "variableBindings": {
                "fontSize": { "variable": "title", "value": 32 },
                "fontFamily": { "variable": "face", "value": "Inter" }
            } })
}

fn theme(device: &str) -> Theme {
    Theme::from([("device".to_owned(), device.to_owned())])
}

fn field(doc: &PenDocument, index: usize, key: &str) -> Value {
    serde_json::to_value(&doc.children[index]).unwrap()[key].clone()
}

#[test]
fn a_bound_field_takes_the_variable_value_for_the_theme() {
    let source = doc(json!([heading("t", 32.0)]));
    let desktop = resolve_document_for_canvas(&source, &theme("desktop"));
    let mobile = resolve_document_for_canvas(&source, &theme("mobile"));
    assert_eq!(field(&desktop, 0, "fontSize"), json!(32.0));
    assert_eq!(field(&mobile, 0, "fontSize"), json!(22.0));
    // The stored document keeps its fallback.
    assert_eq!(field(&source, 0, "fontSize"), json!(32.0));
}

#[test]
fn an_edited_field_no_longer_follows_its_variable() {
    let source = doc(json!([heading("t", 40.0)]));
    let mobile = resolve_document_for_canvas(&source, &theme("mobile"));
    assert_eq!(field(&mobile, 0, "fontSize"), json!(40.0));
}

#[test]
fn a_component_binding_resolves_under_the_instance_theme() {
    let source = doc(json!([
        { "type": "frame", "id": "card", "reusable": true, "children": [heading("t", 32.0)] },
        { "type": "ref", "id": "phone-card", "ref": "card", "theme": { "device": "mobile" } },
        { "type": "ref", "id": "big-card", "ref": "card", "theme": { "device": "mobile" },
          "descendants": { "t": { "fontSize": 50 } } }
    ]));
    let expanded = resolve_refs_for_canvas(&source);
    let resolved = resolve_document_for_canvas(&expanded, &theme("desktop"));
    let child_size = |index: usize| {
        serde_json::to_value(&resolved.children[index]).unwrap()["children"][0]["fontSize"].clone()
    };
    assert_eq!(child_size(0), json!(32.0));
    assert_eq!(child_size(1), json!(22.0));
    // An instance override of the field wins over the binding.
    assert_eq!(child_size(2), json!(50.0));
}

#[test]
fn a_renamed_variable_keeps_the_binding_and_a_deleted_one_leaves_its_value() {
    let mut source = doc(json!([heading("t", 32.0)]));
    let vars = source.variables.clone();
    replace_variable_refs_in_tree(
        &mut source.children,
        "title",
        Some("headline"),
        vars.as_ref(),
        &theme("mobile"),
    );
    let bindings = field(&source, 0, "variableBindings");
    assert_eq!(bindings["fontSize"]["variable"], "headline");
    replace_variable_refs_in_tree(
        &mut source.children,
        "face",
        None,
        vars.as_ref(),
        &theme("mobile"),
    );
    let bindings = field(&source, 0, "variableBindings");
    assert!(bindings.get("fontFamily").is_none());
    assert_eq!(bindings["fontSize"]["value"], json!(32));
    assert_eq!(field(&source, 0, "fontFamily"), json!("Inter"));
}

#[test]
fn a_bound_padding_side_and_mesh_colours_resolve() {
    let source = doc(
        json!([{ "type": "frame", "id": "f", "padding": [4, 12], "fill": [
        { "type": "mesh_gradient", "rows": 2, "cols": 2, "stops": [
            { "row": 0, "col": 0, "color": "$ink" }, { "row": 0, "col": 1, "color": "#FFFFFF" },
            { "row": 1, "col": 0, "color": "#FFFFFF" }, { "row": 1, "col": 1, "color": "#FFFFFF" } ] }
      ],
      "variableBindings": { "padding/1": { "variable": "gap", "value": 12 } } }]),
    );
    let mut source = source;
    let mut vars = source.variables.clone().unwrap();
    vars.insert(
        "ink".to_owned(),
        serde_json::from_value(json!({ "type": "color", "value": "#112233" })).unwrap(),
    );
    vars.insert(
        "gap".to_owned(),
        serde_json::from_value(json!({ "type": "number", "value": 20 })).unwrap(),
    );
    source.variables = Some(vars);
    let resolved = resolve_document_for_canvas(&source, &theme("desktop"));
    assert_eq!(field(&resolved, 0, "padding"), json!([4.0, 20.0]));
    assert_eq!(
        field(&resolved, 0, "fill")[0]["stops"][0]["color"],
        "#112233"
    );
}
