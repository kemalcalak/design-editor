//! Component properties resolve through `descendants` (#367): every type,
//! explicit overrides winning, nested exposure, detach, the inspector
//! merge and a save / load round trip, with the masters on another page.

use jian_ops_schema::node::text::TextContent;
use jian_ops_schema::node::PenNode;
use jian_ops_schema::PenDocument;
use serde_json::{json, Value};

use crate::pen_node_ext::PenNodeExt;
use crate::ref_resolve::resolve_refs_for_canvas;
use crate::{EditorState, NodeId};

/// Three pages: instances on `Screen` and `Notes`, masters on
/// `Components`. `btn` declares one property of each type plus `iconTint`,
/// which exposes the nested icon's `tint`.
pub(super) const DOC: &str = r##"{
  "version": "1.0.0",
  "variables": {"primary": {"type": "color", "value": "#3355ff"}},
  "children": [],
  "pages": [
    {"id": "p-screen", "name": "Screen", "children": [
      {"type": "frame", "id": "screen", "width": 400, "height": 300, "children": [
        {"type": "ref", "id": "b1", "ref": "btn", "props": {"label": "Save", "showIcon": false}},
        {"type": "ref", "id": "b2", "ref": "btn", "props": {"label": "Go", "icon": "icon-plus"},
         "descendants": {"btn-label": {"content": "Explicit"}}},
        {"type": "ref", "id": "b3", "ref": "btn", "props": {"iconTint": "#ff0000"}},
        {"type": "ref", "id": "b4", "ref": "btn", "props": {"tone": "$primary", "label": 5}}
      ]}
    ]},
    {"id": "p-notes", "name": "Notes", "children": [
      {"type": "ref", "id": "n1", "ref": "btn", "props": {"label": "Notes"}}
    ]},
    {"id": "p-kit", "name": "Components", "children": [
      {"type": "frame", "id": "icon-arrow", "reusable": true, "width": 16, "height": 16,
       "componentProperties": {
         "tint": {"type": "color", "default": "#000000",
                  "bind": [{"node": "arrow-shape", "field": "fill"}]}},
       "children": [
         {"type": "rectangle", "id": "arrow-shape", "width": 16, "height": 16,
          "fill": [{"type": "solid", "color": "#000000"}]}]},
      {"type": "frame", "id": "icon-plus", "reusable": true, "width": 16, "height": 16,
       "children": [{"type": "rectangle", "id": "plus-shape", "width": 16, "height": 16}]},
      {"type": "frame", "id": "btn", "reusable": true, "width": 120, "height": 40,
       "componentProperties": {
         "label": {"type": "text", "default": "Button",
                   "bind": [{"node": "btn-label", "field": "content"}]},
         "showIcon": {"type": "boolean", "default": true,
                      "bind": [{"node": "btn-icon", "field": "visible"}]},
         "icon": {"type": "instance", "default": "icon-arrow",
                  "preferred": ["icon-arrow", "icon-plus"],
                  "bind": [{"node": "btn-icon", "field": "ref"}]},
         "tone": {"type": "color", "default": "#111111",
                  "bind": [{"node": "btn-bg", "field": "fill"}]},
         "iconTint": {"type": "color", "default": "#000000",
                      "bind": [{"node": "btn-icon", "field": "props.tint"}]}
       },
       "children": [
         {"type": "rectangle", "id": "btn-bg", "width": 120, "height": 40,
          "fill": [{"type": "solid", "color": "#111111"}]},
         {"type": "text", "id": "btn-label", "content": "Button"},
         {"type": "ref", "id": "btn-icon", "ref": "icon-arrow"}
       ]}
    ]}
  ]
}"##;

pub(super) fn doc() -> PenDocument {
    jian_ops_schema::load_str(DOC).expect("fixture loads").value
}

pub(super) fn state() -> EditorState {
    EditorState::from_document(doc())
}

/// The resolved node `id` (a virtual `inst__child` id) on any page.
fn resolved<'a>(doc: &'a PenDocument, id: &str) -> &'a PenNode {
    fn walk<'a>(nodes: &'a [PenNode], id: &str) -> Option<&'a PenNode> {
        nodes.iter().find_map(|node| {
            if node.id_str() == id {
                return Some(node);
            }
            node.children().and_then(|children| walk(children, id))
        })
    }
    doc.pages
        .iter()
        .flatten()
        .find_map(|page| walk(&page.children, id))
        .unwrap_or_else(|| panic!("{id} resolves"))
}

fn content(node: &PenNode) -> String {
    match node {
        PenNode::Text(text) => match &text.content {
            TextContent::Plain(plain) => plain.clone(),
            other => format!("{other:?}"),
        },
        other => panic!("not text: {}", other.id_str()),
    }
}

fn fill_color(node: &PenNode) -> Value {
    let value = serde_json::to_value(node).unwrap();
    value["fill"][0]["color"].clone()
}

#[test]
fn each_type_renders_through_descendants() {
    let resolved_doc = resolve_refs_for_canvas(&doc());
    assert_eq!(content(resolved(&resolved_doc, "b1__btn-label")), "Save");
    assert_eq!(
        resolved(&resolved_doc, "b1__btn-icon").base().visible,
        Some(false),
        "boolean property hides the icon"
    );
    // Instance swap: the nested icon now expands `icon-plus`.
    let icon = resolved(&resolved_doc, "b2__btn-icon");
    let child_ids: Vec<&str> = icon
        .children()
        .unwrap()
        .iter()
        .map(|c| c.id_str())
        .collect();
    assert_eq!(child_ids, ["b2__btn-icon__plus-shape"]);
    // Colour, through a variable reference resolved later in the scene.
    assert_eq!(
        fill_color(resolved(&resolved_doc, "b4__btn-bg")),
        json!("$primary")
    );
    // Unset properties show the master.
    assert_eq!(content(resolved(&resolved_doc, "b3__btn-label")), "Button");
    assert_eq!(
        fill_color(resolved(&resolved_doc, "b1__btn-bg")),
        json!("#111111")
    );
}

#[test]
fn explicit_descendants_win_over_props() {
    let resolved_doc = resolve_refs_for_canvas(&doc());
    assert_eq!(
        content(resolved(&resolved_doc, "b2__btn-label")),
        "Explicit",
        "a descendants entry for the same field wins"
    );
}

#[test]
fn wrong_typed_values_are_ignored_when_rendering() {
    let resolved_doc = resolve_refs_for_canvas(&doc());
    // b4 sets `label: 5`: the master's text stays.
    assert_eq!(content(resolved(&resolved_doc, "b4__btn-label")), "Button");
}

#[test]
fn nested_instance_property_is_exposed_on_the_outer_component() {
    let resolved_doc = resolve_refs_for_canvas(&doc());
    assert_eq!(
        fill_color(resolved(&resolved_doc, "b3__btn-icon__arrow-shape")),
        json!("#ff0000"),
        "iconTint reaches the nested icon's own tint property"
    );
    assert_eq!(
        fill_color(resolved(&resolved_doc, "b1__btn-icon__arrow-shape")),
        json!("#000000")
    );
}

#[test]
fn instance_on_another_page_than_its_master_resolves() {
    let resolved_doc = resolve_refs_for_canvas(&doc());
    assert_eq!(content(resolved(&resolved_doc, "n1__btn-label")), "Notes");
}

#[test]
fn detach_keeps_what_the_props_showed() {
    let mut s = state();
    assert!(s.detach_component(&NodeId::new("b1")).is_some());
    let PenNode::Frame(detached) =
        crate::walkers::find_node(s.active_children(), &NodeId::new("b1"))
            .expect("detached root keeps the id")
    else {
        panic!("detached into a frame");
    };
    let label = detached
        .children
        .iter()
        .flatten()
        .find(|child| matches!(child, PenNode::Text(_)))
        .expect("label");
    assert_eq!(content(label), "Save");
    assert!(
        detached.component_properties.is_none(),
        "a copy declares no properties"
    );
}

#[test]
fn inspector_display_node_shows_prop_values() {
    let s = state();
    let instance = crate::walkers::find_node(s.active_children(), &NodeId::new("b1")).unwrap();
    let display =
        crate::instance_override::resolve_instance_display_node(&s.doc, instance).expect("display");
    let label = display
        .children()
        .unwrap()
        .iter()
        .find(|child| child.id_str() == "btn-label")
        .unwrap();
    assert_eq!(content(label), "Save");
}

#[test]
fn a_variable_valued_property_follows_the_theme_at_the_instance() {
    let mut raw: Value = serde_json::from_str(DOC).unwrap();
    raw["themes"] = json!({"Mode": ["Light", "Dark"]});
    raw["variables"]["primary"] = json!({"type": "color", "value": [
        {"value": "#3355ff", "theme": {"Mode": "Light"}},
        {"value": "#99aaff", "theme": {"Mode": "Dark"}}]});
    // b3 shows the same `$primary` value in its own Dark scope.
    raw["pages"][0]["children"][0]["children"][2]["props"] = json!({"tone": "$primary"});
    raw["pages"][0]["children"][0]["children"][2]["theme"] = json!({"Mode": "Dark"});
    let doc = jian_ops_schema::load_str(&raw.to_string()).unwrap().value;
    let light = std::collections::BTreeMap::from([("Mode".to_string(), "Light".to_string())]);
    let resolved = crate::variables_resolve::resolve_document_for_canvas(
        &resolve_refs_for_canvas(&doc),
        &light,
    );
    assert_eq!(
        fill_color(resolved(&resolved, "b4__btn-bg")),
        json!("#3355ff")
    );
    assert_eq!(
        fill_color(resolved(&resolved, "b3__btn-bg")),
        json!("#99aaff"),
        "the instance's own theme scope picks the Dark value"
    );
    let dark = std::collections::BTreeMap::from([("Mode".to_string(), "Dark".to_string())]);
    let switched = crate::variables_resolve::resolve_document_for_canvas(
        &resolve_refs_for_canvas(&doc),
        &dark,
    );
    assert_eq!(
        fill_color(resolved(&switched, "b4__btn-bg")),
        json!("#99aaff")
    );
}

#[test]
fn save_and_load_keep_properties_and_props() {
    let original = doc();
    let saved = serde_json::to_string(&original).unwrap();
    let loaded = jian_ops_schema::load_str(&saved).unwrap().value;
    assert_eq!(loaded, original);
    let raw: Value = serde_json::from_str(&saved).unwrap();
    let btn = &raw["pages"][2]["children"][2];
    assert_eq!(
        btn["componentProperties"]["icon"]["preferred"],
        json!(["icon-arrow", "icon-plus"])
    );
    assert_eq!(
        btn["componentProperties"]["iconTint"]["bind"][0]["field"],
        json!("props.tint")
    );
    assert_eq!(
        raw["pages"][0]["children"][0]["children"][0]["props"]["label"],
        "Save"
    );
}
