//! Write-time animation checks (#351): the `scale` alias expands and an
//! animation the loader would not accept is refused with its reason.

use super::conversion_tools::upsert_component_snapshot;
use super::write_tools::{insert_node_snapshot, update_node_snapshot};
use super::{EditorCommand, McpTool, ToolErrorCode, ToolOutcome};
use serde_json::Value;
use std::collections::BTreeMap;

fn animations(property: &str, first_offset: f32) -> String {
    format!(
        r#""animations":[{{"trigger":"mount","durationMs":200,"keyframes":[{{"offset":{first_offset},"values":{{"{property}":0.5}}}},{{"offset":1,"values":{{"{property}":1}}}}]}}]"#
    )
}

fn update_args(data: String) -> BTreeMap<String, String> {
    let mut args = BTreeMap::new();
    args.insert("nodeId".into(), "n11".into());
    args.insert("data".into(), data);
    args
}

#[test]
fn update_node_expands_the_scale_alias() {
    let args = update_args(format!("{{{}}}", animations("scale", 0.0)));
    let ToolOutcome::OkWithCommand(_, EditorCommand::PatchNodeData { patch_json, .. }) =
        update_node_snapshot().call(&args)
    else {
        panic!("expected PatchNodeData for a scale animation");
    };
    let patch: Value = serde_json::from_str(&patch_json).expect("patch json");
    let first = &patch["animations"][0]["keyframes"][0]["values"];
    assert_eq!(first["scaleX"], 0.5);
    assert_eq!(first["scaleY"], 0.5);
    assert!(first.get("scale").is_none());
}

#[test]
fn update_node_refuses_an_unknown_property_by_name() {
    let args = update_args(format!("{{{}}}", animations("zoom", 0.0)));
    let ToolOutcome::Err(code, message) = update_node_snapshot().call(&args) else {
        panic!("expected an error for an unknown animation property");
    };
    assert_eq!(code, ToolErrorCode::InvalidArgument);
    assert!(
        message.contains("unknown animation property zoom"),
        "{message}"
    );
    assert!(message.contains("scaleX"), "{message}");
}

#[test]
fn instance_descendant_update_refuses_an_unknown_property() {
    let mut args = update_args(format!("{{{}}}", animations("zoom", 0.0)));
    args.insert("nodeId".into(), "inst/child".into());
    let ToolOutcome::Err(_, message) = update_node_snapshot().call(&args) else {
        panic!("expected an error for an unknown animation property");
    };
    assert!(
        message.contains("unknown animation property zoom"),
        "{message}"
    );
}

#[test]
fn insert_node_refuses_a_child_animation_without_offset_zero() {
    let mut args = BTreeMap::new();
    args.insert("parent".into(), "n10".into());
    args.insert(
        "data".into(),
        format!(
            r#"{{"type":"frame","name":"Card","width":200,"height":120,"children":[{{"type":"rectangle","name":"Dot","width":10,"height":10,{}}}]}}"#,
            animations("opacity", 0.5)
        ),
    );
    let ToolOutcome::Err(code, message) = insert_node_snapshot().call(&args) else {
        panic!("expected an error for a child animation without offset 0");
    };
    assert_eq!(code, ToolErrorCode::InvalidArgument);
    assert!(message.contains("$.children[0].animations[0]"), "{message}");
    assert!(
        message.contains("first keyframe offset must be 0"),
        "{message}"
    );
}

#[test]
fn upsert_component_expands_the_scale_alias_in_its_root() {
    let mut args = BTreeMap::new();
    args.insert("key".into(), "button".into());
    args.insert("name".into(), "Button".into());
    args.insert(
        "node_json".into(),
        format!(
            r#"{{"type":"frame","id":"btn","width":80,"height":32,{}}}"#,
            animations("scale", 0.0)
        ),
    );
    let ToolOutcome::OkWithCommand(_, EditorCommand::UpsertComponent { root, .. }) =
        upsert_component_snapshot().call(&args)
    else {
        panic!("expected UpsertComponent");
    };
    let animations = root.motion_declarations().1.expect("animations");
    assert!(animations[0].keyframes[0].values.contains_key("scaleX"));
    assert!(!animations[0].keyframes[0].values.contains_key("scale"));
}

#[test]
fn copy_node_overrides_refuse_an_unknown_property_by_name() {
    let mut args = BTreeMap::new();
    args.insert("nodeId".into(), "n11".into());
    args.insert("parent".into(), "n10".into());
    args.insert(
        "overrides".into(),
        format!("{{{}}}", animations("zoom", 0.0)),
    );
    let ToolOutcome::Err(code, message) = super::reparent_tools::copy_node_snapshot().call(&args)
    else {
        panic!("expected an error for an unknown animation property");
    };
    assert_eq!(code, ToolErrorCode::InvalidArgument);
    assert!(
        message.contains("unknown animation property zoom"),
        "{message}"
    );
}
