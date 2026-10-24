//! Node-walk tests for the pen.dev conversion that sit beside `tests.rs`:
//! reusable components whose pen.dev root is not a frame.

use de_editor_core::{ref_resolve, ComponentLibrary, NodeId};
use jian_ops_schema::node::container::LayoutMode;
use jian_ops_schema::node::PenNode;
use jian_ops_schema::sizing::SizingBehavior;
use jian_ops_schema::PenDocument;
use serde_json::{json, Value};

use crate::payload::load_canonical_with_assets;
use crate::pen_dev::{no_assets, PenDevReport};

fn leaf() -> Value {
    json!({
        "type": "rectangle", "id": "leaf", "name": "Leaf/First", "reusable": true,
        "x": 40, "y": 12, "width": 18, "height": 18, "opacity": 0.8,
        "fill": "#3A7D44", "cornerRadius": [18, 0, 18, 0],
        "stroke": "#1F4D2A", "strokeWidth": 1.5, "strokeAlignment": "inner",
        "effect": { "type": "shadow", "shadowType": "outer", "color": "#00000040",
                    "offset": { "x": 0, "y": 2 }, "blur": 4 }
    })
}

fn load(children: Value) -> (PenDocument, PenDevReport) {
    let src = json!({ "version": "2.21", "children": children }).to_string();
    let loaded = load_canonical_with_assets(&src, &no_assets).expect("pen.dev document loads");
    let report = loaded
        .compatibility
        .pen_dev
        .clone()
        .expect("converted as pen.dev");
    (loaded.loaded.value, report)
}

#[test]
fn reusable_rectangle_becomes_a_reusable_frame_with_the_same_look() {
    let (as_frame, report) = load(json!([leaf()]));
    let mut plain = leaf();
    plain.as_object_mut().unwrap().remove("reusable");
    let (as_rect, _) = load(json!([plain]));
    assert!(!report.has_losses(), "{}", report.summary());

    let PenNode::Frame(frame) = &as_frame.children[0] else {
        panic!("reusable rectangle imports as a frame");
    };
    let PenNode::Rectangle(rect) = &as_rect.children[0] else {
        panic!("plain rectangle stays a rectangle");
    };
    assert_eq!(frame.reusable, Some(true));
    assert!(frame.children.is_none());
    assert_eq!(frame.base, rect.base, "id, name, position, opacity");
    // Free-form, fixed size: the frame defaults add no hug layout.
    assert_eq!(frame.container.layout, Some(LayoutMode::None));
    assert_eq!(frame.container.width, Some(SizingBehavior::Number(18.0)));
    assert_eq!(frame.container.height, Some(SizingBehavior::Number(18.0)));
    let mut container = frame.container.clone();
    container.layout = rect.container.layout.clone();
    assert_eq!(
        container, rect.container,
        "size, fill, stroke, radius, effects"
    );
}

#[test]
fn reusable_rectangle_is_a_component_and_its_refs_resolve() {
    let (doc, _) = load(json!([
        leaf(),
        // The canonical schema keeps an instance's root overrides under
        // `descendants[<component id>]`.
        { "type": "ref", "id": "copy", "ref": "leaf", "x": 100, "y": 0,
          "descendants": { "leaf": { "fill": "#FF0000" } } }
    ]));
    let components = ComponentLibrary::from_document(&doc);
    let component = components
        .find_by_id(&NodeId::new("leaf"))
        .expect("listed as a component");
    assert_eq!(component.name, "Leaf/First");

    let roots = ref_resolve::resolve_refs_for_canvas_roots(&doc.children, &doc);
    let PenNode::Frame(instance) = &roots[1] else {
        panic!("ref expands to the component's frame");
    };
    assert_eq!(instance.base.id, "copy");
    assert_eq!(instance.base.x, Some(100.0));
    assert_eq!(instance.reusable, None, "an instance is not a component");
    assert_eq!(instance.container.width, Some(SizingBehavior::Number(18.0)));
    let fill = serde_json::to_value(&instance.container.fill).unwrap();
    assert_eq!(fill[0]["color"], "#FF0000", "root override applies: {fill}");
}

#[test]
fn other_reusable_kinds_are_reported() {
    let (doc, report) = load(json!([
        { "type": "ellipse", "id": "dot", "reusable": true, "width": 8, "height": 8, "fill": "#000000" },
        { "type": "frame", "id": "card", "reusable": true, "width": 10, "height": 10 }
    ]));
    assert!(matches!(doc.children[0], PenNode::Ellipse(_)));
    assert_eq!(
        report.unsupported.get("non-frame reusable component"),
        Some(&1)
    );
    let components = ComponentLibrary::from_document(&doc);
    assert!(components.find_by_id(&NodeId::new("card")).is_some());
    assert!(components.find_by_id(&NodeId::new("dot")).is_none());
}

fn child_ids(node: &Value) -> Vec<&str> {
    node["children"]
        .as_array()
        .unwrap()
        .iter()
        .map(|child| child["id"].as_str().unwrap())
        .collect()
}

#[test]
fn children_take_the_canonical_first_on_top_paint_order() {
    let mut doc = json!({ "version": "2.21", "children": [
        { "type": "frame", "id": "free", "layout": "none", "children": [
            { "type": "rectangle", "id": "back" },
            { "type": "text", "id": "middle", "content": "a" },
            { "type": "rectangle", "id": "front" }
        ]},
        { "type": "frame", "id": "flow", "layout": "vertical", "children": [
            { "type": "rectangle", "id": "shade", "layoutPosition": "absolute" },
            { "type": "text", "id": "first", "content": "a" },
            { "type": "text", "id": "second", "content": "b" },
            { "type": "frame", "id": "badge", "layoutPosition": "absolute" }
        ]}
    ]});
    crate::pen_dev::convert(&mut doc, &no_assets);
    assert_eq!(child_ids(&doc["children"][0]), ["front", "middle", "back"]);
    // Flow order stays; the shade pen.dev paints under the text moves
    // behind it, the badge it paints over stays in front.
    assert_eq!(
        child_ids(&doc["children"][1]),
        ["badge", "first", "second", "shade"]
    );
}

#[test]
fn layout_include_stroke_pads_the_frame_by_its_stroke() {
    let frame = |id: &str, extra: Value| {
        let mut node = json!({ "type": "frame", "id": id, "layout": "vertical",
                               "stroke": "#FF0000", "strokeWidth": 8, "layoutIncludeStroke": true });
        node.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        node
    };
    let mut doc = json!({ "version": "2.21", "children": [
        frame("uniform", json!({ "padding": 10 })),
        frame("pair", json!({ "padding": [4, 6] })),
        frame("sided", json!({ "padding": 2, "strokeWidth": { "top": 12, "left": 4 } })),
        frame("bound", json!({ "padding": "$pad" })),
        frame("free", json!({ "layout": "none", "padding": 10 })),
        frame("off", json!({ "padding": 10, "layoutIncludeStroke": false }))
    ]});
    let report = crate::pen_dev::convert(&mut doc, &no_assets);
    let padding = |index: usize| doc["children"][index]["padding"].clone();
    assert_eq!(padding(0), json!([18.0, 18.0, 18.0, 18.0]));
    assert_eq!(padding(1), json!([12.0, 14.0, 12.0, 14.0]));
    assert_eq!(padding(2), json!([14.0, 2.0, 2.0, 6.0]));
    assert_eq!(padding(3), json!("$pad"));
    assert_eq!(padding(4), json!(10));
    assert_eq!(padding(5), json!(10));
    for index in 0..6 {
        assert!(doc["children"][index].get("layoutIncludeStroke").is_none());
    }
    assert_eq!(
        report
            .unsupported
            .get("layoutIncludeStroke (variable padding)"),
        Some(&1)
    );
}
