use super::*;
use de_editor_core::PenNodeExt;
use jian_ops_schema::node::PenNode;
use serde_json::json;

/// Two overlapping squares plus a text node on a single-page document.
fn state() -> EditorState {
    let doc: jian_ops_schema::PenDocument = serde_json::from_value(json!({
        "version": "1.0",
        "children": [
            {"type":"rectangle", "id":"a", "name":"A", "x":0, "y":0, "width":100, "height":100,
             "fill":[{"type":"solid","color":"#ff0000"}]},
            {"type":"rectangle", "id":"b", "name":"B", "x":50, "y":50, "width":100, "height":100},
            {"type":"text", "id":"t", "name":"T", "x":0, "y":200, "content":"hi"}
        ]
    }))
    .expect("document");
    EditorState::from_document(doc)
}

fn call(state: &EditorState, ids: &str, operation: &str) -> ToolOutcome {
    let mut args = BTreeMap::new();
    args.insert("nodeIds".to_string(), ids.to_string());
    args.insert("operation".to_string(), operation.to_string());
    boolean_op_snapshot(state).call(&args)
}

fn error_code(outcome: ToolOutcome) -> ToolErrorCode {
    match outcome {
        ToolOutcome::Err(code, _) => code,
        other => panic!("expected an error, got {other:?}"),
    }
}

#[test]
fn union_returns_the_id_the_commit_lands_as_one_undo_step() {
    let mut state = state();
    let depth = state.history.past.len();
    let ToolOutcome::OkWithCommand(out, command) = call(&state, r#"["a","b"]"#, "union") else {
        panic!("expected a write");
    };
    let new_id = out.get("id").cloned().expect("id in the result");

    assert!(state.apply(command));

    let ids: Vec<&str> = state.active_children().iter().map(|n| n.id_str()).collect();
    assert_eq!(ids, ["t", new_id.as_str()]);
    let PenNode::Path(path) = &state.active_children()[1] else {
        panic!("boolean result should be a path");
    };
    assert_eq!(path.base.x, Some(0.0));
    assert_eq!(path.base.y, Some(0.0));
    assert!(path.fill.is_some(), "result keeps the first source's fill");
    assert_eq!(state.selection.set, vec![NodeId::new(&new_id)]);
    assert_eq!(state.history.past.len(), depth + 1);

    assert!(state.undo());
    let ids: Vec<&str> = state.active_children().iter().map(|n| n.id_str()).collect();
    assert_eq!(ids, ["a", "b", "t"]);
}

#[test]
fn every_operation_commits() {
    for operation in ["union", "subtract", "intersect", "exclude"] {
        let mut state = state();
        let ToolOutcome::OkWithCommand(_, command) = call(&state, "a, b", operation) else {
            panic!("{operation}: expected a write");
        };
        assert!(state.apply(command), "{operation}: commit applies");
        assert_eq!(state.active_children().len(), 2, "{operation}");
    }
}

#[test]
fn rejects_bad_arguments() {
    let state = state();
    assert_eq!(
        error_code(call(&state, r#"["a"]"#, "union")),
        ToolErrorCode::InvalidArgument
    );
    assert_eq!(
        error_code(call(&state, r#"["a","b"]"#, "merge")),
        ToolErrorCode::InvalidArgument
    );
    assert_eq!(
        error_code(call(&state, r#"["a","missing"]"#, "union")),
        ToolErrorCode::InvalidArgument
    );
    assert_eq!(
        error_code(call(&state, r#"["a","a"]"#, "union")),
        ToolErrorCode::InvalidArgument
    );
    let mut args = BTreeMap::new();
    args.insert("operation".to_string(), "union".to_string());
    assert_eq!(
        error_code(boolean_op_snapshot(&state).call(&args)),
        ToolErrorCode::MissingArgument
    );
}

#[test]
fn rejects_unsupported_shapes_and_sources_with_children() {
    let state = state();
    assert_eq!(
        error_code(call(&state, r#"["a","t"]"#, "union")),
        ToolErrorCode::InvalidArgument
    );
    let doc: jian_ops_schema::PenDocument = serde_json::from_value(json!({
        "version": "1.0",
        "children": [
            {"type":"frame", "id":"card", "x":0, "y":0, "width":100, "height":100, "children":[
                {"type":"text", "id":"label", "x":10, "y":10, "content":"keep me"}
            ]},
            {"type":"rectangle", "id":"b", "x":50, "y":50, "width":100, "height":100}
        ]
    }))
    .expect("document");
    let state = EditorState::from_document(doc);
    assert_eq!(
        error_code(call(&state, r#"["card","b"]"#, "union")),
        ToolErrorCode::InvalidArgument
    );
}

#[test]
fn a_document_edited_after_the_snapshot_rejects_the_commit() {
    let mut state = state();
    let ToolOutcome::OkWithCommand(_, command) = call(&state, r#"["a","b"]"#, "union") else {
        panic!("expected a write");
    };
    assert!(state.apply(EditorCommand::UpdateNode {
        node_id: NodeId::new("b"),
        x: Some(300),
        y: None,
        width: None,
        height: None,
        name: None,
        fill_hex: None,
        page_id: None,
    }));
    let before = state.doc.clone();
    assert!(!state.apply(command));
    assert_eq!(state.doc, before);
}

#[test]
fn disjoint_intersect_fails_instead_of_writing() {
    let doc: jian_ops_schema::PenDocument = serde_json::from_value(json!({
        "version": "1.0",
        "children": [
            {"type":"rectangle", "id":"a", "x":0, "y":0, "width":10, "height":10},
            {"type":"rectangle", "id":"b", "x":100, "y":100, "width":10, "height":10}
        ]
    }))
    .expect("document");
    let state = EditorState::from_document(doc);
    assert_eq!(
        error_code(call(&state, r#"["a","b"]"#, "intersect")),
        ToolErrorCode::ToolFailed
    );
}
