use super::*;
use de_editor_core::walkers::find_node;

/// A card with a title and a free badge, and a separate square.
fn state() -> EditorState {
    let doc: jian_ops_schema::PenDocument = serde_json::from_value(json!({
        "version": "1.0",
        "variables": {"space": {"type": "number", "value": 8}},
        "children": [
            {"type": "frame", "id": "card", "x": 100, "y": 100, "width": 200, "height": 100,
             "gap": "$space", "padding": 10, "cornerRadius": 8,
             "children": [
                {"type": "text", "id": "title", "x": 10, "y": 10, "content": "Hi", "fontSize": 16}
             ]},
            {"type": "rectangle", "id": "sq", "x": 400, "y": 0, "width": 50, "height": 50}
        ]
    }))
    .expect("document");
    EditorState::from_document(doc)
}

fn call(state: &EditorState, args: &[(&str, &str)]) -> ToolOutcome {
    let args: BTreeMap<String, String> = args
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect();
    scale_nodes_snapshot(state).call(&args)
}

fn written(outcome: ToolOutcome) -> (Value, EditorCommand) {
    match outcome {
        ToolOutcome::OkJsonWithCommand(json, command) => {
            (serde_json::from_str(&json).expect("json"), command)
        }
        other => panic!("expected a write, got {other:?}"),
    }
}

fn error(outcome: ToolOutcome) -> (ToolErrorCode, String) {
    match outcome {
        ToolOutcome::Err(code, message) => (code, message),
        other => panic!("expected an error, got {other:?}"),
    }
}

fn field(state: &EditorState, id: &str, key: &str) -> Value {
    let node = find_node(state.active_children(), &NodeId::new(id)).expect("node");
    serde_json::to_value(node).expect("json")[key].clone()
}

#[test]
fn factor_scales_about_the_anchor_as_one_undo_step() {
    let mut s = state();
    let (result, command) = written(call(
        &s,
        &[
            ("nodeIds", r#"["card"]"#),
            ("factor", "2"),
            ("anchor", "center"),
        ],
    ));
    assert_eq!(
        result["bounds"],
        json!({"x": 0.0, "y": 50.0, "width": 400.0, "height": 200.0})
    );
    let warnings = result["warnings"].as_array().expect("warnings");
    assert_eq!(warnings.len(), 1, "the bound gap is listed: {warnings:?}");
    assert!(warnings[0].as_str().unwrap().contains("gap"));
    let depth = s.history.past.len();
    assert!(s.apply(command));
    assert_eq!(s.history.past.len(), depth + 1);
    assert_eq!(field(&s, "card", "x"), json!(0.0));
    assert_eq!(field(&s, "card", "width"), json!(400.0));
    assert_eq!(field(&s, "card", "gap"), json!("$space"));
    assert_eq!(field(&s, "title", "fontSize"), json!(32.0));
    assert_eq!(field(&s, "title", "x"), json!(20.0));
    assert!(s.undo());
    assert_eq!(field(&s, "card", "width"), json!(200.0));
}

#[test]
fn a_target_width_sets_the_factor_and_detach_unbinds() {
    let mut s = state();
    let (result, command) = written(call(
        &s,
        &[
            ("nodeIds", "card, sq"),
            ("width", "175"),
            ("variables", "detach"),
        ],
    ));
    // The union is 100..450 wide (350): half size.
    assert_eq!(result["factor"], json!(0.5));
    assert!(s.apply(command));
    assert_eq!(field(&s, "card", "gap"), json!(4.0));
    assert_eq!(field(&s, "sq", "x"), json!(250.0));
    assert_eq!(field(&s, "sq", "width"), json!(25.0));
}

#[test]
fn bad_arguments_are_refused() {
    let s = state();
    let cases: Vec<(Vec<(&str, &str)>, &str)> = vec![
        (vec![("factor", "2")], "nodeIds"),
        (vec![("nodeIds", "card")], "exactly one"),
        (
            vec![("nodeIds", "card"), ("factor", "2"), ("width", "10")],
            "exactly one",
        ),
        (vec![("nodeIds", "card"), ("factor", "1")], "unchanged"),
        (vec![("nodeIds", "card"), ("factor", "0")], "between"),
        (vec![("nodeIds", "card"), ("factor", "x")], "number"),
        (vec![("nodeIds", "card"), ("height", "-4")], "positive"),
        (vec![("nodeIds", "nope"), ("factor", "2")], "active page"),
        (vec![("nodeIds", "card, card"), ("factor", "2")], "twice"),
        (
            vec![("nodeIds", "card"), ("factor", "2"), ("anchor", "middle")],
            "anchor",
        ),
        (
            vec![("nodeIds", "card"), ("factor", "2"), ("variables", "x")],
            "keep or detach",
        ),
    ];
    for (args, needle) in cases {
        let (_, message) = error(call(&s, &args));
        assert!(message.contains(needle), "{args:?}: {message}");
    }
}
