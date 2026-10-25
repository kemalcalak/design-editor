use super::*;

fn tool() -> (EditorState, SetScreen) {
    let doc: jian_ops_schema::PenDocument = serde_json::from_str(
        r#"{"version":"1.0","children":[
        {"type":"frame","id":"home","name":"Home","screen":"/","width":1280,"height":900,
         "children":[{"type":"text","id":"t","content":"Hi"}]},
        {"type":"frame","id":"about","name":"About us","width":1280,"height":900}]}"#,
    )
    .unwrap();
    let state = EditorState::from_document(doc);
    let tool = set_screen_snapshot(&state);
    (state, tool)
}

fn call(tool: &SetScreen, args: &[(&str, &str)]) -> ToolOutcome {
    tool.call(
        &args
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
    )
}

fn screen(state: &EditorState, id: &str) -> Option<String> {
    state.active_children().iter().find_map(|node| match node {
        PenNode::Frame(f) if f.base.id == id => f.screen.clone(),
        _ => None,
    })
}

#[test]
fn start_moves_root_and_applies_as_one_batch() {
    let (mut state, tool) = tool();
    let ToolOutcome::OkWithCommand(_, command) =
        call(&tool, &[("nodeId", "about"), ("mode", "start")])
    else {
        panic!("expected a command");
    };
    assert!(matches!(command, EditorCommand::Batch { .. }));
    assert!(state.apply(command));
    assert_eq!(screen(&state, "about").as_deref(), Some("/"));
    assert_eq!(screen(&state, "home").as_deref(), Some("/home"));
}

#[test]
fn mark_is_the_default_and_unchanged_writes_nothing() {
    let (mut state, tool) = tool();
    let ToolOutcome::OkWithCommand(_, command) = call(&tool, &[("nodeId", "about")]) else {
        panic!("expected a command");
    };
    assert!(state.apply(command));
    assert_eq!(screen(&state, "about").as_deref(), Some("/about-us"));
    assert!(matches!(
        call(&tool, &[("nodeId", "home"), ("mode", "mark")]),
        ToolOutcome::Ok(out) if out["wrote"] == "false"
    ));
}

#[test]
fn bad_arguments_are_refused() {
    let (_, tool) = tool();
    for (args, code) in [
        (vec![], ToolErrorCode::MissingArgument),
        (vec![("nodeId", "t")], ToolErrorCode::InvalidArgument),
        (
            vec![("nodeId", "about"), ("mode", "pin")],
            ToolErrorCode::InvalidArgument,
        ),
    ] {
        assert!(
            matches!(call(&tool, &args), ToolOutcome::Err(c, _) if c == code),
            "{args:?}"
        );
    }
}
