//! #169: pen.dev `Get` options on `read_nodes`, instance-path writes, and
//! clipping problems in `snapshot_layout`.

use std::collections::BTreeMap;

use de_editor_core::{EditorState, NodeId};
use serde_json::{json, Value};

use super::{
    batch_design_snapshot, read_nodes_snapshot, snapshot_layout_snapshot, update_node_snapshot,
    McpTool, ToolOutcome,
};

const DOC: &str = r##"{
  "version":"1.0.0",
  "variables":{"accent":{"type":"color","value":[
    {"value":"#ff0000","theme":{"mode":"light"}},
    {"value":"#00ff00","theme":{"mode":"dark"}}
  ]}},
  "themes":{"mode":["light","dark"]},
  "children":[
    {"type":"frame","id":"card","name":"Card","reusable":true,"x":0,"y":0,"width":200,"height":100,
     "children":[
       {"type":"text","id":"title","name":"Title","content":"Hello",
        "fill":[{"type":"solid","color":"$accent"}]},
       {"type":"path","id":"icon","name":"Icon","d":"M0 0L10 10","width":10,"height":10}
     ]},
    {"type":"ref","id":"inst1","ref":"card","x":300,"y":0},
    {"type":"frame","id":"viewport","name":"Viewport","x":0,"y":400,"width":100,"height":100,
     "clipContent":true,"layout":"none","children":[
       {"type":"rectangle","id":"inside","x":10,"y":10,"width":20,"height":20},
       {"type":"rectangle","id":"edge","x":90,"y":10,"width":20,"height":20},
       {"type":"rectangle","id":"gone","x":150,"y":10,"width":20,"height":20}
     ]}
  ]
}"##;

fn state() -> EditorState {
    let doc = jian_ops_schema::load_str(DOC)
        .expect("fixture parses")
        .value;
    EditorState::from_document(doc)
}

fn read(state: &EditorState, pairs: &[(&str, &str)]) -> Value {
    let args: BTreeMap<String, String> = pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    match read_nodes_snapshot(state).call(&args) {
        ToolOutcome::OkJson(json) => serde_json::from_str(&json).expect("read_nodes json"),
        other => panic!("expected OkJson, got {other:?}"),
    }
}

fn apply(state: &mut EditorState, outcome: ToolOutcome) {
    let command = match outcome {
        ToolOutcome::OkWithCommand(_, command) | ToolOutcome::OkJsonWithCommand(_, command) => {
            command
        }
        other => panic!("write must emit a command, got {other:?}"),
    };
    assert!(state.apply(command), "write must apply");
}

fn descendants(state: &EditorState) -> Value {
    let node = de_editor_core::walkers::find_node(state.active_children(), &NodeId::new("inst1"))
        .expect("instance");
    serde_json::to_value(node).expect("json")["descendants"].clone()
}

#[test]
fn instances_stay_refs_unless_resolved() {
    let s = state();
    let plain = read(&s, &[("nodeIds", r#"["inst1"]"#)]);
    assert_eq!(plain["nodes"][0]["type"], "ref");
    assert!(plain["nodes"][0].get("children").is_none());

    let resolved = read(
        &s,
        &[("nodeIds", r#"["inst1"]"#), ("resolveInstances", "true")],
    );
    let node = &resolved["nodes"][0];
    assert_eq!(node["id"], "inst1");
    assert_eq!(node["type"], "frame");
    assert_eq!(node["x"], 300.0);
    let ids: Vec<&str> = node["children"]
        .as_array()
        .expect("expanded children")
        .iter()
        .map(|c| c["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["inst1/title", "inst1/icon"]);
}

#[test]
fn an_instance_path_reads_the_expanded_child() {
    let s = state();
    let v = read(&s, &[("nodeIds", r#"["inst1/title"]"#)]);
    assert_eq!(v["nodes"][0]["id"], "inst1/title");
    assert_eq!(v["nodes"][0]["content"], "Hello");
}

#[test]
fn resolve_variables_uses_the_active_theme() {
    let mut s = state();
    let raw = read(&s, &[("nodeIds", r#"["title"]"#)]);
    assert_eq!(raw["nodes"][0]["fill"][0]["color"], "$accent");

    let light = read(
        &s,
        &[("nodeIds", r#"["title"]"#), ("resolveVariables", "true")],
    );
    assert_eq!(light["nodes"][0]["fill"][0]["color"], "#ff0000");

    s.ui.variables
        .active_theme
        .insert("mode".into(), "dark".into());
    let dark = read(
        &s,
        &[("nodeIds", r#"["title"]"#), ("resolveVariables", "true")],
    );
    assert_eq!(dark["nodes"][0]["fill"][0]["color"], "#00ff00");
}

#[test]
fn path_geometry_is_left_out_unless_asked_for() {
    let s = state();
    let without = read(&s, &[("nodeIds", r#"["icon"]"#)]);
    assert!(without["nodes"][0].get("d").is_none(), "{without}");
    assert_eq!(without["nodes"][0]["width"], 10.0);

    let with = read(
        &s,
        &[("nodeIds", r#"["icon"]"#), ("includePathGeometry", "true")],
    );
    assert_eq!(with["nodes"][0]["d"], "M0 0L10 10");
}

#[test]
fn a_bad_flag_value_is_rejected() {
    let s = state();
    let mut args = BTreeMap::new();
    args.insert("resolveInstances".into(), "yes".into());
    match read_nodes_snapshot(&s).call(&args) {
        ToolOutcome::Err(_, msg) => assert!(msg.contains("resolveInstances"), "{msg}"),
        other => panic!("expected an error, got {other:?}"),
    }
}

#[test]
fn update_node_writes_an_instance_path_as_an_override() {
    let mut s = state();
    let mut args = BTreeMap::new();
    args.insert("nodeId".into(), "inst1/title".into());
    args.insert("data".into(), r#"{"content":"Hi"}"#.into());
    let outcome = update_node_snapshot().call(&args);
    apply(&mut s, outcome);
    assert_eq!(descendants(&s), json!({"title": {"content": "Hi"}}));
}

#[test]
fn batch_design_update_accepts_an_instance_path() {
    let mut s = state();
    let mut args = BTreeMap::new();
    args.insert(
        "operations".into(),
        r#"U("inst1/title", {"content":"Hey"})"#.into(),
    );
    let outcome = batch_design_snapshot(&s).call(&args);
    apply(&mut s, outcome);
    assert_eq!(descendants(&s), json!({"title": {"content": "Hey"}}));
}

#[test]
fn snapshot_layout_marks_clipped_children() {
    let s = state();
    let mut args = BTreeMap::new();
    args.insert("parentId".into(), "viewport".into());
    let json = match snapshot_layout_snapshot(&s).call(&args) {
        ToolOutcome::OkJson(json) => json,
        other => panic!("expected OkJson, got {other:?}"),
    };
    let v: Value = serde_json::from_str(&json).expect("layout json");
    let problems: Vec<(String, Option<String>)> = v["layout"]
        .as_array()
        .expect("layout entries")
        .iter()
        .map(|e| {
            (
                e["id"].as_str().unwrap().to_string(),
                e.get("problems")
                    .and_then(Value::as_str)
                    .map(str::to_string),
            )
        })
        .collect();
    assert_eq!(
        problems,
        [
            ("inside".to_string(), None),
            ("edge".to_string(), Some("partially clipped".to_string())),
            ("gone".to_string(), Some("fully clipped".to_string())),
        ]
    );
}
