//! Component-set writes (#368): create from masters, switch instances,
//! add / rename / remove, refusals, undo and a save / load round trip
//! across three pages.

use jian_ops_schema::node::PenNode;
use serde_json::json;

use super::tests::{doc, node_json, values, with_set, MASTERS};
use super::*;
use crate::command::EditorCommand;
use crate::pen_node_ext::PenNodeExt;
use crate::ref_resolve::resolve_refs_for_canvas;
use crate::{EditorState, NodeId};

fn switch(state: &mut EditorState, id: &str, pairs: &[(&str, &str)]) -> VariantReport {
    state
        .apply_component_set(&ComponentSetCommand::SetInstanceVariant {
            node_id: id.into(),
            variant: values(pairs),
        })
        .expect("switch applies")
}

#[test]
fn combining_masters_infers_axes_from_their_names() {
    let mut state = EditorState::from_document(doc(MASTERS));
    let depth = state.history.past.len();
    let report = state
        .apply_component_set(&ComponentSetCommand::Create(CreateSet {
            name: "Button".into(),
            master_ids: vec!["btn-pm".into(), "btn-ps".into(), "btn-gm".into()],
            ..Default::default()
        }))
        .unwrap();
    assert_eq!(report.set_id.as_deref(), Some("button"));
    assert_eq!(state.history.past.len(), depth + 1, "one undo step");
    let set = node_json(&state, "button");
    assert_eq!(
        set["componentSet"],
        json!({"axes": {"variant": ["primary", "ghost"], "size": ["md", "sm"]},
               "default": {"size": "md", "variant": "primary"}})
    );
    let axis_order: Vec<String> = set_summary(&state.doc, "button")
        .unwrap()
        .axes
        .into_iter()
        .map(|(axis, _)| axis)
        .collect();
    assert_eq!(axis_order, ["variant", "size"]);
    assert_eq!(
        node_json(&state, "btn-ps")["variantValues"],
        json!({"size": "sm", "variant": "primary"})
    );
    // The set took the first master's place on the Components page and
    // holds all three; the registry still knows each master.
    let kit = &state.doc.pages.as_ref().unwrap()[2];
    assert_eq!(kit.children.len(), 1);
    assert_eq!(kit.children[0].id_str(), "button");
    for id in ["btn-pm", "btn-ps", "btn-gm"] {
        assert!(state.components.find_by_id(&NodeId::new(id)).is_some());
    }
    // Laid out as a grid: variant across, size down.
    assert_eq!(node_json(&state, "btn-gm")["x"], node_json(&state, "btn-pm")["x"].as_f64().unwrap() + 120.0 + 24.0);
    assert!(node_json(&state, "btn-ps")["y"].as_f64() > node_json(&state, "btn-pm")["y"].as_f64());

    assert!(state.undo());
    assert_eq!(state.doc.pages.as_ref().unwrap()[2].children.len(), 3);
}

#[test]
fn masters_without_key_value_names_get_one_variant_axis() {
    let mut state = EditorState::from_document(doc(MASTERS));
    for (id, name) in [("btn-pm", "Primary"), ("btn-gm", "Ghost")] {
        if let Some(PenNode::Frame(frame)) = super::tree::find_mut(&mut state.doc, id) {
            frame.base.name = Some(name.into());
        }
    }
    state
        .apply_component_set(&ComponentSetCommand::Create(CreateSet {
            name: "Button".into(),
            master_ids: vec!["btn-pm".into(), "btn-gm".into()],
            ..Default::default()
        }))
        .unwrap();
    let summary = set_summary(&state.doc, "button").unwrap();
    assert_eq!(
        summary.axes,
        [("variant".to_string(), vec!["Primary".to_string(), "Ghost".to_string()])]
    );
    assert_eq!(summary.variants[1].name, "variant=Ghost");
}

#[test]
fn switching_keeps_props_and_matching_overrides_and_reports_the_rest() {
    let mut state = with_set();
    let depth = state.history.past.len();
    let report = switch(&mut state, "i1", &[("size", "sm")]);
    assert_eq!(state.history.past.len(), depth + 1);
    let instance = node_json(&state, "i1");
    assert_eq!(instance["ref"], "btn-ps");
    assert_eq!(instance["variant"], json!({"size": "sm", "variant": "primary"}));
    assert_eq!(instance["props"], json!({"label": "Save"}));
    assert_eq!(
        instance["descendants"],
        json!({"btn-ps": {"opacity": 0.5}, "btn-ps-label": {"fontSize": 20}})
    );
    assert_eq!(
        report.dropped_overrides,
        [DroppedOverride {
            instance: "i1".into(),
            key: "btn-pm-dot".into()
        }]
    );
    assert_eq!(report.warnings().len(), 1);

    // The canvas shows the new variant with the props and kept overrides.
    let resolved = resolve_refs_for_canvas(&state.doc);
    let screen = serde_json::to_value(&resolved.pages.as_ref().unwrap()[0]).unwrap();
    let shown = &screen["children"][0]["children"][0];
    assert_eq!(shown["width"], 90.0);
    assert_eq!(shown["opacity"], 0.5);
    assert_eq!(shown["children"][0]["content"], "Save");
    assert_eq!(shown["children"][0]["fontSize"], 20.0);

    assert!(state.undo());
    assert_eq!(node_json(&state, "i1")["ref"], "btn-pm");
}

#[test]
fn a_missing_combination_shows_the_nearest_and_is_reported() {
    let mut state = with_set();
    let report = switch(&mut state, "i2", &[("variant", "ghost")]);
    // ghost/sm does not exist: ghost/md is nearest (variant first).
    assert_eq!(node_json(&state, "i2")["ref"], "btn-gm");
    assert_eq!(
        report.fallbacks,
        [VariantFallback {
            instance: "i2".into(),
            requested: values(&[("variant", "ghost"), ("size", "sm")]),
            shown: "btn-gm".into(),
        }]
    );
    // Adding the missing variant moves the instance onto it.
    let report = state
        .apply_component_set(&ComponentSetCommand::AddVariant {
            set_id: "button".into(),
            values: values(&[("variant", "ghost"), ("size", "sm")]),
            from: Some("btn-ps".into()),
            new_id: None,
        })
        .unwrap();
    assert_eq!(report.created, ["button-ghost-sm"]);
    assert_eq!(node_json(&state, "i2")["ref"], "button-ghost-sm");
    let copy = node_json(&state, "button-ghost-sm");
    assert_eq!(copy["name"], "variant=ghost, size=sm");
    let bound = copy["componentProperties"]["label"]["bind"][0]["node"].clone();
    assert_eq!(bound, copy["children"][0]["id"], "binds follow the copy's ids");
    assert!(state
        .components
        .find_by_id(&NodeId::new("button-ghost-sm"))
        .is_some());
}

#[test]
fn renames_reach_masters_and_instances_on_every_page() {
    let mut state = with_set();
    switch(&mut state, "i2", &[("size", "sm")]);
    state
        .apply_component_set(&ComponentSetCommand::RenameAxis {
            set_id: "button".into(),
            from: "size".into(),
            to: "scale".into(),
        })
        .unwrap();
    state
        .apply_component_set(&ComponentSetCommand::RenameValue {
            set_id: "button".into(),
            axis: "scale".into(),
            from: "sm".into(),
            to: "small".into(),
        })
        .unwrap();
    assert_eq!(
        node_json(&state, "i2")["variant"],
        json!({"scale": "small", "variant": "primary"})
    );
    assert_eq!(node_json(&state, "btn-ps")["name"], "variant=primary, scale=small");
    assert_eq!(
        state
            .components
            .find_by_id(&NodeId::new("btn-ps"))
            .unwrap()
            .name,
        "variant=primary, scale=small"
    );
    let set = node_json(&state, "button");
    assert_eq!(set["componentSet"]["axes"]["scale"], json!(["md", "small"]));
    assert_eq!(set["componentSet"]["default"]["scale"], "md");
}

#[test]
fn removing_a_value_moves_its_instances_to_the_default() {
    let mut state = with_set();
    switch(&mut state, "i1", &[("variant", "ghost")]);
    let report = state
        .apply_component_set(&ComponentSetCommand::RemoveValue {
            set_id: "button".into(),
            axis: "variant".into(),
            value: "ghost".into(),
        })
        .unwrap();
    assert_eq!(report.removed, ["btn-gm"]);
    assert_eq!(report.moved_to_default, ["i1"]);
    assert_eq!(node_json(&state, "i1")["ref"], "btn-pm");
    assert!(state.components.find_by_id(&NodeId::new("btn-gm")).is_none());
}

#[test]
fn bad_writes_are_refused_with_the_allowed_names() {
    let mut state = with_set();
    let duplicate = state.apply_component_set(&ComponentSetCommand::AddVariant {
        set_id: "button".into(),
        values: values(&[("size", "sm")]),
        from: Some("btn-pm".into()),
        new_id: None,
    });
    assert_eq!(
        duplicate.unwrap_err().to_string(),
        "a variant with size=sm, variant=primary already exists"
    );
    let unknown = state.apply_component_set(&ComponentSetCommand::SetInstanceVariant {
        node_id: "i1".into(),
        variant: values(&[("state", "hover")]),
    });
    assert_eq!(
        unknown.unwrap_err().to_string(),
        "unknown variant axis state; axes: variant, size"
    );
    let taken = state.apply_component_set(&ComponentSetCommand::Create(CreateSet {
        name: "Again".into(),
        master_ids: vec!["btn-pm".into()],
        ..Default::default()
    }));
    assert_eq!(
        taken.unwrap_err(),
        VariantError::AlreadyInSet {
            master: "btn-pm".into(),
            set: "button".into()
        }
    );
    let mut fresh = EditorState::from_document(doc(MASTERS));
    let same = fresh.apply_component_set(&ComponentSetCommand::Create(CreateSet {
        name: "Same".into(),
        master_ids: vec!["btn-pm".into(), "btn-gm".into()],
        values: Some(vec![values(&[("v", "a")]), values(&[("v", "a")])]),
        ..Default::default()
    }));
    assert_eq!(
        same.unwrap_err().to_string(),
        "a variant with v=a already exists"
    );
}

#[test]
fn the_command_runs_through_editor_command_as_one_step() {
    let mut state = with_set();
    let depth = state.history.past.len();
    assert!(state.apply(EditorCommand::ComponentSet(Box::new(
        ComponentSetCommand::AddAxis {
            set_id: "button".into(),
            axis: "state".into(),
            value: "default".into(),
        }
    ))));
    assert_eq!(state.history.past.len(), depth + 1);
    assert_eq!(
        node_json(&state, "btn-gm")["name"],
        "variant=ghost, size=md, state=default"
    );
    assert!(!state.apply(EditorCommand::ComponentSet(Box::new(
        ComponentSetCommand::AddAxis {
            set_id: "button".into(),
            axis: "state".into(),
            value: "x".into(),
        }
    ))));
    assert_eq!(state.history.past.len(), depth + 1, "a refusal adds no step");
}

#[test]
fn three_pages_survive_save_and_load() {
    let mut state = with_set();
    switch(&mut state, "i1", &[("size", "sm")]);
    switch(&mut state, "i2", &[("variant", "ghost"), ("size", "md")]);
    let before = serde_json::to_value(resolve_refs_for_canvas(&state.doc).pages).unwrap();
    let saved = serde_json::to_string(&state.doc).unwrap();
    let reopened = EditorState::from_opened_document(doc(&saved));
    assert_eq!(
        set_summary(&reopened.doc, "button"),
        set_summary(&state.doc, "button")
    );
    assert_eq!(node_json(&reopened, "i1"), node_json(&state, "i1"));
    let after = serde_json::to_value(resolve_refs_for_canvas(&reopened.doc).pages).unwrap();
    assert_eq!(after, before);
    let reopened_set = super::tree::find(&reopened.doc, "button").unwrap();
    assert_eq!(variant_masters(reopened_set).len(), 3);
}
