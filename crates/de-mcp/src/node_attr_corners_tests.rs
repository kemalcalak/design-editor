//! Tests for `set_node_corner_radius` (#389): argument parsing and the
//! one validated `SetNodeCorners` step it emits.

use super::node_attr_corners::{parse_radii, parse_smoothing};
use super::node_attr_tools::set_node_corner_radius_snapshot;
use super::test_fixtures::{rect, state_with, text};
use super::{EditorCommand, McpTool, ToolErrorCode, ToolOutcome};
use de_editor_core::{EditorState, NodeId};
use std::collections::BTreeMap;

fn args(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    let mut args = BTreeMap::new();
    args.insert("node_id".into(), "r".into());
    for (key, value) in pairs {
        args.insert((*key).into(), (*value).into());
    }
    args
}

fn state() -> EditorState {
    state_with(vec![
        rect("r", "box", 0.0, 0.0, 80.0, 60.0),
        text("t", "label", 0.0, 80.0, 80.0, 20.0, "Hi"),
    ])
}

fn node_json(s: &EditorState, id: &str) -> serde_json::Value {
    let node =
        de_editor_core::walkers::find_node(s.active_children(), &NodeId::new(id)).expect("node");
    serde_json::to_value(node).expect("json")
}

fn command(pairs: &[(&str, &str)]) -> EditorCommand {
    match set_node_corner_radius_snapshot().call(&args(pairs)) {
        ToolOutcome::OkWithCommand(_, command) => command,
        other => panic!("{pairs:?}: expected a command, got {other:?}"),
    }
}

#[test]
fn radius_takes_one_number_or_four() {
    assert_eq!(parse_radii("12"), Ok([12.0; 4]));
    assert_eq!(parse_radii(" 0 "), Ok([0.0; 4]));
    assert_eq!(parse_radii("[1, 2, 3, 4]"), Ok([1.0, 2.0, 3.0, 4.0]));
    assert_eq!(parse_radii("[1,2,3,4]"), Ok([1.0, 2.0, 3.0, 4.0]));
    assert_eq!(parse_radii("1 2 3 4"), Ok([1.0, 2.0, 3.0, 4.0]));
    for bad in [
        "",
        "-1",
        "nan",
        "inf",
        "[1, 2]",
        "[1]",
        "[1, 2, 3, -4]",
        "[1, 2, 3, 4",
        "1 2",
        "x",
    ] {
        assert!(parse_radii(bad).is_err(), "{bad:?}");
    }
}

#[test]
fn smoothing_is_a_fraction_or_a_percentage() {
    assert_eq!(parse_smoothing("0.6"), Ok(0.6));
    assert_eq!(parse_smoothing("0"), Ok(0.0));
    assert_eq!(parse_smoothing("1"), Ok(1.0));
    assert_eq!(parse_smoothing("60%"), Ok(0.6));
    assert_eq!(parse_smoothing("100 %"), Ok(1.0));
    // A bare 60 is not read as a percentage: it is out of range.
    for bad in ["60", "-0.1", "1.5", "101%", "nan", "%", "x"] {
        assert!(parse_smoothing(bad).is_err(), "{bad:?}");
    }
}

#[test]
fn the_tool_needs_a_radius_or_a_smoothing() {
    match set_node_corner_radius_snapshot().call(&args(&[])) {
        ToolOutcome::Err(code, _) => assert_eq!(code, ToolErrorCode::MissingArgument),
        other => panic!("expected MissingArgument, got {other:?}"),
    }
    for pairs in [
        &[("radius", "-2")][..],
        &[("radius", "[1, 2, 3]")][..],
        &[("smoothing", "60")][..],
        &[("radius", "4"), ("smoothing", "2")][..],
    ] {
        match set_node_corner_radius_snapshot().call(&args(pairs)) {
            ToolOutcome::Err(code, _) => assert_eq!(code, ToolErrorCode::InvalidArgument),
            other => panic!("{pairs:?}: expected InvalidArgument, got {other:?}"),
        }
    }
}

#[test]
fn per_corner_radii_and_smoothing_land_in_one_step() {
    let mut s = state();
    assert!(s.apply(command(&[("radius", "[2, 4, 6, 8]"), ("smoothing", "60%")])));
    let json = node_json(&s, "r");
    assert_eq!(
        json["cornerRadius"],
        serde_json::json!([2.0, 4.0, 6.0, 8.0])
    );
    assert_eq!(json["cornerSmoothing"], serde_json::json!(0.6));
    // Smoothing alone keeps the radii; a uniform number stores one value.
    assert!(s.apply(command(&[("smoothing", "0")])));
    let json = node_json(&s, "r");
    assert!(json.get("cornerSmoothing").is_none(), "{json}");
    assert_eq!(
        json["cornerRadius"],
        serde_json::json!([2.0, 4.0, 6.0, 8.0])
    );
    assert!(s.apply(command(&[("radius", "10")])));
    assert_eq!(node_json(&s, "r")["cornerRadius"], serde_json::json!(10.0));
}

#[test]
fn a_node_without_corners_is_refused_untouched() {
    let mut s = state();
    let before = node_json(&s, "t");
    let command = match set_node_corner_radius_snapshot().call(&{
        let mut a = args(&[("radius", "4")]);
        a.insert("node_id".into(), "t".into());
        a
    }) {
        ToolOutcome::OkWithCommand(_, command) => command,
        other => panic!("expected a command, got {other:?}"),
    };
    assert!(!s.apply(command));
    assert_eq!(node_json(&s, "t"), before);
}
