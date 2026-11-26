use super::*;
use serde_json::json;

fn state() -> EditorState {
    let doc = json!({"version": "1.0.0", "children": [
        {"type": "frame", "id": "screen", "name": "Frame 1", "width": 1440, "height": 900,
         "layout": "vertical", "children": [
            {"type": "frame", "id": "cta", "name": "Rectangle 3", "width": 160, "height": 48,
             "fill": [{"type": "solid", "color": "#2563EB"}],
             "children": [{"type": "text", "id": "cta-label", "name": "Text", "content": "Start", "fontSize": 16}]},
            {"type": "frame", "id": "kept", "name": "Testimonials", "width": 1440, "height": 300}
         ]}
    ]});
    EditorState::from_document(
        jian_ops_schema::load_str(&doc.to_string())
            .expect("fixture")
            .value,
    )
}

fn call(state: &EditorState, args: &[(&str, &str)]) -> ToolOutcome {
    let args = args
        .iter()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect();
    rename_layers_snapshot(state).call(&args)
}

fn json_of(outcome: &ToolOutcome) -> Value {
    match outcome {
        ToolOutcome::OkJson(text) | ToolOutcome::OkJsonWithCommand(text, _) => {
            serde_json::from_str(text).expect("json")
        }
        _ => panic!("expected a JSON result"),
    }
}

fn name_of(state: &EditorState, id: &str) -> Option<String> {
    fn find<'a>(nodes: &'a [PenNode], id: &str) -> Option<&'a PenNode> {
        nodes.iter().find_map(|node| {
            (node.id_str() == id)
                .then_some(node)
                .or_else(|| find(node.children().map_or(&[][..], Vec::as_slice), id))
        })
    }
    find(state.active_children(), id).and_then(|node| node.base().name.clone())
}

#[test]
fn a_rules_dry_run_reports_without_a_command() {
    let state = state();
    let outcome = call(&state, &[("dryRun", "true")]);
    assert!(matches!(outcome, ToolOutcome::OkJson(_)));
    let result = json_of(&outcome);
    assert_eq!(result["mode"], "rules");
    assert_eq!(result["dryRun"], true);
    let renamed = result["renamed"].as_array().expect("renamed");
    assert!(renamed.iter().any(|r| r["id"] == "cta"
        && r["current"] == "Rectangle 3"
        && r["proposed"] == "Primary button"));
    assert!(renamed
        .iter()
        .any(|r| r["id"] == "cta-label" && r["proposed"] == "Start"));
    assert!(!renamed.iter().any(|r| r["id"] == "kept"));
}

#[test]
fn rules_apply_as_one_undo_step() {
    let mut state = state();
    let ToolOutcome::OkJsonWithCommand(_, command) = call(&state, &[("nodeIds", "[\"cta\"]")])
    else {
        panic!("a rename carries the command");
    };
    assert!(state.apply(command));
    assert_eq!(name_of(&state, "cta").as_deref(), Some("Primary button"));
    assert_eq!(
        name_of(&state, "screen").as_deref(),
        Some("Frame 1"),
        "outside nodeIds"
    );
    assert!(state.undo());
    assert_eq!(name_of(&state, "cta").as_deref(), Some("Rectangle 3"));
    assert_eq!(name_of(&state, "cta-label").as_deref(), Some("Text"));
}

#[test]
fn ai_mode_briefs_the_caller_then_applies_its_names() {
    let mut state = state();
    let brief = json_of(&call(&state, &[("mode", "ai"), ("nodeIds", "cta")]));
    assert_eq!(brief["dryRun"], true);
    assert_eq!(brief["total"], brief["count"]);
    assert_eq!(brief["truncated"], false);
    assert_eq!(brief["language"], "English (en-US)");
    assert_eq!(brief["layers"][0]["id"], "cta");
    assert_eq!(brief["layers"][0]["suggested"], "Primary button");
    assert!(brief["next"].as_str().unwrap().contains("names"));

    let names = r#"{"cta": "Start button", "cta-label": "Frame 2"}"#;
    let outcome = call(&state, &[("mode", "ai"), ("names", names)]);
    let result = json_of(&outcome);
    assert_eq!(
        result["skipped"][0]["id"], "cta-label",
        "a default name is refused"
    );
    let ToolOutcome::OkJsonWithCommand(_, command) = outcome else {
        panic!("names apply");
    };
    assert!(state.apply(command));
    assert_eq!(name_of(&state, "cta").as_deref(), Some("Start button"));
}

#[test]
fn bad_arguments_are_rejected() {
    let state = state();
    for args in [
        vec![("mode", "magic")],
        vec![("dryRun", "maybe")],
        vec![("nodeIds", "nope")],
        vec![("mode", "ai"), ("names", "[1]")],
        vec![("mode", "ai"), ("names", r#"{"ghost": "Hero"}"#)],
        vec![("names", r#"{"cta": "Hero"}"#)],
    ] {
        assert!(
            matches!(
                call(&state, &args),
                ToolOutcome::Err(ToolErrorCode::InvalidArgument, _)
            ),
            "{args:?}"
        );
    }
}

/// Over the wire `nodeIds` and `names` arrive as a JSON array and object,
/// which the parser used to refuse, so ai mode could never apply (#430).
#[test]
fn ai_names_apply_through_the_wire_parser() {
    let mut state = state();
    let line = r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"rename_layers","arguments":{"mode":"ai","nodeIds":["cta"],"names":{"cta":"Start button"}}}}"#;
    let call = crate::parse_tool_call(line).expect("structured arguments parse");
    assert_eq!(call.arguments["nodeIds"], r#"["cta"]"#);
    let ToolOutcome::OkJsonWithCommand(_, command) =
        rename_layers_snapshot(&state).call(&call.arguments)
    else {
        panic!("names apply");
    };
    assert!(state.apply(command));
    assert_eq!(name_of(&state, "cta").as_deref(), Some("Start button"));
}
