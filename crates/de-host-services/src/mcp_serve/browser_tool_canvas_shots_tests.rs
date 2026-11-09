//! Canvas shots: the snapshot patch against a fake shooter, the tool's
//! import with a scripted page, and (with `--ignored`) a real WebGL page.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use de_browser_render::{BrowserRenderError, ShotTarget};
use de_editor_core::{EditorCommand, EditorState};
use de_mcp::{McpTool, ToolErrorCode, ToolOutcome};
use serde_json::{json, Value};

use super::*;
use crate::mcp_serve::browser_tool::BrowserTool;
use crate::mcp_serve::browser_tool_driver::{PageCall, PageDriver, PageOp, PageReply, PageResult};

const BLANK: &str = "data:image/png;base64,blank";

fn canvas(id: u64, y: f64, tainted: bool) -> Value {
    let mut node = json!({
        "kind": "image", "tag": "canvas",
        "rect": { "x": 0, "y": y, "w": 64, "h": 32 },
        "src": BLANK, "styles": {},
        "canvasShot": { "id": id, "x": 4.5, "y": y, "w": 64, "h": 32 },
    });
    if tainted {
        node["tainted"] = json!(true);
    }
    node
}

fn snapshot(children: Vec<Value>) -> String {
    json!({
        "version": 1,
        "root": { "kind": "element", "tag": "body",
                  "rect": { "x": 0, "y": 0, "w": 400, "h": 4000 },
                  "styles": {}, "children": children }
    })
    .to_string()
}

fn children(patched: &str) -> Vec<Value> {
    let value: Value = serde_json::from_str(patched).unwrap();
    value["root"]["children"].as_array().unwrap().clone()
}

fn png() -> Vec<u8> {
    b"\x89PNG\r\n\x1a\nshot".to_vec()
}

fn shot_url() -> String {
    format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(png())
    )
}

#[test]
fn a_snapshot_without_tagged_canvases_is_left_as_it_came() {
    let original = snapshot(vec![json!({ "kind": "image", "tag": "img",
        "rect": { "x": 0, "y": 0, "w": 1, "h": 1 }, "src": BLANK, "styles": {} })]);
    let mut calls = 0;
    let (patched, report) = patch_canvas_shots(original.clone(), &mut |_| {
        calls += 1;
        Ok(png())
    });
    assert_eq!(patched, original, "byte for byte");
    assert_eq!(report, CanvasShotReport::default());
    assert_eq!(calls, 0);
}

#[test]
fn each_tagged_canvas_gets_the_screenshot_of_its_rect() {
    let plain = json!({ "kind": "image", "tag": "img",
        "rect": { "x": 0, "y": 0, "w": 1, "h": 1 }, "src": BLANK, "styles": {} });
    let original = snapshot(vec![
        canvas(1, 10.0, false),
        plain.clone(),
        canvas(2, 900.0, true),
    ]);
    let mut asked = Vec::new();
    let (patched, report) = patch_canvas_shots(original, &mut |target| {
        asked.push(target);
        Ok(png())
    });
    assert_eq!(
        asked,
        vec![
            ShotTarget::Rect {
                x: 4.5,
                y: 10.0,
                w: 64.0,
                h: 32.0
            },
            ShotTarget::Rect {
                x: 4.5,
                y: 900.0,
                w: 64.0,
                h: 32.0
            },
        ]
    );
    let nodes = children(&patched);
    for node in [&nodes[0], &nodes[2]] {
        assert_eq!(node["src"], json!(shot_url()));
        assert!(node.get("canvasShot").is_none(), "the tag is dropped");
        assert!(
            node.get("tainted").is_none(),
            "the shot is what the page showed"
        );
    }
    assert_eq!(nodes[1], plain);
    assert_eq!(
        report,
        CanvasShotReport {
            tagged: 2,
            captured: 2,
            warnings: vec![]
        }
    );
}

#[test]
fn a_failed_shot_keeps_the_read_back_warns_and_ends_the_shots() {
    let original = snapshot(vec![
        canvas(1, 0.0, false),
        canvas(2, 50.0, true),
        canvas(3, 90.0, false),
    ]);
    let (patched, report) = patch_canvas_shots(original, &mut |target| match target {
        ShotTarget::Rect { y: 50.0, .. } => Err((
            ToolErrorCode::ToolFailed,
            "Page.captureScreenshot timed out".into(),
        )),
        _ => Ok(png()),
    });
    let nodes = children(&patched);
    assert_eq!(nodes[0]["src"], json!(shot_url()));
    assert_eq!(nodes[1]["src"], json!(BLANK));
    assert_eq!(nodes[1]["tainted"], json!(true), "still counted as unread");
    assert!(nodes[1].get("canvasShot").is_none());
    assert_eq!(nodes[2]["src"], json!(BLANK), "not tried after the failure");
    assert_eq!((report.tagged, report.captured), (3, 1));
    assert_eq!(
        report.warnings,
        vec![
            "canvas 2 could not be captured (Page.captureScreenshot timed out); \
             it keeps the page's own read-back"
                .to_string(),
            "1 more canvases were not captured (after a failed shot); \
             they keep the page's own read-back"
                .to_string(),
        ]
    );
}

#[test]
fn styles_the_shot_already_shows_are_not_applied_again() {
    let mut faded = canvas(1, 0.0, false);
    faded["styles"] = json!({ "opacity": "0.5", "filter": "blur(2px)",
        "mix-blend-mode": "multiply", "border-radius": "8px" });
    let (patched, _) = patch_canvas_shots(snapshot(vec![faded]), &mut |_| Ok(png()));
    assert_eq!(
        children(&patched)[0]["styles"],
        json!({ "border-radius": "8px" })
    );
}

#[test]
fn shots_stop_at_the_count_cap() {
    let tagged = (1..=MAX_CANVAS_SHOTS as u64 + 2)
        .map(|id| canvas(id, id as f64 * 40.0, false))
        .collect();
    let mut calls = 0;
    let (patched, report) = patch_canvas_shots(snapshot(tagged), &mut |_| {
        calls += 1;
        Ok(png())
    });
    assert_eq!(calls, MAX_CANVAS_SHOTS);
    assert_eq!(report.captured, MAX_CANVAS_SHOTS);
    assert_eq!(report.tagged, MAX_CANVAS_SHOTS + 2);
    assert!(report.warnings[0].starts_with("2 more canvases were not captured (at most 16"));
    let nodes = children(&patched);
    assert_eq!(nodes[MAX_CANVAS_SHOTS]["src"], json!(BLANK));
}

#[test]
fn shots_stop_at_the_byte_cap() {
    // 17 MiB of PNG is ~22.7 MiB as base64: one fits the 24 MiB budget.
    let big = vec![0u8; 17 * 1024 * 1024];
    let original = snapshot(vec![canvas(1, 0.0, false), canvas(2, 50.0, false)]);
    let (patched, report) = patch_canvas_shots(original, &mut |_| Ok(big.clone()));
    assert_eq!(report.captured, 1);
    assert_eq!(
        report.warnings,
        vec!["canvas 2 is too large to embed; it keeps the page's own read-back".to_string()]
    );
    assert!(patched.len() <= de_html::snapshot::MAX_SNAPSHOT_BYTES);
    assert_eq!(children(&patched)[1]["src"], json!(BLANK));
}

#[test]
fn the_report_joins_the_import_warnings() {
    let mut output = BTreeMap::new();
    output.insert("warnings".to_string(), "first\nsecond".to_string());
    let command = EditorCommand::Undo;
    let report = CanvasShotReport {
        tagged: 2,
        captured: 1,
        warnings: vec!["third".into()],
    };
    let ToolOutcome::OkWithCommand(output, _) =
        report.write(ToolOutcome::OkWithCommand(output, command))
    else {
        panic!("still the import");
    };
    assert_eq!(output["warnings"], "first\nsecond\nthird");
    assert_eq!(output["canvasShots"], "1 of 2");
}

#[test]
fn the_extractor_writes_the_tag_this_module_reads() {
    let js = de_html::snapshot::SNAPSHOT_EXTRACTOR_JS;
    let tag = format!("result.{SHOT_KEY} = {{ id: canvasShotCount }}");
    let mut markers = vec![
        tag,
        "var canvasShots = !!(options && options.canvasShots);".to_string(),
        "dataUrl === blankCanvasUrl(canvas.width, canvas.height)".to_string(),
    ];
    for (field, value) in [
        ("x", "box.left + window.scrollX"),
        ("y", "box.top + window.scrollY"),
        ("w", "box.width"),
        ("h", "box.height"),
    ] {
        markers.push(format!("shot.{field} = round({value});"));
    }
    for marker in markers {
        assert!(js.contains(&marker), "extractor is missing {marker}");
    }
    assert!(
        !js.contains("!!canvas.getContext("),
        "probing a context would give a context-less canvas one for good"
    );
}

/// A page whose extractor run tags one canvas; its screenshot fails when
/// `fail` is set.
struct ScriptedPage {
    fail: bool,
    ops: Mutex<Vec<PageOp>>,
}

impl PageDriver for ScriptedPage {
    fn run(&self, _call: &PageCall, op: PageOp) -> Result<PageResult, BrowserRenderError> {
        self.ops.lock().unwrap().push(op.clone());
        let reply = match op {
            PageOp::Extract { .. } => PageReply::Snapshot(snapshot(vec![canvas(1, 120.0, false)])),
            PageOp::Screenshot(_) if self.fail => return Err(BrowserRenderError::Screenshot),
            PageOp::Screenshot(_) => PageReply::Png(png()),
            PageOp::FetchFonts(urls) => PageReply::Json(json!(vec![None::<String>; urls.len()])),
            _ => PageReply::Json(json!({})),
        };
        Ok(PageResult {
            reply,
            canvas: None,
        })
    }
}

fn import(fail: bool) -> (ToolOutcome, Vec<PageOp>) {
    let page = Arc::new(ScriptedPage {
        fail,
        ops: Mutex::default(),
    });
    let tool = BrowserTool::with_driver(&EditorState::new(), page.clone());
    let args = BTreeMap::from([("action".to_string(), "import-to-canvas".to_string())]);
    let outcome = tool.call(&args);
    let ops = page.ops.lock().unwrap().clone();
    (outcome, ops)
}

/// Every image `src` the command inserts.
fn image_sources(command: &EditorCommand) -> Vec<String> {
    let EditorCommand::InsertSubtree { nodes, .. } = command else {
        panic!("expected an insert, got {command:?}");
    };
    let mut out = Vec::new();
    fn walk(value: &Value, out: &mut Vec<String>) {
        match value {
            Value::Object(node) => {
                if node.get("type") == Some(&json!("image")) {
                    out.extend(node.get("src").and_then(Value::as_str).map(str::to_string));
                }
                node.values().for_each(|child| walk(child, out));
            }
            Value::Array(items) => items.iter().for_each(|item| walk(item, out)),
            _ => {}
        }
    }
    walk(&serde_json::to_value(nodes).unwrap(), &mut out);
    out
}

#[test]
fn an_import_shoots_its_tagged_canvases_before_importing() {
    let (outcome, ops) = import(false);
    assert_eq!(
        ops[1],
        PageOp::Screenshot(ShotTarget::Rect {
            x: 4.5,
            y: 120.0,
            w: 64.0,
            h: 32.0
        })
    );
    let ToolOutcome::OkWithCommand(output, command) = outcome else {
        panic!("expected an import, got {outcome:?}");
    };
    assert_eq!(output["canvasShots"], "1 of 1");
    assert_eq!(image_sources(&command), vec![shot_url()]);
}

#[test]
fn a_failed_shot_is_a_warning_not_a_failed_import() {
    let (outcome, _) = import(true);
    let ToolOutcome::OkWithCommand(output, command) = outcome else {
        panic!("expected an import, got {outcome:?}");
    };
    assert_eq!(output["canvasShots"], "0 of 1");
    assert!(
        output["warnings"]
            .contains("canvas 1 could not be captured (the screenshot was not a PNG)"),
        "{output:?}"
    );
    assert_eq!(image_sources(&command), vec![BLANK.to_string()]);
}

/// A WebGL canvas without `preserveDrawingBuffer`, cleared red, imported
/// from a real page: `cargo test -p de-host-services canvas_shots -- --ignored`.
#[test]
#[ignore = "needs an installed Chromium-based browser"]
fn a_webgl_canvas_imports_as_the_browser_drew_it() {
    use crate::mcp_serve::browser_tool_driver::SessionDriver;

    let dir = std::env::temp_dir().join(format!("de-webgl-import-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let page = dir.join("page.html");
    std::fs::write(
        &page,
        "<html><body style='margin:0;height:2400px'>\
         <canvas id='gl' width='80' height='40' \
         style='position:absolute;left:10px;top:1600px;width:80px;height:40px'></canvas>\
         <script>const gl = document.getElementById('gl').getContext('webgl');\
         gl.clearColor(1, 0, 0, 1); gl.clear(gl.COLOR_BUFFER_BIT);</script></body></html>",
    )
    .unwrap();
    let tool = BrowserTool::with_driver(&EditorState::new(), Arc::new(SessionDriver));
    let url = format!("file://{}", page.display());
    let call = |pairs: &[(&str, &str)]| {
        let args: BTreeMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        tool.call(&args)
    };
    let loaded = call(&[("action", "load-page"), ("url", &url)]);
    assert!(matches!(loaded, ToolOutcome::OkJson(_)), "{loaded:?}");
    let outcome = call(&[("action", "import-to-canvas")]);
    let ToolOutcome::OkWithCommand(output, command) = outcome else {
        panic!("expected an import, got {outcome:?}");
    };
    assert_eq!(output["canvasShots"], "1 of 1", "{output:?}");
    let sources = image_sources(&command);
    assert_eq!(sources.len(), 1);
    let encoded = sources[0].strip_prefix("data:image/png;base64,").unwrap();
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .unwrap();
    let image = skia_safe::Image::from_encoded(skia_safe::Data::new_copy(&bytes)).unwrap();
    assert_eq!((image.width(), image.height()), (80, 40));
    let info = skia_safe::ImageInfo::new(
        (1, 1),
        skia_safe::ColorType::RGBA8888,
        skia_safe::AlphaType::Unpremul,
        None,
    );
    let mut pixel = [0u8; 4];
    assert!(image.read_pixels(
        &info,
        &mut pixel,
        4,
        (40, 20),
        skia_safe::image::CachingHint::Allow
    ));
    assert_eq!(pixel, [255, 0, 0, 255], "the canvas is red, not blank");
    de_browser_render::PageSessions::shared().close("browser-tool");
    let _ = std::fs::remove_dir_all(dir);
}
