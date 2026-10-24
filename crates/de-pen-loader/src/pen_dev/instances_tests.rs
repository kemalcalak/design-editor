//! Instance root overrides and partial stroke patches.

use de_editor_core::ref_resolve;
use jian_ops_schema::node::PenNode;
use jian_ops_schema::sizing::SizingBehavior;
use jian_ops_schema::PenDocument;
use serde_json::{json, Value};

use crate::payload::load_canonical_with_assets;
use crate::pen_dev::{convert, no_assets, PenDevReport};

fn badge() -> Value {
    json!({
        "type": "frame", "id": "badge", "reusable": true, "width": 120, "height": 40,
        "layout": "horizontal", "padding": [8, 16], "justifyContent": "start",
        "fill": "#FFFFFF", "stroke": "#222222", "strokeWidth": 1.5,
        "children": [
            { "type": "rectangle", "id": "dot", "width": 8, "height": 8,
              "stroke": "#00AA00", "strokeWidth": { "bottom": 2 } }
        ]
    })
}

fn load(children: Value) -> (PenDocument, PenDevReport) {
    let src = json!({ "version": "2.21", "children": children }).to_string();
    let loaded = load_canonical_with_assets(&src, &no_assets).expect("pen.dev document loads");
    let report = loaded.compatibility.pen_dev.clone().expect("converted");
    (loaded.loaded.value, report)
}

#[test]
fn root_overrides_move_under_the_component_id() {
    let mut doc = json!({ "version": "2.21", "children": [badge(), {
        "type": "ref", "id": "copy", "ref": "badge", "x": 10, "y": 20,
        "theme": { "mode": "dark" }, "enabled": false,
        "width": "fill_container", "height": 32, "padding": [4, 8],
        "justifyContent": "center", "stroke": "#FF0000",
        "descendants": { "dot": { "fill": "#000000" } }
    }]});
    let report = convert(&mut doc, &no_assets);
    let copy = doc["children"][1].as_object().unwrap();
    for key in ["width", "height", "padding", "justifyContent", "stroke"] {
        assert!(!copy.contains_key(key), "{key} left the instance");
    }
    assert_eq!(copy["x"], 10);
    assert_eq!(copy["theme"], json!({ "mode": "dark" }));
    assert_eq!(copy["visible"], false);
    let root = &copy["descendants"]["badge"];
    assert_eq!(root["width"], "fill_container");
    assert_eq!(root["height"], 32);
    assert_eq!(root["padding"], json!([4, 8]));
    assert_eq!(root["justifyContent"], "center");
    // Paint-only: the component's width completes the canonical stroke.
    assert_eq!(root["stroke"]["thickness"], 1.5);
    assert_eq!(root["stroke"]["fill"][0]["color"], "#FF0000");
    assert_eq!(copy["descendants"]["dot"]["fill"][0]["color"], "#000000");
    assert!(
        !report.unsupported.contains_key("partial stroke override"),
        "{}",
        report.summary()
    );
}

#[test]
fn a_resized_restyled_instance_renders_as_authored() {
    let (doc, report) = load(json!([badge(), {
        "type": "ref", "id": "copy", "ref": "badge", "x": 200, "y": 0,
        "width": 180, "height": 48, "padding": 12, "stroke": "#FF0000",
        "descendants": { "dot": { "strokeWidth": { "bottom": 4 } } }
    }]));
    assert!(!report.has_losses(), "{}", report.summary());
    let roots = ref_resolve::resolve_refs_for_canvas_roots(&doc.children, &doc);
    let PenNode::Frame(instance) = &roots[1] else {
        panic!("ref expands to the component's frame");
    };
    assert_eq!(instance.base.x, Some(200.0));
    assert_eq!(
        instance.container.width,
        Some(SizingBehavior::Number(180.0))
    );
    assert_eq!(
        instance.container.height,
        Some(SizingBehavior::Number(48.0))
    );
    let value = serde_json::to_value(instance).unwrap();
    assert_eq!(value["padding"], 12.0);
    assert_eq!(value["stroke"]["thickness"], 1.5);
    assert_eq!(value["stroke"]["fill"][0]["color"], "#FF0000");
    // Width-only on a child: the child's own paint is kept.
    let dot = &value["children"][0];
    assert_eq!(dot["stroke"]["thickness"]["bottom"], 4.0);
    assert_eq!(dot["stroke"]["fill"][0]["color"], "#00AA00");
}

#[test]
fn an_instance_without_root_overrides_is_left_alone() {
    let mut doc = json!({ "version": "2.21", "children": [badge(), {
        "type": "ref", "id": "copy", "ref": "badge", "x": 1, "y": 2
    }]});
    convert(&mut doc, &no_assets);
    assert!(doc["children"][1].get("descendants").is_none());
}

#[test]
fn a_patch_with_nothing_to_complete_it_is_still_reported() {
    let (_, report) = load(json!([{
        "type": "ref", "id": "copy", "ref": "elsewhere", "strokeWidth": 3
    }]));
    assert_eq!(report.unsupported.get("partial stroke override"), Some(&1));
}

#[test]
fn a_nested_instance_patch_completes_from_its_component() {
    let mut doc = json!({ "version": "2.21", "children": [badge(), {
        "type": "frame", "id": "card", "reusable": true, "children": [
            { "type": "ref", "id": "inner", "ref": "badge" }
        ]
    }, {
        "type": "ref", "id": "copy", "ref": "card",
        "descendants": { "inner": { "stroke": "#FF0000" } }
    }]});
    let report = convert(&mut doc, &no_assets);
    let patch = &doc["children"][2]["descendants"]["inner"];
    assert_eq!(patch["stroke"]["thickness"], 1.5);
    assert!(!report.unsupported.contains_key("partial stroke override"));
}
