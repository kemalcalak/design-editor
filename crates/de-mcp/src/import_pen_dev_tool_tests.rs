use std::collections::BTreeMap;
use std::path::PathBuf;

use de_editor_core::{EditorState, PenNodeExt};
use jian_ops_schema::node::PenNode;

use super::import_pen_dev_tool::import_pen_dev_snapshot;
use super::{EditorCommand, McpTool, ToolOutcome};

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "de-mcp-pen-dev-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(dir.join("images")).expect("create temp dir");
    dir
}

/// A document that already uses the id `btn` and the variable `bg`.
fn host_state() -> EditorState {
    let doc = jian_ops_schema::load_str(
        r##"{"version":"1.0","themes":{"mode":["light"]},
             "variables":{"bg":{"type":"color","value":"#123456"}},
             "children":[{"type":"frame","id":"btn","width":10,"height":10}]}"##,
    )
    .expect("host fixture")
    .value;
    EditorState::from_document(doc)
}

const DESIGN: &str = r##"{
  "version": "2.21",
  "themes": { "mode": ["light", "dark"] },
  "variables": {
    "bg": { "type": "color", "value": "#FFFFFF" },
    "accent": { "type": "color", "value": "#3E6DF5" }
  },
  "children": [
    { "type": "frame", "id": "btn", "name": "Button", "reusable": true, "x": 0, "y": 0,
      "fill": { "type": "image", "url": "images/a.png" },
      "children": [{ "type": "text", "id": "label", "content": "Go", "fill": "$accent" }] },
    { "type": "ref", "id": "use", "ref": "btn", "x": 200, "y": 0,
      "descendants": { "label": { "content": "Buy" } } },
    { "type": "note", "id": "n", "content": "todo" }
  ]
}"##;

fn call(dir: &std::path::Path, state: &EditorState) -> ToolOutcome {
    std::fs::write(dir.join("images/a.png"), [0x89, b'P', b'N', b'G']).unwrap();
    let pen = dir.join("design.pen");
    std::fs::write(&pen, DESIGN).unwrap();
    let mut args = BTreeMap::new();
    args.insert("penPath".to_string(), pen.display().to_string());
    import_pen_dev_snapshot(state).call(&args)
}

#[test]
fn imports_keep_instances_bound_and_existing_variables_untouched() {
    let dir = temp_dir("bound");
    let state = host_state();
    let ToolOutcome::OkWithCommand(result, EditorCommand::Batch { commands }) = call(&dir, &state)
    else {
        panic!("expected a batch command");
    };
    assert_eq!(
        result.get("keptExistingVariables").map(String::as_str),
        Some("bg")
    );
    assert_eq!(result.get("renamedIds").map(String::as_str), Some("1"));
    assert!(result["notCarriedOver"].contains("note ×1"));

    let [EditorCommand::SetVariables {
        variables,
        replace: false,
    }, EditorCommand::SetThemes {
        themes,
        replace: false,
    }, EditorCommand::InsertAuthoredSubtreePreservingRoots { nodes, .. }] = commands.as_slice()
    else {
        panic!("variables, themes, then the boards: {commands:?}");
    };
    assert_eq!(variables.keys().collect::<Vec<_>>(), ["accent"]);
    assert_eq!(themes["mode"], ["light", "dark"]);

    // `btn` collides with the host document: renamed, and the instance
    // follows it.
    assert_eq!(nodes.len(), 2, "the note is dropped");
    assert_eq!(nodes[0].id_str(), "btn-2");
    let PenNode::Ref(instance) = &nodes[1] else {
        panic!("instance expected");
    };
    assert_eq!(instance.target, "btn-2");
    assert!(instance.descendants.as_ref().unwrap().contains_key("label"));

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_batch_applies_in_one_step() {
    let dir = temp_dir("apply");
    let mut state = host_state();
    let ToolOutcome::OkWithCommand(_, command) = call(&dir, &state) else {
        panic!("expected a command");
    };
    assert!(state.apply(command), "batch applies");
    let ids: Vec<&str> = state.doc.children.iter().map(|n| n.id_str()).collect();
    assert!(ids.contains(&"btn") && ids.contains(&"btn-2") && ids.contains(&"use"));
    let vars = state.doc.variables.as_ref().unwrap();
    assert_eq!(
        serde_json::to_value(&vars["bg"]).unwrap()["value"],
        "#123456",
        "the document's own variable wins"
    );
    assert!(vars.contains_key("accent"));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_file_that_is_not_pen_dev_is_refused() {
    let dir = temp_dir("refuse");
    let op = dir.join("doc.op");
    std::fs::write(&op, r#"{"version":"1.0","children":[]}"#).unwrap();
    let mut args = BTreeMap::new();
    args.insert("penPath".to_string(), op.display().to_string());
    let outcome = import_pen_dev_snapshot(&EditorState::new()).call(&args);
    assert!(matches!(outcome, ToolOutcome::Err(..)));
    let missing = import_pen_dev_snapshot(&EditorState::new()).call(&BTreeMap::new());
    assert!(matches!(missing, ToolOutcome::Err(..)));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn instances_of_a_library_component_never_borrow_a_host_node() {
    let dir = temp_dir("dangling");
    let pen = dir.join("lib.pen");
    // `kit-button` came from a `.pen` library the converter does not follow.
    std::fs::write(
        &pen,
        r##"{"version":"2.21","children":[
            { "type": "ref", "id": "a", "ref": "btn", "x": 0, "y": 0 },
            { "type": "ref", "id": "b", "ref": "kit-button", "x": 100, "y": 0 }
        ]}"##,
    )
    .unwrap();
    let mut args = BTreeMap::new();
    args.insert("penPath".to_string(), pen.display().to_string());
    // The host document happens to own a node called `btn`.
    let ToolOutcome::OkWithCommand(result, EditorCommand::Batch { commands }) =
        import_pen_dev_snapshot(&host_state()).call(&args)
    else {
        panic!("expected a batch");
    };
    assert_eq!(
        result.get("droppedInstances").map(String::as_str),
        Some("1")
    );
    assert_eq!(
        result.get("instancesWithoutComponent").map(String::as_str),
        Some("1")
    );
    let Some(EditorCommand::InsertAuthoredSubtreePreservingRoots { nodes, .. }) = commands.last()
    else {
        panic!("insert last");
    };
    let ids: Vec<&str> = nodes.iter().map(|node| node.id_str()).collect();
    assert_eq!(
        ids,
        ["b"],
        "the instance that would show the host's `btn` is gone"
    );
    let _ = std::fs::remove_dir_all(dir);
}
