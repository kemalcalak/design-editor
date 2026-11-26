//! Component-set writes that must stay inside their set and refuse cleanly
//! (#368 review): a new axis reaches only its own set's instances, a bad
//! switch is refused before anything changes, ids are never reused, and
//! masters that cannot move are refused.

use jian_ops_schema::node::PenNode;
use serde_json::json;

use super::tests::{doc, node_json, values, with_set, MASTERS};
use super::*;
use crate::EditorState;

/// The Button set plus a second set `badge` with its own instance `b1`.
fn two_sets() -> EditorState {
    let mut state = with_set();
    let mut json = serde_json::to_value(&state.doc).unwrap();
    json["pages"][2]["children"].as_array_mut().unwrap().push(
        json!({"type": "frame", "id": "badge", "name": "Badge",
            "componentSet": {"axes": {"tone": ["info", "warn"]}},
            "children": [
              {"type": "frame", "id": "bi", "reusable": true, "variantValues": {"tone": "info"}},
              {"type": "frame", "id": "bw", "reusable": true, "variantValues": {"tone": "warn"}}]}),
    );
    json["pages"][1]["children"]
        .as_array_mut()
        .unwrap()
        .push(json!({"type": "ref", "id": "b1", "ref": "bw", "variant": {"tone": "warn"}}));
    state = EditorState::from_document(serde_json::from_value(json).unwrap());
    state
}

#[test]
fn a_new_axis_reaches_only_its_own_sets_instances() {
    let mut state = two_sets();
    state
        .apply_component_set(&ComponentSetCommand::SetInstanceVariant {
            node_id: "i2".into(),
            variant: values(&[("size", "sm")]),
        })
        .unwrap();
    state
        .apply_component_set(&ComponentSetCommand::AddAxis {
            set_id: "button".into(),
            axis: "state".into(),
            value: "default".into(),
        })
        .unwrap();
    assert_eq!(node_json(&state, "i2")["variant"]["state"], "default");
    assert_eq!(
        node_json(&state, "b1")["variant"],
        json!({"tone": "warn"}),
        "the other set's instance is untouched"
    );
}

#[test]
fn a_bad_switch_is_refused_by_check_and_apply_alike() {
    let mut state = with_set();
    let command = ComponentSetCommand::SetInstanceVariant {
        node_id: "i1".into(),
        variant: values(&[("size", "xl")]),
    };
    let expected = "unknown value xl for axis size; values: md, sm";
    assert_eq!(command.check(&state.doc).unwrap_err().to_string(), expected);
    let depth = state.history.past.len();
    assert_eq!(
        state.apply_component_set(&command).unwrap_err().to_string(),
        expected
    );
    assert_eq!(state.history.past.len(), depth, "no empty undo step");
}

#[test]
fn a_stored_value_the_set_dropped_never_blocks_a_switch() {
    let mut state = with_set();
    if let Some(PenNode::Ref(reference)) = super::tree::find_mut(&mut state.doc, "i1") {
        reference.variant = Some(values(&[("size", "md"), ("variant", "retired")]));
    }
    let command = ComponentSetCommand::SetInstanceVariant {
        node_id: "i1".into(),
        variant: values(&[("size", "sm")]),
    };
    assert!(command.check(&state.doc).is_ok());
    state.apply_component_set(&command).unwrap();
    assert_eq!(node_json(&state, "i1")["ref"], "btn-ps");
}

#[test]
fn a_set_id_already_in_use_is_refused() {
    let mut state = EditorState::from_document(doc(MASTERS));
    let error = state
        .apply_component_set(&ComponentSetCommand::Create(CreateSet {
            name: "Button".into(),
            master_ids: vec!["btn-pm".into(), "btn-ps".into()],
            set_id: Some("screen".into()),
            ..Default::default()
        }))
        .unwrap_err();
    assert_eq!(error, VariantError::IdTaken("screen".into()));
}

#[test]
fn a_master_inside_another_component_is_refused() {
    let mut json = serde_json::to_value(doc(MASTERS)).unwrap();
    let kit = json["pages"][2]["children"].as_array_mut().unwrap();
    let inner = kit.remove(2);
    kit[0]["children"].as_array_mut().unwrap().push(inner);
    let mut state = EditorState::from_document(serde_json::from_value(json).unwrap());
    let error = state
        .apply_component_set(&ComponentSetCommand::Create(CreateSet {
            name: "Button".into(),
            master_ids: vec!["btn-gm".into(), "btn-ps".into()],
            ..Default::default()
        }))
        .unwrap_err();
    assert_eq!(error, VariantError::NestedMaster("btn-gm".into()));
    assert!(
        super::tree::find(&state.doc, "btn-gm").is_some(),
        "nothing lost"
    );
}

#[test]
fn masters_from_another_page_are_reported_as_moved() {
    let mut json = serde_json::to_value(doc(MASTERS)).unwrap();
    let ghost = json["pages"][2]["children"]
        .as_array_mut()
        .unwrap()
        .remove(2);
    json["pages"][1]["children"]
        .as_array_mut()
        .unwrap()
        .push(ghost);
    let mut state = EditorState::from_document(serde_json::from_value(json).unwrap());
    let report = state
        .apply_component_set(&ComponentSetCommand::Create(CreateSet {
            name: "Button".into(),
            master_ids: vec!["btn-pm".into(), "btn-gm".into()],
            ..Default::default()
        }))
        .unwrap();
    assert_eq!(report.moved_masters, ["btn-gm"]);
    assert!(report.warnings()[0].contains("btn-gm moved from its page"));
}
