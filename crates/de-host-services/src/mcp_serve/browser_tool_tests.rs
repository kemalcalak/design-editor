//! The `browser` tool against a scripted page driver: argument rules, what
//! each action returns and which document command it carries. The page
//! side itself is tested in `de-browser-render` (with a real browser under
//! `--ignored`).

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use de_browser_render::{BrowserRenderError, ShotTarget};
use de_editor_core::pen_node_ext::PenNodeExt;
use de_editor_core::{EditorCommand, EditorState};
use de_mcp::{McpTool, ToolErrorCode, ToolOutcome};
use jian_ops_schema::node::PenNode;
use serde_json::json;

use super::{BrowserTool, ACTIONS};
use crate::mcp_serve::browser_tool_driver::{PageCall, PageDriver, PageOp, PageReply, PageResult};

/// A 2×1 PNG header is all the size reader needs.
fn png() -> Vec<u8> {
    let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
    png.extend_from_slice(&2u32.to_be_bytes());
    png.extend_from_slice(&1u32.to_be_bytes());
    png.extend_from_slice(&[8, 6, 0, 0, 0]);
    png
}

#[derive(Default)]
struct FakeDriver {
    calls: Mutex<Vec<(PageCall, PageOp)>>,
}

impl PageDriver for FakeDriver {
    fn run(&self, call: &PageCall, op: PageOp) -> Result<PageResult, BrowserRenderError> {
        self.calls.lock().unwrap().push((call.clone(), op.clone()));
        let changes_page = matches!(op, PageOp::Load(_) | PageOp::Cdp(..));
        let reply = match op {
            PageOp::Load(url) => PageReply::Loaded {
                url,
                title: "Example".into(),
            },
            PageOp::Screenshot(_) => PageReply::Png(png()),
            PageOp::Element(selector) if selector == "#missing" => {
                return Err(BrowserRenderError::NoMatch { selector })
            }
            PageOp::Element(_) => PageReply::Json(json!({ "html": "<h1>Hi</h1>" })),
            PageOp::Cdp(method, _) if method == "Page.captureScreenshot" => {
                PageReply::Json(json!({ "data": base64_png() }))
            }
            PageOp::Cdp(_, _) => PageReply::Json(json!({ "ok": true })),
            PageOp::Extract(_) => PageReply::Snapshot(snapshot()),
        };
        let canvas = call.canvas_scale.filter(|_| changes_page).map(|_| png());
        Ok(PageResult { reply, canvas })
    }
}

fn base64_png() -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.encode(png())
}

fn snapshot() -> String {
    json!({
        "version": 1,
        "root": {
            "kind": "element", "tag": "div",
            "rect": { "x": 0, "y": 0, "w": 200, "h": 40 },
            "styles": {},
            "children": [{ "kind": "text", "rect": { "x": 8, "y": 8, "w": 96, "h": 20 },
                           "text": "Hello", "lines": 1, "styles": { "font-size": "14px" } }]
        }
    })
    .to_string()
}

fn state() -> EditorState {
    let doc = serde_json::from_value(json!({
        "version": "1.0",
        "children": [
            { "type": "browser", "id": "b", "x": 100, "y": 50, "width": 390, "height": 844,
              "url": "https://example.com", "deviceId": "phone" },
            { "type": "frame", "id": "wrap", "x": 2000, "y": 0, "width": 800, "height": 800,
              "layout": "none", "children": [
                { "type": "browser", "id": "nested", "x": 10, "y": 20, "width": 400,
                  "height": 300, "url": "https://example.org" }
              ] },
            { "type": "rectangle", "id": "r", "width": 10, "height": 10 }
        ]
    }))
    .unwrap();
    let mut state = EditorState::new();
    state.replace_document(doc);
    state
}

fn call(tool: &BrowserTool, args: &[(&str, &str)]) -> ToolOutcome {
    let args: BTreeMap<String, String> = args
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    tool.call(&args)
}

fn tool() -> (BrowserTool, Arc<FakeDriver>) {
    let driver = Arc::new(FakeDriver::default());
    (BrowserTool::with_driver(&state(), driver.clone()), driver)
}

#[test]
fn load_page_on_a_node_sets_its_address_and_shows_the_page_on_the_canvas() {
    let (tool, driver) = tool();
    let outcome = call(
        &tool,
        &[
            ("action", "load-page"),
            ("nodeId", "b"),
            ("url", "example.org"),
        ],
    );
    let ToolOutcome::OkJsonWithCommand(
        json,
        EditorCommand::PatchNodeData {
            node_id,
            patch_json,
            ..
        },
    ) = outcome
    else {
        panic!("expected a node patch, got {outcome:?}");
    };
    assert!(json.contains("https://example.org") && json.contains("Example"));
    assert_eq!(node_id.as_str(), "b");
    assert_eq!(patch_json, r#"{"url":"https://example.org"}"#);
    let calls = driver.calls.lock().unwrap();
    let (page, op) = &calls[0];
    assert_eq!(page.key, "node:b");
    assert_eq!(
        (
            page.viewport.width,
            page.viewport.height,
            page.viewport.mobile
        ),
        (390, 844, true)
    );
    assert_eq!(page.viewport.scale, 1.0, "the client gets 1× pictures");
    assert_eq!(page.node_url.as_deref(), Some("https://example.com"));
    assert_eq!(page.canvas_scale, Some(2.0));
    assert_eq!(op, &PageOp::Load("https://example.org".into()));
    // The canvas shows the loaded page under the patched node's key.
    let target = &tool.targets["b"];
    let patched = target.with_url("https://example.org");
    let key = de_editor_core::browser_live::browser_render_request(&patched, 0)
        .unwrap()
        .key;
    assert_eq!(
        de_editor_ui::widgets::browser_textures::ready("b").map(|(k, _)| k),
        Some(key)
    );
}

#[test]
fn the_own_page_needs_no_node_and_writes_nothing_on_load() {
    let (tool, driver) = tool();
    assert!(matches!(
        call(
            &tool,
            &[("action", "load-page"), ("url", "https://example.com")]
        ),
        ToolOutcome::OkJson(_)
    ));
    let calls = driver.calls.lock().unwrap();
    assert_eq!(calls[0].0.key, "browser-tool");
    assert_eq!(calls[0].0.canvas_scale, None);
}

#[test]
fn a_held_document_keeps_its_nodes_out_of_reach() {
    let mut held = state();
    held.hold_browser_pages_if_any();
    let tool = BrowserTool::with_driver(&held, Arc::new(FakeDriver::default()));
    assert!(matches!(
        call(&tool, &[("action", "return-screenshot"), ("nodeId", "b")]),
        ToolOutcome::Err(ToolErrorCode::InvalidArgument, m) if m.contains("Load pages")
    ));
    assert!(matches!(
        call(&tool, &[("action", "return-screenshot")]),
        ToolOutcome::OkImageContent { .. }
    ));
}

#[test]
fn screenshots_come_back_as_images_and_targets_are_checked() {
    let (tool, driver) = tool();
    let outcome = call(&tool, &[("action", "return-screenshot"), ("nodeId", "b")]);
    let ToolOutcome::OkImageContent {
        mime_type,
        metadata_json,
        ..
    } = outcome
    else {
        panic!("expected an image, got {outcome:?}");
    };
    assert_eq!(mime_type, "image/png");
    assert!(metadata_json.unwrap().contains("\"width\":2"));
    call(
        &tool,
        &[
            ("action", "return-screenshot"),
            ("target", "query"),
            ("querySelector", ".hero"),
        ],
    );
    assert_eq!(
        driver.calls.lock().unwrap()[1].1,
        PageOp::Screenshot(ShotTarget::Query(".hero".into()))
    );
    let refused = call(
        &tool,
        &[("action", "return-screenshot"), ("target", "selection")],
    );
    assert!(
        matches!(refused, ToolOutcome::Err(ToolErrorCode::InvalidArgument, m) if m.contains("query"))
    );
    assert!(matches!(
        call(
            &tool,
            &[("action", "return-screenshot"), ("target", "query")]
        ),
        ToolOutcome::Err(ToolErrorCode::MissingArgument, _)
    ));
    assert!(matches!(
        call(
            &tool,
            &[
                ("action", "return-element"),
                ("target", "query"),
                ("querySelector", "#missing")
            ]
        ),
        ToolOutcome::Err(ToolErrorCode::InvalidArgument, _)
    ));
}

#[test]
fn cdp_takes_page_domains_and_returns_a_screenshot_as_an_image() {
    let (tool, driver) = tool();
    let params = r#"{"type":"mousePressed","x":10,"y":20,"button":"left"}"#;
    assert!(matches!(
        call(
            &tool,
            &[
                ("action", "cdp"),
                ("method", "Input.dispatchMouseEvent"),
                ("params", params)
            ]
        ),
        ToolOutcome::OkJson(_)
    ));
    assert!(matches!(
        &driver.calls.lock().unwrap()[0].1,
        PageOp::Cdp(method, params) if method == "Input.dispatchMouseEvent" && params["x"] == 10
    ));
    assert!(matches!(
        call(
            &tool,
            &[("action", "cdp"), ("method", "Page.captureScreenshot")]
        ),
        ToolOutcome::OkImageContent { .. }
    ));
    assert!(matches!(
        call(&tool, &[("action", "cdp"), ("method", "Browser.close")]),
        ToolOutcome::Err(ToolErrorCode::InvalidArgument, _)
    ));
    assert!(matches!(
        call(
            &tool,
            &[
                ("action", "cdp"),
                ("method", "DOM.getDocument"),
                ("params", "[1")
            ]
        ),
        ToolOutcome::Err(ToolErrorCode::InvalidArgument, _)
    ));
}

#[test]
fn canvas_actions_insert_beside_a_root_node_and_let_placement_handle_nested_ones() {
    let (tool, _) = tool();
    let outcome = call(
        &tool,
        &[("action", "screenshot-to-canvas"), ("nodeId", "b")],
    );
    let ToolOutcome::OkWithCommand(result, EditorCommand::InsertSubtree { nodes, .. }) = outcome
    else {
        panic!("expected an insert, got {outcome:?}");
    };
    assert_eq!(result["width"], "2");
    let PenNode::Image(image) = &nodes[0] else {
        panic!("image node")
    };
    // The node sits at (100, 50) and is 390 wide: 80 px gap.
    assert_eq!((image.base.x, image.base.y), (Some(570.0), Some(50.0)));
    assert!(image.src.as_str().starts_with("data:image/png;base64,"));
    let nested = call(
        &tool,
        &[("action", "screenshot-to-canvas"), ("nodeId", "nested")],
    );
    let ToolOutcome::OkWithCommand(_, EditorCommand::InsertSubtree { nodes, .. }) = nested else {
        panic!("expected an insert, got {nested:?}");
    };
    assert_eq!(nodes[0].base().x, None, "root placement decides");
    let imported = call(&tool, &[("action", "import-to-canvas"), ("nodeId", "b")]);
    assert!(
        matches!(
            imported,
            ToolOutcome::OkWithCommand(_, EditorCommand::InsertSubtree { .. })
        ),
        "got {imported:?}"
    );
}

#[test]
fn bad_arguments_are_named() {
    let (tool, _) = tool();
    assert!(matches!(
        call(&tool, &[]),
        ToolOutcome::Err(ToolErrorCode::MissingArgument, _)
    ));
    assert!(matches!(
        call(&tool, &[("action", "fly")]),
        ToolOutcome::Err(ToolErrorCode::InvalidArgument, _)
    ));
    assert!(matches!(
        call(&tool, &[("action", "return-screenshot"), ("nodeId", "r")]),
        ToolOutcome::Err(ToolErrorCode::InvalidArgument, m) if m.contains("not a browser node")
    ));
    assert!(matches!(
        call(
            &tool,
            &[("action", "load-page"), ("url", "javascript:alert(1)")]
        ),
        ToolOutcome::Err(ToolErrorCode::InvalidArgument, _)
    ));
}

#[test]
fn the_schema_lists_the_actions_and_cdp_domains_the_tool_has() {
    let schema = crate::mcp_serve::TOOL_SCHEMAS
        .iter()
        .find(|s| s.contains(r#""name":"browser""#))
        .expect("browser schema");
    let schema: serde_json::Value = serde_json::from_str(schema).unwrap();
    let actions: Vec<&str> = schema["inputSchema"]["properties"]["action"]["enum"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    assert_eq!(actions, ACTIONS);
    let description = schema["description"].as_str().unwrap();
    for domain in de_browser_render::CDP_DOMAINS {
        assert!(description.contains(domain), "{domain}");
    }
}

#[test]
fn the_extractor_wrapper_catches_the_download_and_roots_at_a_selector() {
    let script = crate::mcp_serve::browser_tool_driver::extractor_script(Some("main .hero"));
    assert!(script.contains(r#"const selector = "main .hero";"#));
    assert!(script.contains("URL.createObjectURL = (blob)"));
    assert!(script.contains(de_html::snapshot::SNAPSHOT_EXTRACTOR_JS));
    assert!(
        crate::mcp_serve::browser_tool_driver::extractor_script(None)
            .contains("const selector = null;")
    );
}
