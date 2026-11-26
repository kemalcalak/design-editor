//! Picking variants (#368): exact matches, the deterministic fallback,
//! `ref` naming the set on the canvas and on load.

use jian_ops_schema::component_set::VariantValues;
use jian_ops_schema::node::PenNode;
use jian_ops_schema::PenDocument;
use serde_json::Value;

use super::*;
use crate::pen_node_ext::PenNodeExt;
use crate::ref_resolve::resolve_refs_for_canvas;
use crate::EditorState;

/// Three pages: instances on `Screen` and `Other`, three Button masters
/// (no ghost / sm) on `Components`, not yet in a set.
pub(super) const MASTERS: &str = r##"{
  "version": "1.0.0",
  "children": [],
  "pages": [
    {"id": "p-screen", "name": "Screen", "children": [
      {"type": "frame", "id": "screen", "width": 400, "height": 300, "children": [
        {"type": "ref", "id": "i1", "ref": "btn-pm", "props": {"label": "Save"},
         "descendants": {"btn-pm": {"opacity": 0.5}, "btn-pm-label": {"fontSize": 20},
                         "btn-pm-dot": {"opacity": 0.2}}}
      ]}
    ]},
    {"id": "p-other", "name": "Other", "children": [
      {"type": "ref", "id": "i2", "ref": "btn-ps"}
    ]},
    {"id": "p-kit", "name": "Components", "children": [
      {"type": "frame", "id": "btn-pm", "name": "variant=primary, size=md", "reusable": true,
       "width": 120, "height": 40, "fill": [{"type": "solid", "color": "#111111"}],
       "componentProperties": {"label": {"type": "text", "default": "Button",
          "bind": [{"node": "btn-pm-label", "field": "content"}]}},
       "children": [
         {"type": "text", "id": "btn-pm-label", "name": "Label", "content": "Button"},
         {"type": "rectangle", "id": "btn-pm-dot", "name": "Dot", "width": 4, "height": 4}]},
      {"type": "frame", "id": "btn-ps", "name": "variant=primary, size=sm", "reusable": true,
       "width": 90, "height": 32, "fill": [{"type": "solid", "color": "#111111"}],
       "componentProperties": {"label": {"type": "text", "default": "Button",
          "bind": [{"node": "btn-ps-label", "field": "content"}]}},
       "children": [{"type": "text", "id": "btn-ps-label", "name": "Label", "content": "Button"}]},
      {"type": "frame", "id": "btn-gm", "name": "variant=ghost, size=md", "reusable": true,
       "width": 120, "height": 40,
       "componentProperties": {"label": {"type": "text", "default": "Button",
          "bind": [{"node": "btn-gm-label", "field": "content"}]}},
       "children": [{"type": "text", "id": "btn-gm-label", "name": "Label", "content": "Button"}]}
    ]}
  ]
}"##;

pub(super) fn doc(source: &str) -> PenDocument {
    jian_ops_schema::load_str(source)
        .expect("fixture parses")
        .value
}

pub(super) fn values(pairs: &[(&str, &str)]) -> VariantValues {
    pairs
        .iter()
        .map(|(axis, value)| (axis.to_string(), value.to_string()))
        .collect()
}

/// The fixture with its three masters combined into the set `button`.
pub(super) fn with_set() -> EditorState {
    let mut state = EditorState::from_document(doc(MASTERS));
    let command = ComponentSetCommand::Create(CreateSet {
        name: "Button".into(),
        master_ids: vec!["btn-pm".into(), "btn-ps".into(), "btn-gm".into()],
        set_id: Some("button".into()),
        ..Default::default()
    });
    state.apply_component_set(&command).expect("set created");
    state
}

pub(super) fn node_json(state: &EditorState, id: &str) -> Value {
    serde_json::to_value(super::tree::find(&state.doc, id).expect("node exists")).unwrap()
}

#[test]
fn an_exact_combination_picks_its_master() {
    let state = with_set();
    let set = super::tree::find(&state.doc, "button").unwrap();
    let pick = pick_variant(set, &values(&[("variant", "primary"), ("size", "sm")])).unwrap();
    assert_eq!(pick.master.id_str(), "btn-ps");
    assert!(pick.exact);
}

#[test]
fn a_missing_combination_falls_back_by_axis_order() {
    let state = with_set();
    let set = super::tree::find(&state.doc, "button").unwrap();
    // primary/sm and ghost/md both match one axis; `variant` comes first,
    // so ghost/md wins, every time.
    for _ in 0..3 {
        let pick = pick_variant(set, &values(&[("variant", "ghost"), ("size", "sm")])).unwrap();
        assert_eq!(pick.master.id_str(), "btn-gm");
        assert!(!pick.exact);
    }
}

#[test]
fn an_axis_a_master_leaves_out_matches_any_value() {
    let set: PenNode = serde_json::from_str(
        r#"{"type":"frame","id":"s","componentSet":{"axes":{"tone":["a","b"],"size":["sm","lg"]}},
            "children":[
              {"type":"frame","id":"any-size","reusable":true,"variantValues":{"tone":"b"}},
              {"type":"frame","id":"a-sm","reusable":true,"variantValues":{"tone":"a","size":"sm"}}]}"#,
    )
    .unwrap();
    let pick = pick_variant(&set, &values(&[("tone", "b"), ("size", "lg")])).unwrap();
    assert_eq!(pick.master.id_str(), "any-size");
    assert!(pick.exact);
}

#[test]
fn a_ref_naming_the_set_renders_the_picked_variant() {
    let mut state = with_set();
    let PenNode::Ref(reference) = super::tree::find_mut(&mut state.doc, "i2").unwrap() else {
        panic!("ref");
    };
    reference.target = "button".into();
    reference.variant = Some(values(&[("variant", "ghost")]));
    let resolved = resolve_refs_for_canvas(&state.doc);
    let page = &resolved.pages.as_ref().unwrap()[1];
    let PenNode::Frame(frame) = &page.children[0] else {
        panic!("instance expands to a frame, not the whole set");
    };
    let label = &frame.children.as_ref().unwrap()[0];
    assert_eq!(
        label.id_str(),
        "i2__btn-gm-label",
        "ghost/md (size default)"
    );
    assert!(frame.component_set.is_none() && frame.variant_values.is_none());
}

#[test]
fn opening_a_document_points_set_refs_at_their_master() {
    let state = with_set();
    let mut json = serde_json::to_value(&state.doc).unwrap();
    let other = &mut json["pages"][1]["children"][0];
    other["ref"] = "button".into();
    other["variant"] = serde_json::json!({"size": "sm", "variant": "ghost"});
    let opened = EditorState::from_opened_document(serde_json::from_value(json).unwrap());
    let instance = node_json(&opened, "i2");
    assert_eq!(instance["ref"], "btn-gm", "nearest to ghost/sm");
    assert_eq!(instance["variant"]["size"], "sm", "the request is kept");
}

#[test]
fn inserted_instances_are_normalized_and_bad_values_refused() {
    let state = with_set();
    let mut nodes: Vec<PenNode> = vec![serde_json::from_str(
        r#"{"type":"ref","id":"new","ref":"button","variant":{"size":"sm"}}"#,
    )
    .unwrap()];
    let fallbacks = normalize_inserted(&state.doc, &mut nodes).unwrap();
    assert!(fallbacks.is_empty());
    let PenNode::Ref(reference) = &nodes[0] else {
        panic!("ref");
    };
    assert_eq!(reference.target, "btn-ps");
    assert_eq!(
        reference.variant.as_ref().unwrap(),
        &values(&[("variant", "primary"), ("size", "sm")])
    );

    let mut bad: Vec<PenNode> = vec![serde_json::from_str(
        r#"{"type":"ref","id":"bad","ref":"button","variant":{"size":"xl"}}"#,
    )
    .unwrap()];
    let error = normalize_inserted(&state.doc, &mut bad).unwrap_err();
    assert_eq!(
        error.to_string(),
        "unknown value xl for axis size; values: md, sm"
    );
}

#[test]
fn variant_names_parse_and_print_in_axis_order() {
    assert_eq!(
        parse_variant_name("variant=primary, size=md"),
        Some(vec![
            ("variant".into(), "primary".into()),
            ("size".into(), "md".into())
        ])
    );
    assert_eq!(parse_variant_name("Primary Button"), None);
    assert_eq!(parse_variant_name("a=1, a=2"), None);
    let state = with_set();
    let set = component_set_of(super::tree::find(&state.doc, "button").unwrap()).unwrap();
    assert_eq!(
        variant_name(set, &values(&[("size", "sm"), ("variant", "ghost")])),
        "variant=ghost, size=sm"
    );
}
