//! Transaction regression tests for `U()` sizing updates in mixed programs.

use std::collections::BTreeMap;

use de_editor_core::{EditorCommand, NodeId};

use super::batch_design_snapshot;
use super::test_fixtures::sample;
use super::{McpTool, ToolOutcome};

#[test]
fn batch_program_transaction_applies_sizing_keywords() {
    let mut state = sample();
    let tool = batch_design_snapshot(&state);
    let mut args = BTreeMap::new();
    args.insert(
        "operations".into(),
        concat!(
            "U(\"n10\", {\"x\":48})\n",
            "U(\"n10\", {\"width\":\"fill_container\",\"height\":\"fit_content\"})"
        )
        .into(),
    );

    let ToolOutcome::OkJsonWithCommand(json, command) = tool.call(&args) else {
        panic!("successful transaction must emit a command");
    };
    let envelope: serde_json::Value = serde_json::from_str(&json).expect("result envelope");
    assert!(envelope.get("errors").is_none(), "{envelope}");
    let EditorCommand::Batch { commands } = &command else {
        panic!("two updates must remain one atomic batch, got {command:?}");
    };
    assert!(matches!(commands[0], EditorCommand::UpdateNode { .. }));
    assert!(matches!(commands[1], EditorCommand::PatchNodeData { .. }));

    assert!(state.apply(command), "complete transaction must apply");
    let node = de_editor_core::walkers::find_node(state.active_children(), &NodeId::new("n10"))
        .expect("updated frame");
    let value = serde_json::to_value(node).expect("updated node json");
    assert_eq!(value["x"].as_f64(), Some(48.0));
    assert_eq!(value["width"], "fill_container");
    assert_eq!(value["height"], "fit_content");
}

#[test]
fn batch_program_transaction_applies_token_only_fill_patch() {
    let mut state = sample();
    let tool = batch_design_snapshot(&state);
    let mut args = BTreeMap::new();
    args.insert(
        "operations".into(),
        concat!(
            "U(\"n10\", {\"x\":48})\n",
            "U(\"n10\", {\"fill\":[{\"type\":\"solid\",\"color\":\"$--primary\"}]})"
        )
        .into(),
    );

    let ToolOutcome::OkJsonWithCommand(json, command) = tool.call(&args) else {
        panic!("successful transaction must emit a command");
    };
    let envelope: serde_json::Value = serde_json::from_str(&json).expect("result envelope");
    assert!(envelope.get("errors").is_none(), "{envelope}");
    let EditorCommand::Batch { commands } = &command else {
        panic!("two updates must remain one atomic batch, got {command:?}");
    };
    assert!(matches!(commands[0], EditorCommand::UpdateNode { .. }));
    assert!(matches!(commands[1], EditorCommand::PatchNodeData { .. }));

    assert!(state.apply(command), "complete transaction must apply");
    let node = de_editor_core::walkers::find_node(state.active_children(), &NodeId::new("n10"))
        .expect("updated frame");
    let value = serde_json::to_value(node).expect("updated node json");
    assert_eq!(value["x"].as_f64(), Some(48.0));
    assert_eq!(value["fill"][0]["color"], "$--primary");
}

/// An `auto` text ignores a numeric width in layout, so `U({width})` switches
/// it to `fixed-width`; `textGrowth: "auto"` in the same call still wins.
#[test]
fn width_update_switches_auto_text_to_fixed_width_unless_auto_is_requested() {
    for (patch, expected) in [
        (r#"{"width":200}"#, "fixed-width"),
        (r#"{"width":200,"textGrowth":"auto"}"#, "auto"),
    ] {
        let mut state = sample();
        let mut set_auto = BTreeMap::new();
        set_auto.insert(
            "operations".into(),
            "U(\"n11\", {\"textGrowth\":\"auto\"})".into(),
        );
        apply_operations(&mut state, set_auto);
        let mut args = BTreeMap::new();
        args.insert("operations".into(), format!("U(\"n11\", {patch})"));
        apply_operations(&mut state, args);
        let node = de_editor_core::walkers::find_node(state.active_children(), &NodeId::new("n11"))
            .expect("updated text");
        let value = serde_json::to_value(node).expect("updated node json");
        assert_eq!(value["width"].as_f64(), Some(200.0), "{patch}");
        assert_eq!(value["textGrowth"], expected, "{patch}");
    }
}

fn apply_operations(state: &mut de_editor_core::EditorState, args: BTreeMap<String, String>) {
    let tool = batch_design_snapshot(state);
    let command = match tool.call(&args) {
        ToolOutcome::OkWithCommand(_, command) | ToolOutcome::OkJsonWithCommand(_, command) => {
            command
        }
        other => panic!("update must emit a command, got {other:?}"),
    };
    assert!(state.apply(command), "update must apply");
}
