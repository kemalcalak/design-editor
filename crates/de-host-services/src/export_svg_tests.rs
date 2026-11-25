//! The layered SVG export through the MCP tools (#407), from a document:
//! layer names reach the file as ids, and each tool hands back `warnings`.

use std::collections::BTreeMap;

use base64::Engine as _;
use de_editor_core::EditorState;
use de_mcp::{McpTool, ToolOutcome};
use serde_json::Value;

use crate::mcp_serve::execute_tool::execute_snapshot;
use crate::mcp_serve::export_frames_tool::export_frames_snapshot;
use crate::mcp_serve::export_item_tool::export_item_snapshot;
use crate::mcp_serve::export_tool::export_nodes_snapshot;

fn state() -> EditorState {
    let doc = jian_ops_schema::load_str(
        r##"{"version":"1.0.0","children":[
            {"type":"frame","id":"card","name":"Pricing card","x":0,"y":0,"width":240,"height":160,
             "layout":"vertical","gap":8,"padding":16,
             "fill":[{"type":"solid","color":"#ffffff"}],
             "effects":[{"type":"background_blur","radius":12}],
             "children":[
               {"type":"text","id":"title","name":"Title","content":"Team plan",
                "fontFamily":"Inter","fontSize":20,"fontWeight":"700",
                "fill":[{"type":"solid","color":"#111111"}]},
               {"type":"frame","id":"row","name":"Price row","layout":"horizontal","gap":4,
                "width":"fill_container","height":24,
                "children":[
                  {"type":"rectangle","id":"chip","name":"Chip","width":24,"height":24,
                   "fill":[{"type":"solid","color":"#3366ff"}]}
                ]}
             ]},
            {"type":"frame","id":"second","name":"Second board","x":400,"y":0,"width":100,"height":100,
             "fill":[{"type":"solid","color":"#eeeeee"}]}
        ]}"##,
    )
    .expect("fixture parses")
    .value;
    EditorState::from_document(doc)
}

fn json_of(outcome: ToolOutcome) -> Value {
    match outcome {
        ToolOutcome::OkJson(json) | ToolOutcome::OkJsonWithImages { json, .. } => {
            serde_json::from_str(&json).expect("tool json")
        }
        ToolOutcome::Err(code, message) => panic!("tool failed: {code:?} {message}"),
        _ => panic!("unexpected tool outcome"),
    }
}

fn args(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

fn decoded(value: &Value) -> String {
    let b64 = value["bytes_base64"].as_str().expect("bytes_base64");
    String::from_utf8(
        base64::engine::general_purpose::STANDARD
            .decode(b64)
            .expect("base64"),
    )
    .expect("utf-8 svg")
}

fn assert_layered(svg: &str) {
    assert!(svg.starts_with("<svg "), "{svg}");
    let card = svg
        .find(r#"<g id="Pricing card" data-name="Pricing card">"#)
        .unwrap_or_else(|| panic!("card group: {svg}"));
    let title = svg
        .find(r#"<text id="Title" data-name="Title""#)
        .unwrap_or_else(|| panic!("title text: {svg}"));
    let row = svg
        .find(r#"<g id="Price row" data-name="Price row">"#)
        .unwrap_or_else(|| panic!("row group: {svg}"));
    let chip = svg
        .find(r#"<rect id="Chip" data-name="Chip""#)
        .unwrap_or_else(|| panic!("chip rect: {svg}"));
    assert!(card < title && title < row && row < chip, "{svg}");
    assert!(svg.contains(r#"font-family="Inter""#), "{svg}");
    assert!(svg.contains(r#"font-weight="700""#), "{svg}");
    assert!(svg.contains(">Team plan</tspan>"), "{svg}");
}

fn has_background_blur_warning(warnings: &Value) -> bool {
    warnings
        .as_array()
        .expect("warnings array")
        .iter()
        .any(|w| {
            w.as_str()
                .is_some_and(|w| w.starts_with("Pricing card (card): background blur"))
        })
}

#[test]
fn export_nodes_writes_one_layered_svg_per_node() {
    let tool = export_nodes_snapshot(&state());
    let result = json_of(tool.call(&args(&[
        ("nodeIds", r#"["card","second","missing"]"#),
        ("format", "svg"),
    ])));
    let files = result["files"].as_array().expect("files");
    assert_eq!(files.len(), 3, "{result}");
    assert_eq!(files[0]["format"], "svg");
    assert_layered(&decoded(&files[0]));
    assert!(
        has_background_blur_warning(&files[0]["warnings"]),
        "{result}"
    );
    let second = decoded(&files[1]);
    assert!(second.contains(r#"id="Second board""#), "{second}");
    assert!(!second.contains("Pricing"), "one file per node: {second}");
    assert!(files[2]["error"].is_string(), "{result}");
}

#[test]
fn export_item_writes_a_node_or_a_whole_page_as_svg() {
    let tool = export_item_snapshot(&state());
    let node = json_of(tool.call(&args(&[("itemId", "card"), ("format", "svg")])));
    assert_eq!(node["itemType"], "node");
    assert_eq!(node["format"], "svg");
    assert_layered(&decoded(&node));
    assert!(has_background_blur_warning(&node["warnings"]), "{node}");

    let page_id = de_pen_loader::editor_state_to_layout_scene(&state()).pages[0]
        .id
        .clone();
    let page = json_of(tool.call(&args(&[("itemId", &page_id), ("format", "svg")])));
    assert_eq!(page["itemType"], "page");
    let svg = decoded(&page);
    assert_layered(&svg);
    assert!(svg.contains(r#"id="Second board""#), "{svg}");
}

fn temp_dir(tag: &str) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!(
        "de-svg-export-{tag}-{}-{nanos}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

#[test]
fn export_frames_writes_one_svg_file_per_frame() {
    let dir = temp_dir("frames");
    let tool = export_frames_snapshot(&state());
    let result = json_of(tool.call(&args(&[
        ("outputDir", dir.to_str().expect("utf-8 dir")),
        ("format", "svg"),
    ])));
    let written = result["written"].as_array().expect("written");
    assert_eq!(written.len(), 2, "{result}");
    let card_file = written
        .iter()
        .filter_map(Value::as_str)
        .find(|name| name.contains("Pricing card"))
        .unwrap_or_else(|| panic!("card file: {result}"));
    assert!(card_file.ends_with(".svg"), "{result}");
    assert_layered(&std::fs::read_to_string(dir.join(card_file)).expect("card svg"));
    let warnings = result["warnings"].as_array().expect("warnings");
    assert!(
        warnings
            .iter()
            .any(|w| w.as_str().is_some_and(|w| w.contains("background blur"))),
        "{result}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn execute_export_writes_svg_files_and_reports_their_warnings() {
    let dir = temp_dir("execute");
    let script = format!(
        r#"Export(["card"], "svg", {:?});"#,
        dir.to_str().expect("utf-8 dir")
    );
    let result = json_of(execute_snapshot(&state()).call(&args(&[("script", &script)])));
    let exported = result["exported"].as_array().expect("exported paths");
    assert_eq!(exported.len(), 1, "{result}");
    let path = exported[0].as_str().expect("path");
    assert!(path.ends_with("card.svg"), "{result}");
    assert_layered(&std::fs::read_to_string(path).expect("written svg"));
    let warnings = result["warnings"].as_array().expect("warnings");
    assert!(
        warnings.iter().any(|w| w.as_str().is_some_and(
            |w| w.starts_with("Export: card.svg: Pricing card (card): background blur")
        )),
        "{result}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
