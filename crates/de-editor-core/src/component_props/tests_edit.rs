//! Component-property writes (#367): declare, rename, remove, set values,
//! validation messages, unbound reports and undo.

use jian_ops_schema::component_props::{BindingField, ComponentPropertyType, PropertyBinding};
use jian_ops_schema::node::PenNode;
use serde_json::{json, Map, Value};

use super::tests::state;
use super::{
    property_summaries, ComponentPropertyCommand, PropertyUpsert, PropsContext, PropsError,
};
use crate::command::EditorCommand;
use crate::pen_node_ext::PenNodeExt;
use crate::ref_resolve::resolve_refs_for_canvas;
use crate::EditorState;

fn props(value: Value) -> Map<String, Value> {
    value.as_object().cloned().unwrap()
}

fn instance_props(s: &EditorState, id: &str) -> Option<Map<String, Value>> {
    match crate::component_backing::find_node_in_document(&s.doc, id) {
        Some(PenNode::Ref(reference)) => reference
            .props
            .as_ref()
            .map(|props| props.clone().into_iter().collect()),
        _ => panic!("{id} is an instance"),
    }
}

fn instance_json(s: &EditorState, id: &str) -> Value {
    serde_json::to_value(crate::component_backing::find_node_in_document(&s.doc, id).unwrap())
        .unwrap()
}

fn master_json(s: &EditorState) -> Value {
    instance_json(s, "btn")
}

fn apply(s: &mut EditorState, command: ComponentPropertyCommand) -> Result<(), PropsError> {
    s.apply_component_property(&command)
}

/// The two pages holding instances, as rendered (the masters' own page
/// carries the property declarations, which these edits change).
fn rendered(s: &EditorState) -> Value {
    let pages = resolve_refs_for_canvas(&s.doc).pages.unwrap();
    serde_json::to_value(&pages[..2]).unwrap()
}

#[test]
fn rename_rewrites_every_instance_in_one_undo_step() {
    let mut s = state();
    let before = rendered(&s);
    let depth = s.history.past.len();
    apply(
        &mut s,
        ComponentPropertyCommand::Upsert(PropertyUpsert {
            component_id: "btn".into(),
            name: "caption".into(),
            rename_from: Some("label".into()),
            ..Default::default()
        }),
    )
    .unwrap();
    assert_eq!(s.history.past.len(), depth + 1, "one undo step");
    assert_eq!(instance_props(&s, "b1").unwrap()["caption"], "Save");
    assert!(!instance_props(&s, "b1").unwrap().contains_key("label"));
    assert_eq!(
        instance_props(&s, "n1").unwrap()["caption"],
        "Notes",
        "an instance on another page follows"
    );
    assert!(master_json(&s)["componentProperties"]["caption"].is_object());
    assert!(master_json(&s)["componentProperties"]["label"].is_null());
    assert_eq!(rendered(&s), before, "the canvas does not change");
    assert!(s.apply(EditorCommand::Undo));
    assert_eq!(instance_props(&s, "b1").unwrap()["label"], "Save");
}

#[test]
fn renaming_a_nested_property_updates_the_outer_exposure() {
    let mut s = state();
    apply(
        &mut s,
        ComponentPropertyCommand::Upsert(PropertyUpsert {
            component_id: "icon-arrow".into(),
            name: "color".into(),
            rename_from: Some("tint".into()),
            ..Default::default()
        }),
    )
    .unwrap();
    assert_eq!(
        master_json(&s)["componentProperties"]["iconTint"]["bind"][0]["field"],
        "props.color"
    );
    let resolved = rendered(&s);
    let text = resolved.to_string();
    assert!(text.contains("#ff0000"), "b3's tint still reaches the icon");
}

#[test]
fn remove_keeps_the_visual_result_as_overrides() {
    let mut s = state();
    let before = rendered(&s);
    apply(
        &mut s,
        ComponentPropertyCommand::Remove {
            component_id: "btn".into(),
            name: "label".into(),
        },
    )
    .unwrap();
    let b1 = instance_json(&s, "b1");
    assert_eq!(b1["descendants"]["btn-label"]["content"], "Save");
    assert!(b1["props"].get("label").is_none());
    let b2 = instance_json(&s, "b2");
    assert_eq!(
        b2["descendants"]["btn-label"]["content"], "Explicit",
        "an explicit override is not overwritten"
    );
    assert!(master_json(&s)["componentProperties"]["label"].is_null());
    assert_eq!(rendered(&s), before, "every instance looks the same");
}

#[test]
fn set_instance_props_merges_and_clears() {
    let mut s = state();
    apply(
        &mut s,
        ComponentPropertyCommand::SetInstanceProps {
            node_id: "b1".into(),
            props: props(json!({"label": "Send", "showIcon": null, "icon": "icon-plus"})),
        },
    )
    .unwrap();
    assert_eq!(
        instance_props(&s, "b1").unwrap(),
        props(json!({"label": "Send", "icon": "icon-plus"}))
    );
    apply(
        &mut s,
        ComponentPropertyCommand::SetInstanceProps {
            node_id: "b1".into(),
            props: props(json!({"label": null, "icon": null})),
        },
    )
    .unwrap();
    assert_eq!(instance_props(&s, "b1"), None, "no values, no props key");
}

#[test]
fn validation_errors_name_the_allowed_properties_and_types() {
    let mut s = state();
    let set = |name: &str, value: Value| ComponentPropertyCommand::SetInstanceProps {
        node_id: "b1".into(),
        props: props(json!({ name: value })),
    };
    let message = |s: &mut EditorState, command| apply(s, command).unwrap_err().to_string();
    let unknown = message(&mut s, set("labl", json!("x")));
    assert!(unknown.contains("no property \"labl\""), "{unknown}");
    assert!(
        unknown.contains("label (text)") && unknown.contains("showIcon (boolean)"),
        "{unknown}"
    );
    let wrong = message(&mut s, set("showIcon", json!("no")));
    assert!(
        wrong.contains("takes a boolean value, got a string"),
        "{wrong}"
    );
    let component = message(&mut s, set("icon", json!("btn-bg")));
    assert!(component.contains("no component"), "{component}");
    let colour = message(&mut s, set("tone", json!("$missing")));
    assert!(colour.contains("$variable"), "{colour}");
    assert!(apply(&mut s, set("tone", json!("#abc"))).is_ok());
    let not_instance = apply(
        &mut s,
        ComponentPropertyCommand::SetInstanceProps {
            node_id: "btn-bg".into(),
            props: Map::new(),
        },
    );
    assert_eq!(
        not_instance,
        Err(PropsError::InstanceNotFound("btn-bg".into()))
    );
}

#[test]
fn new_property_takes_its_default_from_the_bound_node() {
    let mut s = state();
    apply(
        &mut s,
        ComponentPropertyCommand::Upsert(PropertyUpsert {
            component_id: "icon-plus".into(),
            name: "visible".into(),
            kind: Some(ComponentPropertyType::Boolean),
            bind: Some(vec![PropertyBinding {
                node: "plus-shape".into(),
                field: BindingField::Visible,
            }]),
            ..Default::default()
        }),
    )
    .unwrap();
    let master = instance_json(&s, "icon-plus");
    assert_eq!(master["componentProperties"]["visible"]["default"], true);
}

#[test]
fn an_explicit_default_is_written_into_the_master() {
    let mut s = state();
    apply(
        &mut s,
        ComponentPropertyCommand::Upsert(PropertyUpsert {
            component_id: "btn".into(),
            name: "label".into(),
            default: Some(json!("Click")),
            ..Default::default()
        }),
    )
    .unwrap();
    let btn = master_json(&s);
    assert_eq!(btn["children"][1]["content"], "Click");
    assert_eq!(btn["componentProperties"]["label"]["default"], "Click");
}

#[test]
fn bindings_are_checked_against_the_master() {
    let s = state();
    let context = PropsContext {
        doc: &s.doc,
        components: &s.components,
    };
    let upsert = |node: &str, field: BindingField, kind| PropertyUpsert {
        component_id: "btn".into(),
        name: "extra".into(),
        kind: Some(kind),
        bind: Some(vec![PropertyBinding {
            node: node.into(),
            field,
        }]),
        ..Default::default()
    };
    let outside = upsert(
        "plus-shape",
        BindingField::Visible,
        ComponentPropertyType::Boolean,
    )
    .plan(context)
    .unwrap_err();
    assert!(matches!(outside, PropsError::BindNodeNotFound { .. }));
    let mismatch = upsert("btn-bg", BindingField::Content, ComponentPropertyType::Text)
        .plan(context)
        .unwrap_err()
        .to_string();
    assert!(mismatch.contains("content binds a text node"), "{mismatch}");
    let wrong_type = upsert(
        "btn-label",
        BindingField::Content,
        ComponentPropertyType::Boolean,
    )
    .plan(context)
    .unwrap_err()
    .to_string();
    assert!(wrong_type.contains("carries a text value"), "{wrong_type}");
    let missing_type = PropertyUpsert {
        component_id: "btn".into(),
        name: "fresh".into(),
        ..Default::default()
    }
    .plan(context)
    .unwrap_err();
    assert_eq!(missing_type, PropsError::MissingType("fresh".into()));
    let nested = upsert(
        "btn-icon",
        BindingField::Prop("nope".into()),
        ComponentPropertyType::Color,
    )
    .plan(context)
    .unwrap_err()
    .to_string();
    assert!(nested.contains("no such property"), "{nested}");
}

#[test]
fn type_change_is_refused_while_instances_set_the_property() {
    let mut s = state();
    let error = apply(
        &mut s,
        ComponentPropertyCommand::Upsert(PropertyUpsert {
            component_id: "btn".into(),
            name: "label".into(),
            kind: Some(ComponentPropertyType::Boolean),
            bind: Some(vec![PropertyBinding {
                node: "btn-icon".into(),
                field: BindingField::Visible,
            }]),
            ..Default::default()
        }),
    )
    .unwrap_err();
    assert!(
        matches!(error, PropsError::TypeChangeInUse { instances: 4, .. }),
        "{error}"
    );
}

#[test]
fn a_property_whose_node_left_the_master_is_reported_unbound() {
    let mut s = state();
    let master = crate::component_props::edit::find_node_mut(&mut s.doc, "btn").unwrap();
    master
        .children_mut()
        .unwrap()
        .retain(|child| child.id_str() != "btn-label");
    let context = PropsContext {
        doc: &s.doc,
        components: &s.components,
    };
    let master = context.master("btn").unwrap();
    let summaries = property_summaries(context, master);
    let label = summaries.iter().find(|p| p.name == "label").unwrap();
    assert!(label.is_unbound());
    assert_eq!(label.to_json()["unbound"][0]["node"], "btn-label");
    assert_eq!(
        label.default,
        Some(json!("Button")),
        "falls back to the stored default"
    );
    let icon = summaries.iter().find(|p| p.name == "icon").unwrap();
    assert!(!icon.is_unbound());
    assert_eq!(icon.default, Some(json!("icon-arrow")));
    // Instances keep their values, and the property can still be renamed.
    assert_eq!(instance_props(&s, "b1").unwrap()["label"], "Save");
    apply(
        &mut s,
        ComponentPropertyCommand::Upsert(PropertyUpsert {
            component_id: "btn".into(),
            name: "caption".into(),
            rename_from: Some("label".into()),
            ..Default::default()
        }),
    )
    .unwrap();
    assert_eq!(instance_props(&s, "b1").unwrap()["caption"], "Save");
}

#[test]
fn editor_command_routes_and_refuses() {
    let mut s = state();
    let good =
        EditorCommand::ComponentProperty(Box::new(ComponentPropertyCommand::SetInstanceProps {
            node_id: "n1".into(),
            props: props(json!({"label": "Other page"})),
        }));
    assert!(s.apply(good), "instances on another page take props");
    let bad =
        EditorCommand::ComponentProperty(Box::new(ComponentPropertyCommand::SetInstanceProps {
            node_id: "n1".into(),
            props: props(json!({"nope": 1})),
        }));
    assert!(!s.apply(bad));
}
