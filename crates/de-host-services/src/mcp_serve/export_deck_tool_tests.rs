//! `export_deck` over MCP: the HyperFrames arm end to end through
//! `tools/call`, plus the argument errors every format shares.

use super::*;
use crate::export_hyperframes::RENDER_NOTES_FILE;
use de_editor_core::{EditorCommand, NodeFlag, NodeId};

/// Two white 320x180 boards, the second hidden when `hide_second`.
fn deck(hide_second: bool) -> EditorState {
    let mut state = EditorState::new();
    for (index, name) in ["Cover", "Agenda"].iter().enumerate() {
        state.apply(EditorCommand::InsertNode {
            kind: "frame".into(),
            name: (*name).into(),
            x: (index as i32) * 400,
            y: 0,
            width: 320,
            height: 180,
            fill_hex: Some("#ffffff".into()),
            target_parent: NodeId::NONE,
            page_id: None,
        });
    }
    if hide_second {
        let id = de_editor_core::preview_slideshow::active_page_boards(&state)[1].clone();
        assert!(state.apply(EditorCommand::SetNodeFlag {
            node_id: NodeId::new(&id),
            flag: NodeFlag::Hidden,
            value: true,
        }));
    }
    state
}

fn temp_dir(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!(
        "de-mcp-export-deck-{tag}-{}-{nanos}",
        std::process::id()
    ))
}

fn call(state: &EditorState, format: &str, output_path: &Path) -> ToolOutcome {
    let mut args = BTreeMap::new();
    args.insert("format".to_string(), format.to_string());
    args.insert("outputPath".to_string(), output_path.display().to_string());
    export_deck_snapshot(state).call(&args)
}

fn error_code(outcome: ToolOutcome) -> ToolErrorCode {
    match outcome {
        ToolOutcome::Err(code, _) => code,
        other => panic!("expected an error, got {other:?}"),
    }
}

#[test]
fn hyperframes_over_tools_call_writes_the_composition_into_the_directory() {
    let mut state = deck(false);
    let dir = temp_dir("tools-call");
    let line = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 7,
        "method": "tools/call",
        "params": {
            "name": "export_deck",
            "arguments": {"format": "hyperframes", "outputPath": dir.display().to_string()}
        }
    })
    .to_string();

    let response =
        crate::mcp_serve::process_message_with_applier(&mut state, &line, |_, _, _| false)
            .expect("dispatch")
            .expect("response");
    let result: serde_json::Value =
        serde_json::from_str(&crate::mcp_serve::tool_text(&response)).expect("result JSON");

    assert_eq!(result["format"], "hyperframes", "{result}");
    assert_eq!(result["path"], dir.display().to_string());
    assert_eq!(result["slides"], 2);
    assert_eq!(result["fps"], 30);
    assert_eq!(result["width"], 320.0);
    assert_eq!(result["height"], 180.0);
    // Two text-free boards hold the 3 s floor each.
    assert_eq!(result["durationSeconds"], 6.0);
    let composition = dir.join(COMPOSITION_FILE);
    assert_eq!(result["compositionPath"], composition.display().to_string());
    assert_eq!(
        result["renderNotesPath"],
        dir.join(RENDER_NOTES_FILE).display().to_string()
    );

    // The result is only a claim until the files are on disk.
    let html = std::fs::read_to_string(&composition).expect("composition on disk");
    assert!(html.contains("data-composition-id"), "{html}");
    assert_eq!(html.matches("class=\"clip hf-scene\"").count(), 2);
    assert!(dir.join(RENDER_NOTES_FILE).is_file());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn hyperframes_counts_only_the_scenes_it_emitted() {
    let state = deck(true);
    let dir = temp_dir("hidden");
    let ToolOutcome::OkJson(json) = call(&state, "hyperframes", &dir) else {
        panic!("expected a result");
    };
    let result: serde_json::Value = serde_json::from_str(&json).expect("result JSON");
    // A hidden board costs no time and is not a slide of the video.
    assert_eq!(result["slides"], 1, "{json}");
    assert_eq!(result["durationSeconds"], 3.0, "{json}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn hyperframes_refuses_an_html_file_path() {
    let state = deck(false);
    let dir = temp_dir("html-path");
    for name in ["deck.html", "deck.HTM"] {
        let target = dir.join(name);
        assert_eq!(
            error_code(call(&state, "hyperframes", &target)),
            ToolErrorCode::InvalidArgument,
            "{name}"
        );
        assert!(!target.exists(), "{name}: nothing is written");
    }
}

#[test]
fn hyperframes_reports_an_empty_page_as_a_failure() {
    let state = EditorState::new();
    let dir = temp_dir("empty");
    assert_eq!(
        error_code(call(&state, "hyperframes", &dir)),
        ToolErrorCode::Internal
    );
    assert!(!dir.join(COMPOSITION_FILE).exists());
}

#[test]
fn rejects_missing_and_unknown_arguments() {
    let state = deck(false);
    let tool = export_deck_snapshot(&state);
    assert_eq!(
        error_code(tool.call(&BTreeMap::new())),
        ToolErrorCode::MissingArgument
    );
    let mut args = BTreeMap::new();
    args.insert("format".to_string(), "hyperframes".to_string());
    assert_eq!(error_code(tool.call(&args)), ToolErrorCode::MissingArgument);
    assert_eq!(
        error_code(call(&state, "mp4", &temp_dir("unknown"))),
        ToolErrorCode::InvalidArgument
    );
}
