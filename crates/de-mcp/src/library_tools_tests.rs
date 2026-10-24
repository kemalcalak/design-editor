//! Tests for `library_tools.rs`.

use super::*;

fn args(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

fn temp_dir(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("de-mcp-lib-{tag}-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn write_library(dir: &Path) {
    let doc = json!({
        "version": "1.0",
        "children": [{
            "id": "kit-btn", "type": "frame", "name": "Button", "reusable": true,
            "width": 80, "height": 32,
        }],
    });
    std::fs::write(dir.join("kit.de"), doc.to_string()).unwrap();
}

fn apply(state: &mut EditorState, outcome: ToolOutcome) -> String {
    match outcome {
        ToolOutcome::OkJsonWithCommand(json, command) => {
            assert!(state.apply(command));
            json
        }
        ToolOutcome::OkWithCommand(out, command) => {
            assert!(state.apply(command));
            format!("{out:?}")
        }
        ToolOutcome::OkJson(json) => json,
        other => panic!("unexpected outcome: {other:?}"),
    }
}

#[test]
fn add_list_and_remove_a_library() {
    let dir = temp_dir("roundtrip");
    write_library(&dir);
    let mut state = EditorState::new();
    state.editor_ui.libraries.base_dir = Some(dir.to_string_lossy().into_owned());

    let outcome = add_library_snapshot(&state).call(&args(&[("path", "kit.de"), ("alias", "ui")]));
    let added = apply(&mut state, outcome);
    assert!(added.contains(r#""path":"kit.de""#), "{added}");
    assert!(state
        .components
        .find_by_id(&de_editor_core::NodeId::new("kit-btn"))
        .is_some());

    let outcome = list_libraries_snapshot(&state).call(&args(&[]));
    let listed = apply(&mut state, outcome);
    let listed: Value = serde_json::from_str(&listed).unwrap();
    assert_eq!(listed["count"], 1);
    assert_eq!(listed["libraries"][0]["name"], "ui");
    assert_eq!(listed["libraries"][0]["status"], "synced");
    assert_eq!(listed["libraries"][0]["components"], 1);

    let outcome = remove_library_snapshot(&state).call(&args(&[("path", "ui")]));
    apply(&mut state, outcome);
    assert!(state.editor_ui.libraries.entries.is_empty());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn adding_a_missing_file_fails_without_linking() {
    let dir = temp_dir("missing");
    let mut state = EditorState::new();
    state.editor_ui.libraries.base_dir = Some(dir.to_string_lossy().into_owned());
    match add_library_snapshot(&state).call(&args(&[("path", "nope.de")])) {
        ToolOutcome::Err(ToolErrorCode::ToolFailed, message) => {
            assert!(message.contains("not found"), "{message}")
        }
        other => panic!("expected a failure, got {other:?}"),
    }
    assert!(state.editor_ui.libraries.entries.is_empty());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn removing_an_unknown_library_is_an_argument_error() {
    let state = EditorState::new();
    assert!(matches!(
        remove_library_snapshot(&state).call(&args(&[("path", "x.de")])),
        ToolOutcome::Err(ToolErrorCode::InvalidArgument, _)
    ));
}
