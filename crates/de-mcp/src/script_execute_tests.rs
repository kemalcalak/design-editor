//! Tests for the `execute` script tool: reads see the writes before them,
//! the whole script is one undo step, and any failure applies nothing.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use de_editor_core::{EditorCommand, EditorState, PenNodeExt};
use serde_json::Value;

use super::{
    execute_snapshot, execute_snapshot_with_renderer, ExportFormat, ExportRequest,
    ScriptRenderError, ScriptRenderer,
};
use crate::test_fixtures::{frame, rect, sample, state_with};
use crate::{ImageContent, McpTool, ToolErrorCode, ToolOutcome};

struct Ran {
    result: Value,
    command: Option<EditorCommand>,
    images: Vec<ImageContent>,
}

fn args(script: &str) -> BTreeMap<String, String> {
    let mut args = BTreeMap::new();
    args.insert("script".to_string(), script.to_string());
    args
}

fn unwrap_ok(outcome: ToolOutcome) -> Ran {
    match outcome {
        ToolOutcome::OkJsonWithImages {
            json,
            images,
            command,
        } => Ran {
            result: serde_json::from_str(&json).expect("result json"),
            command,
            images,
        },
        other => panic!("expected an execute result, got {other:?}"),
    }
}

fn run_ok(state: &EditorState, script: &str) -> Ran {
    unwrap_ok(execute_snapshot(state).call(&args(script)))
}

fn run_err(state: &EditorState, script: &str) -> (ToolErrorCode, String) {
    match execute_snapshot(state).call(&args(script)) {
        ToolOutcome::Err(code, message) => (code, message),
        other => panic!("expected an error, got {other:?}"),
    }
}

fn printed(ran: &Ran) -> Vec<String> {
    ran.result["printed"]
        .as_array()
        .expect("printed")
        .iter()
        .map(|line| line.as_str().expect("line").to_string())
        .collect()
}

fn empty() -> EditorState {
    state_with(Vec::new())
}

#[test]
fn a_read_sees_the_write_before_it_and_the_ids_land_as_reported() {
    let mut state = empty();
    let ran = run_ok(
        &state,
        r#"
const card = Insert(document, {type: "frame", name: "Card", x: 40, y: 40, width: 320, height: 200});
const title = Insert(card, {type: "text", name: "Title", content: "Hello"});
const read = Get(card, {depth: 1});
Print(read.name, read.children.length, read.children[0].id === title);
Update(title, {content: "Hi"});
Print(Get(title).content);
"#,
    );
    assert_eq!(printed(&ran), vec!["Card 1 true", "Hi"]);
    let created = ran.result["created"].as_array().expect("created");
    assert_eq!(created.len(), 2);
    assert_eq!(created[0]["name"], "Card");
    let card_id = created[0]["id"].as_str().expect("id").to_string();
    assert!(state.apply(ran.command.expect("writes ship a command")));
    let card = &state.active_children()[0];
    assert_eq!(card.id_str(), card_id, "the reported id is the landed id");
    assert_eq!(card.base().name.as_deref(), Some("Card"));
}

#[test]
fn the_whole_script_is_one_undo_step() {
    let mut state = sample();
    let before = serde_json::to_value(state.active_children()).expect("doc");
    let ran = run_ok(
        &state,
        r#"
const a = Insert(document, {type: "frame", name: "A", x: 600, y: 0, width: 100, height: 100});
Insert(a, {type: "rectangle", name: "R", width: 10, height: 10});
Update("n11", {content: "Changed"});
Move("n13", a);
Delete("n14");
"#,
    );
    let command = ran.command.expect("command");
    assert!(matches!(command, EditorCommand::Batch { .. }));
    assert!(state.apply(command));
    assert_ne!(
        serde_json::to_value(state.active_children()).expect("doc"),
        before
    );
    assert!(state.undo());
    assert_eq!(
        serde_json::to_value(state.active_children()).expect("doc"),
        before,
        "one undo reverts every write of the script"
    );
}

#[test]
fn a_throw_applies_nothing_and_drops_the_output() {
    let state = sample();
    let (code, message) = run_err(
        &state,
        r#"
Insert(document, {type: "frame", name: "Kept?", width: 10, height: 10});
Print("lost");
throw new Error("stop here");
"#,
    );
    assert_eq!(code, ToolErrorCode::ToolFailed);
    assert!(message.contains("nothing was applied"), "{message}");
    assert!(message.contains("stop here"), "{message}");
}

#[test]
fn a_caught_write_failure_still_rolls_back() {
    let state = sample();
    let (_, message) = run_err(
        &state,
        r#"
Insert(document, {type: "frame", name: "A", width: 10, height: 10});
try { Update("missing-node", {name: "x"}); } catch (e) { Print("caught", e.message); }
"#,
    );
    assert!(message.contains("a write failed"), "{message}");
    assert!(message.contains("missing-node"), "{message}");
}

#[test]
fn a_caught_read_failure_does_not_fail_the_run() {
    let state = sample();
    let ran = run_ok(
        &state,
        r#"try { Get("missing-node"); } catch (e) { Print("no node"); }"#,
    );
    assert_eq!(printed(&ran), vec!["no node"]);
    assert!(ran.command.is_none(), "a read-only script ships no command");
    assert_eq!(ran.result["applied"], false);
}

#[test]
fn a_runaway_script_times_out() {
    let outcome = super::run(
        &empty(),
        "while (true) {}",
        None,
        None,
        std::time::Duration::from_millis(200),
    );
    let Err(error) = outcome else {
        panic!("expected a timeout");
    };
    assert!(error.to_string().contains("longer than 200 ms"), "{error}");
}

#[test]
fn visitor_reports_resolved_bounds_depth_index_and_parents() {
    let state = empty();
    let ran = run_ok(
        &state,
        r#"
const col = Insert(document, {type: "frame", name: "Col", x: 100, y: 50, width: 200, height: 300,
  layout: "vertical", gap: 10, padding: 20});
Insert(col, {type: "rectangle", name: "First", width: 50, height: 40});
Insert(col, {type: "rectangle", name: "Second", width: 50, height: 30});
Get(col, (n, c) => Print(n.name, c.depth, c.index, c.bounds.x, c.bounds.y, c.bounds.width,
  c.parentCtx ? c.parentCtx.node.name : "-"));
"#,
    );
    assert_eq!(
        printed(&ran),
        vec![
            "Col 0 0 100 50 200 -",
            "First 1 0 20 20 50 Col",
            "Second 1 1 20 70 50 Col",
        ],
        "children flow down the column; bounds are parent-relative"
    );
}

#[test]
fn visitor_flags_clipped_children_and_can_skip_subtrees() {
    let state = empty();
    let ran = run_ok(
        &state,
        r#"
const box = Insert(document, {type: "frame", name: "Box", x: 0, y: 0, width: 100, height: 100, clipContent: true});
Insert(box, {type: "rectangle", name: "Inside", x: 10, y: 10, width: 20, height: 20});
Insert(box, {type: "rectangle", name: "Edge", x: 80, y: 10, width: 50, height: 20});
Insert(box, {type: "rectangle", name: "Out", x: 300, y: 10, width: 20, height: 20});
Get((n, c) => c.problems && Print(n.name, c.problems));
const seen = Get((n, c) => { c.skipChildren(); return n.name; });
Print(seen.join(","));
"#,
    );
    assert_eq!(
        printed(&ran),
        vec!["Edge partially clipped", "Out fully clipped", "Box"]
    );
}

#[test]
fn visitor_can_update_what_it_matches() {
    let mut state = sample();
    let ran = run_ok(
        &state,
        r#"Get(n => n.type === "text" && Update(n.id, {fontSize: 30}));"#,
    );
    assert!(state.apply(ran.command.expect("updates ship")));
    let mut sizes = Vec::new();
    fn walk(nodes: &[jian_ops_schema::node::PenNode], out: &mut Vec<String>) {
        for node in nodes {
            if let jian_ops_schema::node::PenNode::Text(text) = node {
                out.push(format!("{:?}", text.font_size));
            }
            if let Some(children) = node.children() {
                walk(children, out);
            }
        }
    }
    walk(state.active_children(), &mut sizes);
    assert!(!sizes.is_empty());
    assert!(sizes.iter().all(|s| s.contains("30")), "{sizes:?}");
}

#[test]
fn copy_replace_and_move_return_and_use_real_ids() {
    let mut state = state_with(vec![frame(
        "f1",
        "Screen",
        0.0,
        0.0,
        400.0,
        400.0,
        vec![
            rect("r1", "A", 0.0, 0.0, 10.0, 10.0),
            rect("r2", "B", 20.0, 0.0, 10.0, 10.0),
        ],
    )]);
    let ran = run_ok(
        &state,
        r#"
const copy = Copy("f1", document, {name: "Screen 2", x: 500});
const kids = Get(copy, {depth: 1}).children;
Print(kids.length, kids[0].id !== "r1");
const swapped = Replace("r2", {type: "ellipse", name: "Dot", width: 8, height: 8});
Move(swapped, undefined, 0);
Print(Get("f1", {depth: 1}).children.map(k => k.name).join(","));
"#,
    );
    assert_eq!(printed(&ran), vec!["2 true", "Dot,A"]);
    assert!(state.apply(ran.command.expect("command")));
    assert_eq!(state.active_children().len(), 2);
}

#[test]
fn delete_of_a_missing_node_throws() {
    let (_, message) = run_err(&sample(), r#"Delete("nope")"#);
    assert!(
        message.contains("Delete: node not found: nope"),
        "{message}"
    );
}

#[test]
fn set_variables_registers_theme_values_and_reads_back() {
    let mut state = empty();
    let ran = run_ok(
        &state,
        r##"
SetVariables({
  accent: {type: "color", value: "#A3B59A"},
  bg: {type: "color", value: [{value: "#FFFFFF", theme: {mode: "light"}}, {value: "#000000", theme: {mode: "dark"}}]}
});
const v = GetVariables();
Print(Object.keys(v.variables).sort().join(","), v.themes.mode.join(","));
"##,
    );
    assert_eq!(printed(&ran), vec!["accent,bg light,dark"]);
    assert!(state.apply(ran.command.expect("command")));
    assert_eq!(
        state
            .doc
            .themes
            .as_ref()
            .and_then(|t| t.get("mode"))
            .cloned(),
        Some(vec!["light".to_string(), "dark".to_string()])
    );
    let (_, message) = run_err(
        &state,
        r#"SetVariables({"$bad": {type: "number", value: 1}})"#,
    );
    assert!(message.contains("must not start with $"), "{message}");
}

#[test]
fn find_empty_space_sees_inserts_from_the_same_script() {
    let state = empty();
    let ran = run_ok(
        &state,
        r#"
Insert(document, {type: "frame", name: "A", x: 0, y: 0, width: 300, height: 200});
Print(FindEmptySpace({width: 100, height: 100, padding: 40}));
"#,
    );
    assert_eq!(printed(&ran), vec![r#"{"x":340,"y":0}"#]);
}

#[test]
fn print_formats_like_pen_dev_and_globals_need_no_declaration() {
    let ran = run_ok(
        &empty(),
        r#"
total = 2;
Print("rows", total, {a: [1, "b"]}, null, undefined);
console.log("via console");
"#,
    );
    assert_eq!(
        printed(&ran),
        vec![r#"rows 2 {"a":[1,"b"]} null undefined"#, "via console"]
    );
}

#[test]
fn get_without_a_visitor_needs_a_node() {
    let (_, message) = run_err(&sample(), "Get(document)");
    assert!(message.contains("needs a node id"), "{message}");
    let (_, message) = run_err(&sample(), r#"Insert(Get("n10"), {type: "text"})"#);
    assert!(message.contains("must be an id string"), "{message}");
}

#[test]
fn screenshots_need_a_renderer() {
    let (_, message) = run_err(&sample(), r#"TakeScreenshot(["n10"])"#);
    assert!(message.contains("needs a host that renders"), "{message}");
}

#[derive(Default)]
struct FakeRenderer {
    shots: Mutex<Vec<(Option<String>, usize)>>,
    exports: Mutex<Vec<ExportRequest>>,
}

impl ScriptRenderer for FakeRenderer {
    fn screenshot(
        &self,
        state: &EditorState,
        node_id: Option<&str>,
    ) -> Result<ImageContent, ScriptRenderError> {
        let roots = state.active_children().len();
        self.shots
            .lock()
            .expect("lock")
            .push((node_id.map(str::to_string), roots));
        Ok(ImageContent {
            data: format!("png-{}", node_id.unwrap_or("document")),
            mime_type: "image/png".into(),
        })
    }

    fn export(
        &self,
        _state: &EditorState,
        request: &ExportRequest,
    ) -> Result<Vec<String>, ScriptRenderError> {
        self.exports.lock().expect("lock").push(request.clone());
        Ok(request
            .node_ids
            .iter()
            .map(|id| format!("{}/{id}.png", request.output_path))
            .collect())
    }
}

#[test]
fn screenshots_show_the_state_at_the_call_and_ride_with_the_command() {
    let renderer = Arc::new(FakeRenderer::default());
    let state = sample();
    let tool = execute_snapshot_with_renderer(&state, renderer.clone());
    let ran = unwrap_ok(tool.call(&args(
        r#"
TakeScreenshot("document");
const a = Insert(document, {type: "frame", name: "New", x: 900, y: 0, width: 50, height: 50});
TakeScreenshot([a]);
Export([a], "png", "/tmp/out", {scale: 1});
"#,
    )));
    assert_eq!(ran.images.len(), 2);
    assert!(ran.command.is_some());
    let shots = renderer.shots.lock().expect("lock").clone();
    assert_eq!(shots[0], (None, 1), "the first shot predates the insert");
    assert_eq!(shots[1].1, 2, "the second shot sees it");
    let exports = renderer.exports.lock().expect("lock").clone();
    assert_eq!(exports.len(), 1);
    assert_eq!(exports[0].format, ExportFormat::Png);
    assert_eq!(exports[0].scale, 1.0);
    assert_eq!(ran.result["exported"].as_array().map(Vec::len), Some(1));
    assert_eq!(ran.result["screenshots"][0], "document");
}

#[test]
fn a_failed_script_renders_and_exports_nothing() {
    let renderer = Arc::new(FakeRenderer::default());
    let tool = execute_snapshot_with_renderer(&sample(), renderer.clone());
    let outcome = tool.call(&args(
        r#"TakeScreenshot("n10"); Export(["n10"], "png", "/tmp/out"); throw new Error("no");"#,
    ));
    assert!(matches!(outcome, ToolOutcome::Err(..)), "{outcome:?}");
    assert!(renderer.shots.lock().expect("lock").is_empty());
    assert!(renderer.exports.lock().expect("lock").is_empty());
}

#[test]
fn export_refuses_html_formats_for_now() {
    let tool = execute_snapshot_with_renderer(&sample(), Arc::new(FakeRenderer::default()));
    let outcome = tool.call(&args(r#"Export(["n10"], "html-tailwind", "/tmp/x.html")"#));
    let ToolOutcome::Err(_, message) = outcome else {
        panic!("expected an error");
    };
    assert!(message.contains("codegen_export"), "{message}");
}

#[test]
fn a_page_id_pins_reads_and_writes_to_that_page() {
    let mut state = sample();
    assert!(state.apply(EditorCommand::AddPage {
        name: Some("Second".into()),
        children: Some(Vec::new()),
    }));
    let pages = state.doc.pages.clone().expect("pages");
    let first = pages[0].id.clone();
    let mut call_args = args(
        r#"Print(Get(n => n.name).length); Insert(document, {type: "frame", name: "Here", width: 10, height: 10});"#,
    );
    call_args.insert("pageId".into(), first.clone());
    let ran = unwrap_ok(execute_snapshot(&state).call(&call_args));
    assert_ne!(
        printed(&ran),
        vec!["0"],
        "the first page's nodes are visible"
    );
    assert!(state.apply(ran.command.expect("command")));
    let page = &state.doc.pages.as_ref().expect("pages")[0];
    assert!(page
        .children
        .iter()
        .any(|n| n.base().name.as_deref() == Some("Here")));
}

#[test]
fn empty_and_oversized_scripts_are_invalid_arguments() {
    let (code, _) = run_err(&empty(), "   ");
    assert_eq!(code, ToolErrorCode::InvalidArgument);
    let big = "x".repeat(super::MAX_SCRIPT_BYTES + 1);
    let (code, message) = run_err(&empty(), &big);
    assert_eq!(code, ToolErrorCode::InvalidArgument);
    assert!(message.contains("too large"), "{message}");
}
