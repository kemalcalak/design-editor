//! Imports are the user's content, not generated work: a file-backed
//! session saves them exactly as imported, through write repairs and
//! auto-finalize alike (#299).

use super::*;
use de_mcp::ToolOutcome;

const SNAPSHOT_FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../de-html/tests/fixtures/tailwind_app_snapshot.json"
);

fn temp_document(tag: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "design-editor-mcp-import-finalize-{tag}-{}-{}.de",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock")
            .as_nanos()
    ));
    std::fs::write(&path, r#"{"version":"1.0.0","children":[]}"#).expect("seed document");
    path
}

fn saved_children(path: &Path) -> serde_json::Value {
    let saved = load_editor_state(path).expect("saved document");
    serde_json::to_value(saved.active_children()).expect("serialize saved page")
}

/// The page the import's own command produces: applied as is, with no
/// repair or finalize, and round-tripped through a save like the session's.
fn raw_import(tag: &str, apply: impl FnOnce(&mut EditorState)) -> serde_json::Value {
    let path = temp_document(tag);
    let mut state = load_editor_state(&path).expect("load document");
    apply(&mut state);
    save_editor_state(&state, &path).expect("save reference");
    let page = saved_children(&path);
    let _ = std::fs::remove_file(&path);
    page
}

fn import_web_snapshot_line() -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {
            "name": "import_web_snapshot",
            "arguments": {"snapshotPath": SNAPSHOT_FIXTURE}
        }
    })
    .to_string()
}

#[test]
fn stdio_eof_saves_an_imported_web_snapshot_unchanged() {
    let imported = raw_import("reference", |state| {
        let response =
            process_message_with_applier(state, &import_web_snapshot_line(), |_, state, cmd| {
                state.apply(cmd.clone())
            })
            .expect("dispatch")
            .expect("response");
        assert!(!response.contains(r#""isError":true"#), "{response}");
    });

    // The same import in a file-backed stdio session that ends at EOF.
    let path = temp_document("session");
    let mut state = load_editor_state(&path).expect("load document");
    let mut input = std::io::Cursor::new(format!("{}\n", import_web_snapshot_line()).into_bytes());
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

    assert_eq!(
        saved_children(&path),
        imported,
        "the session must save the snapshot exactly as imported"
    );
    assert!(
        auto.last_finalized_revision().is_none(),
        "an import-only session has nothing to finalize"
    );
    let _ = std::fs::remove_file(&path);
}

#[test]
fn browser_import_to_canvas_lands_verbatim_in_both_layouts() {
    let snapshot = std::fs::read_to_string(SNAPSHOT_FIXTURE).expect("read fixture");
    for auto_layout in [false, true] {
        let ToolOutcome::OkWithCommand(_, command) = browser_tool_canvas::place_import(
            browser_tool_canvas::import_nodes(&snapshot, auto_layout),
            None,
        ) else {
            panic!("import-to-canvas produced no command (auto_layout={auto_layout})");
        };
        let imported = raw_import("reference", |state| {
            assert!(state.apply(command.clone()));
        });

        let path = temp_document(if auto_layout { "auto" } else { "absolute" });
        let mut state = load_editor_state(&path).expect("load document");
        let mut auto = auto_finalize::AutoFinalize::for_test(std::time::Duration::ZERO);
        assert!(matches!(
            apply_file_write(&mut state, &path, "browser", &command, Some(&mut auto)),
            FileWrite::Applied
        ));
        assert_eq!(
            saved_children(&path),
            imported,
            "the write must save the import as is (auto_layout={auto_layout})"
        );
        assert!(
            auto.run(&mut state, "stdin-eof").is_none(),
            "an import must not arm auto-finalize (auto_layout={auto_layout})"
        );
        let _ = std::fs::remove_file(&path);
    }
}

#[test]
fn every_import_tool_writes_verbatim() {
    let imports: Vec<String> = TOOL_SCHEMAS
        .iter()
        .filter_map(|schema| {
            let schema: serde_json::Value = serde_json::from_str(schema).ok()?;
            Some(schema["name"].as_str()?.to_string())
        })
        .filter(|name| name.starts_with("import_"))
        .collect();
    assert!(imports.len() >= 6, "{imports:?}");
    for tool in imports.iter().map(String::as_str).chain(["browser"]) {
        assert!(writes_verbatim(tool), "{tool}");
    }
    // Generated work still gets its repairs.
    for tool in ["batch_design", "insert_node", "design_skeleton"] {
        assert!(!writes_verbatim(tool), "{tool}");
    }
}
