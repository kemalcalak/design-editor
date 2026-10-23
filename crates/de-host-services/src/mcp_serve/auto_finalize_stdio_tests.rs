use super::*;
use de_editor_core::PenNodeExt;

#[test]
fn file_mcp_applier_materializes_empty_image_fill_before_save() {
    let path = std::env::temp_dir().join(format!(
        "design-editor-mcp-image-slot-{}-{}.op",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock")
            .as_nanos()
    ));
    std::fs::write(&path, r#"{"version":"1.0.0","children":[]}"#).expect("seed document");
    let mut state = load_editor_state(&path).expect("load document");
    let operations = r##"I(null,{
        "type":"frame",
        "name":"Gallery",
        "width":390,
        "height":844,
        "layout":"vertical",
        "children":[{
            "type":"rectangle",
            "name":"Album Cover",
            "width":240,
            "height":160,
            "fill":[{"type":"image","url":""}]
        }]
    })"##;
    let line = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {"name": "batch_design", "arguments": {"operations": operations}}
    })
    .to_string();

    let response = process_message(&mut state, &path, &line)
        .expect("dispatch")
        .expect("response");
    assert!(!response.contains(r#""isError":true"#), "{response}");

    let saved = load_editor_state(&path).expect("saved document");
    let root = &saved.active_children()[0];
    let slot = root
        .children()
        .expect("gallery children")
        .iter()
        .find(|node| node.base().name.as_deref() == Some("Album Cover"))
        .expect("image slot");
    assert!(
        matches!(slot, jian_ops_schema::node::PenNode::Image(_)),
        "{slot:?}"
    );
    let _ = std::fs::remove_file(&path);
}

#[test]
fn stdio_eof_auto_finalizes_only_the_screen_the_session_created() {
    let path = std::env::temp_dir().join(format!(
        "design-editor-mcp-stdio-finalize-{}-{}.op",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock")
            .as_nanos()
    ));
    // A hand-made screen whose bottom nav is not the last child: finalize
    // would anchor it, but the session never made this screen (#107).
    std::fs::write(
        &path,
        r##"{
          "version":"1.0.0",
          "children":[{
            "type":"frame", "id":"screen", "name":"Screen",
            "width":390, "height":844, "layout":"vertical",
            "children":[
              {"type":"frame", "id":"status", "name":"Status Bar", "role":"status-bar", "width":"fill_container", "height":62},
              {"type":"frame", "id":"nav", "name":"Bottom Tab Bar", "role":"bottom-tab-bar", "width":"fill_container", "height":72, "layout":"horizontal"},
              {"type":"frame", "id":"content", "name":"Content", "width":"fill_container", "height":200}
            ]
          }]
        }"##,
    )
    .expect("seed document");
    // Round-trip once so the comparison starts from the saved shape (the
    // writer spells empty containers as `"children": []`).
    let mut state = load_editor_state(&path).expect("load document");
    save_editor_state(&state, &path).expect("save seed");
    let authored_before =
        serde_json::to_value(&state.active_children()[0]).expect("serialize authored screen");
    // The session builds a second screen with the same misplaced nav.
    let operations = r##"I(null,{"type":"frame","name":"Generated","x":600,"y":0,"width":390,"height":844,"layout":"vertical","children":[{"type":"frame","name":"Status Bar","role":"status-bar","width":"fill_container","height":62},{"type":"frame","name":"Bottom Tab Bar","role":"bottom-tab-bar","width":"fill_container","height":72,"layout":"horizontal"},{"type":"frame","name":"Content","width":"fill_container","height":200}]})"##;
    let write = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/call",
        "params": {"name": "batch_design", "arguments": {"operations": operations}}
    })
    .to_string();
    let mut input = std::io::Cursor::new(format!("{write}\n").into_bytes());
    let mut output = Vec::new();
    let mut auto = auto_finalize::AutoFinalize::for_test(std::time::Duration::ZERO);
    let shutdown = std::sync::atomic::AtomicBool::new(false);
    auto_finalize::run_stdio_session(
        &mut input,
        &mut output,
        &mut state,
        &path,
        &mut auto,
        &shutdown,
    )
    .expect("stdio session");

    let saved = load_editor_state(&path).expect("saved finalized document");
    let authored = saved
        .active_children()
        .iter()
        .find(|node| node.id_str() == "screen")
        .expect("authored screen");
    assert_eq!(
        serde_json::to_value(authored).expect("serialize authored screen"),
        authored_before,
        "auto-finalize must leave the hand-made screen as it was"
    );
    let generated = saved
        .active_children()
        .iter()
        .find(|node| node.base().name.as_deref() == Some("Generated"))
        .and_then(|screen| screen.children())
        .expect("generated screen children");
    assert_eq!(
        generated
            .last()
            .and_then(|node| node.base().role.as_deref()),
        Some("bottom-tab-bar"),
        "EOF finalize must anchor the generated screen's bottom nav as the last child"
    );
    assert!(
        !auto.due(std::time::Instant::now(), state.document_revision()),
        "EOF finalize must record the finalized revision"
    );
    // Finalize is one ordinary undo step over the whole page: undoing it
    // keeps the hand-made screen.
    assert!(state.apply(de_editor_core::EditorCommand::Undo));
    assert!(state
        .active_children()
        .iter()
        .any(|node| node.id_str() == "screen"));
    assert!(String::from_utf8_lossy(&output).contains(r#""id":3"#));
    // The reloaded document starts a fresh in-memory revision counter, so the
    // proof that EOF finalize ran is the recorded revision on the session
    // state (plus the structural effect asserted above).
    assert!(
        auto.last_finalized_revision().is_some(),
        "EOF finalize must record the revision it ran against"
    );
    let _ = std::fs::remove_file(&path);
}

#[test]
fn explicit_finalize_design_marks_the_current_revision_finalized() {
    let path = std::env::temp_dir().join(format!(
        "design-editor-mcp-explicit-finalize-{}-{}.op",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock")
            .as_nanos()
    ));
    std::fs::write(
        &path,
        r##"{"version":"1.0.0","children":[{
          "type":"frame","id":"root","name":"Landing",
          "width":900,"height":600,"layout":"vertical",
          "children":[{"type":"rectangle","id":"photo","name":"Photo",
            "width":240,"height":160,"fill":[{"type":"image","url":""}]}]
        }]}"##,
    )
    .expect("seed document");
    let mut state = load_editor_state(&path).expect("load document");
    let mut auto = auto_finalize::AutoFinalize::for_test(std::time::Duration::ZERO);
    auto.note_write(
        std::time::Instant::now(),
        &WriteScope {
            new_roots: vec!["root".to_string()],
            ..WriteScope::default()
        },
    );
    let line = r#"{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"finalize_design","arguments":{}}}"#;

    let response = process_message_with_auto_finalize(&mut state, &path, line, Some(&mut auto))
        .expect("dispatch")
        .expect("response");
    assert!(!response.contains(r#""isError":true"#), "{response}");
    assert!(
        !auto.due(std::time::Instant::now(), state.document_revision()),
        "explicit finalize must suppress a duplicate idle finalize"
    );
    let _ = std::fs::remove_file(&path);
}
