//! pen.dev variable references on typed fields become live bindings.

use serde_json::{json, Value};

use crate::payload::load_canonical_with_assets;
use crate::pen_dev::{convert, no_assets};

fn document(children: Value) -> Value {
    json!({
        "version": "2.21",
        "themes": { "device": ["desktop", "mobile"] },
        "variables": {
            "h1": { "type": "number", "value": [
                { "value": 56, "theme": { "device": "desktop" } },
                { "value": 32, "theme": { "device": "mobile" } }
            ] },
            "face": { "type": "string", "value": "Geologica" },
            "space": { "type": "number", "value": 24 },
            "round": { "type": "number", "value": 8 },
            "line": { "type": "number", "value": 2 },
            "shown": { "type": "boolean", "value": true },
            "glyph": { "type": "string", "value": "plus" },
            "glow": { "type": "number", "value": 12 }
        },
        "children": children
    })
}

#[test]
fn typed_field_references_become_bindings_with_their_values() {
    let mut doc = document(json!([
        { "type": "frame", "id": "card", "layout": "vertical", "padding": ["$space", 8],
          "cornerRadius": [0, "$round", 0, 0], "stroke": "#000000", "strokeWidth": "$line",
          "enabled": "$shown", "effect": { "type": "shadow", "blur": "$glow" },
          "children": [
            { "type": "text", "id": "title", "content": "a", "fontSize": "$h1", "fontFamily": "$face" },
            { "type": "icon", "id": "add", "icon": "$glyph", "library": "lucide", "width": 16, "height": 16 }
          ] }
    ]));
    let report = convert(&mut doc, &no_assets);
    let card = &doc["children"][0];
    let bound = &card["variableBindings"];
    assert_eq!(card["padding"], json!([24, 8]));
    assert_eq!(
        bound["padding/0"],
        json!({ "variable": "space", "value": 24 })
    );
    assert_eq!(bound["cornerRadius/1"]["variable"], "round");
    assert_eq!(
        bound["stroke/thickness"],
        json!({ "variable": "line", "value": 2 })
    );
    assert_eq!(card["stroke"]["thickness"], 2);
    assert_eq!(bound["visible"]["variable"], "shown");
    assert_eq!(card["visible"], true);
    let title = card["children"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "title")
        .unwrap();
    assert_eq!(title["fontSize"], 56);
    assert_eq!(title["variableBindings"]["fontSize"]["variable"], "h1");
    assert_eq!(
        title["variableBindings"]["fontFamily"]["value"],
        "Geologica"
    );
    let icon = card["children"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "add")
        .unwrap();
    assert_eq!(
        icon["variableBindings"]["iconFontName"]["variable"],
        "glyph"
    );
    // Only the effect value, inside a paint list, stays fixed.
    let fixed: Vec<&str> = report
        .unsupported
        .keys()
        .copied()
        .filter(|k| k.contains("fixed"))
        .collect();
    assert_eq!(
        fixed,
        vec!["paint variable (fixed value)"],
        "{}",
        report.summary()
    );
}

#[test]
fn a_binding_loads_and_follows_the_theme_on_the_canvas() {
    let src = document(json!([
        { "type": "frame", "id": "screen", "layout": "vertical", "theme": { "device": "mobile" }, "children": [
            { "type": "text", "id": "title", "content": "a", "fontSize": "$h1" }
        ] }
    ]))
    .to_string();
    let loaded = load_canonical_with_assets(&src, &no_assets).expect("loads");
    let doc = loaded.loaded.value;
    let text = serde_json::to_value(&doc.children[0]).unwrap()["children"][0].clone();
    // Resolved for the node's own theme at import, and still bound.
    assert_eq!(text["fontSize"], 32.0);
    assert_eq!(text["variableBindings"]["fontSize"]["variable"], "h1");
    let desktop = de_editor_core::variables_resolve::resolve_document_for_canvas(
        &{
            let mut doc = doc.clone();
            if let jian_ops_schema::node::PenNode::Frame(frame) = &mut doc.children[0] {
                frame.base.theme = None;
            }
            doc
        },
        &Default::default(),
    );
    let text = serde_json::to_value(&desktop.children[0]).unwrap()["children"][0].clone();
    // Without the mobile theme the binding resolves to the desktop value.
    assert_eq!(text["fontSize"], 56.0);
}
