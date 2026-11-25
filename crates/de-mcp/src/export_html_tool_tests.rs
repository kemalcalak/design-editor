use std::collections::BTreeMap;

use de_editor_core::EditorState;
use serde_json::Value;

use super::export_html_snapshot;
use crate::{McpTool, ToolErrorCode, ToolOutcome};

fn state() -> EditorState {
    let doc = jian_ops_schema::load_str(
        r##"{"version":"1.0.0","children":[
            {"type":"frame","id":"hero","name":"Hero","width":320,"height":120,"layout":"vertical",
             "children":[{"type":"text","id":"title","content":"Hello"}]}]}"##,
    )
    .expect("fixture")
    .value;
    EditorState::from_document(doc)
}

fn call(pairs: &[(&str, &str)]) -> ToolOutcome {
    let args: BTreeMap<String, String> = pairs
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect();
    export_html_snapshot(&state()).call(&args)
}

fn json(outcome: ToolOutcome) -> Value {
    match outcome {
        ToolOutcome::OkJson(text) => serde_json::from_str(&text).expect("json result"),
        other => panic!("expected OkJson, got {other:?}"),
    }
}

#[test]
fn without_an_output_path_the_html_comes_back_inline() {
    let out = json(call(&[("nodeIds", r#"["hero"]"#)]));
    assert_eq!(out["format"], "html-tailwind");
    assert_eq!(out["nodeCount"], 1);
    let html = out["html"].as_str().expect("html");
    assert!(
        html.contains("<!DOCTYPE html>") || html.contains("<!doctype html>"),
        "{html}"
    );
    assert!(html.contains("Hello"), "{html}");
    assert!(
        html.contains("data-name"),
        "layer names are on by default: {html}"
    );
}

#[test]
fn pen_dev_switches_shape_the_output() {
    let out = json(call(&[
        ("nodeIds", r#"["hero"]"#),
        ("format", "html-css"),
        ("includeHtmlScaffold", "false"),
        ("includeLayerNames", "false"),
    ]));
    let html = out["html"].as_str().expect("html");
    assert!(
        !html.to_ascii_lowercase().contains("<!doctype html>"),
        "{html}"
    );
    assert!(!html.contains("data-name"), "{html}");
}

#[test]
fn an_output_path_writes_the_file_and_names_it() {
    let dir = std::env::temp_dir().join(format!("de-export-html-{}", std::process::id()));
    let file = dir.join("out").join("hero.html");
    let out = json(call(&[
        ("nodeIds", r#"["hero"]"#),
        ("outputPath", file.to_str().expect("utf-8 path")),
    ]));
    assert_eq!(
        out["filePath"].as_str(),
        Some(de_util::display_path::display_path(&file).as_str())
    );
    let written = std::fs::read_to_string(&file).expect("written");
    assert!(written.contains("Hello"));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn bad_arguments_are_refused() {
    assert!(matches!(
        call(&[("outputPath", "relative/out.html")]),
        ToolOutcome::Err(ToolErrorCode::InvalidArgument, _)
    ));
    assert!(matches!(
        call(&[("format", "react")]),
        ToolOutcome::Err(ToolErrorCode::InvalidArgument, _)
    ));
    assert!(matches!(
        call(&[("nodeIds", r#"["missing"]"#)]),
        ToolOutcome::Err(ToolErrorCode::ToolFailed, _)
    ));
}
