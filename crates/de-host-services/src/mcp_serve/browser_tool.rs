//! MCP tool `browser` (#20): drive a real web page in a headless browser,
//! pen.dev's `browser` tool. A call names a browser node (`nodeId`, on any
//! page) or, without one, the tool's own page that no node shows; each
//! keeps its tab open between calls, so a page loaded once can be clicked,
//! read and captured afterwards. The actions are [`ACTIONS`].
//!
//! - `load-page` opens `url`; on a node it also becomes the node's address
//!   (one undo step).
//! - On a node, `load-page` and `cdp` also put what the tab now shows on the
//!   canvas, so the node shows the page the agent drives; its tab follows
//!   the node's address when the user changes it.
//! - `return-screenshot` / `return-element` hand the page (or one element,
//!   `target: query`) back to the client.
//! - `cdp` sends one DevTools command (`de_browser_render::CDP_DOMAINS`).
//! - `screenshot-to-canvas` / `import-to-canvas` put the page into the
//!   document as an image or as editable layers (the Chrome extension's
//!   snapshot path). An import also makes the fonts its text names
//!   available where the host can (`browser_tool_fonts`, #301).
//!
//! A document whose pages the user has not let load (`browser_live`) keeps
//! its nodes out of reach; addresses follow `browser_url_policy`. Calls
//! block their MCP connection thread while the page loads (capped, see
//! `de_browser_render`), never the UI thread.

use std::collections::BTreeMap;
use std::sync::Arc;

use base64::Engine as _;
use de_browser_render::{BrowserRenderError, ShotTarget, Viewport};
use de_editor_core::browser_devices::normalize_browser_url;
use de_editor_core::browser_live::resolve_page_url;
use de_editor_core::browser_live::{browser_layout, browser_url};
use de_editor_core::{EditorCommand, EditorState, NodeId};
use de_mcp::{McpTool, ToolErrorCode, ToolOutcome};
use serde_json::{json, Value};

use super::browser_tool_canvas::{
    collect_targets, import_nodes, install_canvas_picture, place_import, screenshot_command,
    BrowserTarget,
};
use super::browser_tool_driver::{PageCall, PageDriver, PageOp, PageReply, SessionDriver};
use super::browser_tool_fonts::{decode_fetched, load_import_fonts, FontReport};
use super::font_tools::NO_FONT_HOST;
use crate::font_host::{self, FontHost};

/// Every action, in the schema's order.
pub(crate) const ACTIONS: [&str; 6] = [
    "load-page",
    "return-screenshot",
    "return-element",
    "cdp",
    "screenshot-to-canvas",
    "import-to-canvas",
];
/// The tool's own page (no node): a desktop viewport.
const OWN_PAGE_VIEWPORT: Viewport = Viewport {
    width: 1440,
    height: 900,
    scale: 1.0,
    mobile: false,
};
const OWN_PAGE_KEY: &str = "browser-tool";

pub(crate) type ToolError = (ToolErrorCode, String);

pub struct BrowserTool {
    targets: BTreeMap<String, BrowserTarget>,
    pages_allowed: bool,
    /// Where `./page.html` addresses resolve (#283).
    document_folder: Option<String>,
    driver: Arc<dyn PageDriver>,
    /// Makes an import's fonts available; `None` where the host installs
    /// no font host (the `--serve-web` daemon).
    font_host: Option<&'static dyn FontHost>,
}

impl McpTool for BrowserTool {
    fn name(&self) -> &str {
        "browser"
    }

    fn call(&self, args: &BTreeMap<String, String>) -> ToolOutcome {
        match self.run(args) {
            Ok(outcome) => outcome,
            Err((code, message)) => ToolOutcome::Err(code, message),
        }
    }
}

/// Snapshot constructor: the document's browser nodes and the load gate.
pub fn browser_tool_snapshot(state: &EditorState) -> BrowserTool {
    BrowserTool::with_driver(state, Arc::new(SessionDriver)).with_font_host(font_host::installed())
}

impl BrowserTool {
    pub(crate) fn with_driver(state: &EditorState, driver: Arc<dyn PageDriver>) -> Self {
        Self {
            targets: collect_targets(state),
            pages_allowed: state.editor_ui.browser_live.pages_allowed(),
            document_folder: state.editor_ui.document_folder.clone(),
            driver,
            font_host: None,
        }
    }

    pub(crate) fn with_font_host(mut self, font_host: Option<&'static dyn FontHost>) -> Self {
        self.font_host = font_host;
        self
    }

    fn run(&self, args: &BTreeMap<String, String>) -> Result<ToolOutcome, ToolError> {
        let action = required(args, "action")?;
        if !ACTIONS.contains(&action) {
            return Err(invalid(format!(
                "unknown action `{action}` ({})",
                ACTIONS.join(", ")
            )));
        }
        let target = self.target(args)?;
        let folder = self.document_folder.as_deref();
        let node_url = target
            .and_then(|t| browser_url(&t.node))
            .map(|url| resolve_page_url(url, folder).into_owned());
        if let Some(url) = &node_url {
            ensure_permitted(url)?;
        }
        let call = PageCall {
            key: target.map_or(OWN_PAGE_KEY.to_string(), |t| format!("node:{}", t.id)),
            viewport: target.map_or(OWN_PAGE_VIEWPORT, |t| node_viewport(&t.node)),
            node_url: node_url.clone(),
            canvas_scale: target.and_then(BrowserTarget::canvas_scale),
        };
        let run = |op: PageOp| self.driver.run(&call, op).map_err(render_error);
        match action {
            "load-page" => {
                let raw = required(args, "url")?;
                // The node keeps the address as typed (a `./` path stays
                // relative); the page loads its resolved form.
                let typed = normalize_browser_url(raw)
                    .filter(|url| !url.is_empty())
                    .ok_or_else(|| {
                        invalid(format!("`{raw}` is not an http, https or file address"))
                    })?;
                let url = resolve_page_url(&typed, folder).into_owned();
                ensure_permitted(&url)?;
                let done = run(PageOp::Load(url.clone()))?;
                let PageReply::Loaded { url, title } = done.reply else {
                    return Err(internal());
                };
                let result = json!({ "loaded": url, "title": title }).to_string();
                let Some(target) = target else {
                    return Ok(ToolOutcome::OkJson(result));
                };
                if let Some(png) = done.canvas {
                    let shown = target.with_url(&typed);
                    install_canvas_picture(&shown, target.reload, folder, png);
                }
                if browser_url(&target.node) == Some(typed.as_str()) {
                    return Ok(ToolOutcome::OkJson(result));
                }
                Ok(ToolOutcome::OkJsonWithCommand(
                    result,
                    EditorCommand::PatchNodeData {
                        node_id: NodeId::new(&target.id),
                        patch_json: json!({ "url": typed }).to_string(),
                        page_id: target.page_id.clone(),
                    },
                ))
            }
            "return-screenshot" => {
                let PageReply::Png(png) = run(PageOp::Screenshot(shot_target(args)?))?.reply else {
                    return Err(internal());
                };
                Ok(image_outcome(&png))
            }
            "return-element" => {
                // Without a query the whole body: prefer a specific selector.
                let selector = match shot_target(args)? {
                    ShotTarget::Query(selector) => selector,
                    ShotTarget::FullPage => "body".to_string(),
                };
                let PageReply::Json(value) = run(PageOp::Element(selector))?.reply else {
                    return Err(internal());
                };
                Ok(ToolOutcome::OkJson(value.to_string()))
            }
            "cdp" => {
                let method = required(args, "method")?.to_string();
                de_browser_render::check_cdp_method(&method).map_err(render_error)?;
                let params = match arg(args, "params") {
                    Some(raw) => serde_json::from_str::<Value>(raw)
                        .map_err(|e| invalid(format!("params must be a JSON object: {e}")))?,
                    None => json!({}),
                };
                let done = run(PageOp::Cdp(method.clone(), params))?;
                if let (Some(target), Some(png)) = (target, done.canvas) {
                    install_canvas_picture(&target.node, target.reload, folder, png);
                }
                let PageReply::Json(reply) = done.reply else {
                    return Err(internal());
                };
                if method == "Page.captureScreenshot" {
                    let png = de_browser_render::PageSession::screenshot_reply_png(&reply)
                        .map_err(render_error)?;
                    return Ok(image_outcome(&png));
                }
                Ok(ToolOutcome::OkJson(reply.to_string()))
            }
            "screenshot-to-canvas" => {
                let PageReply::Png(png) = run(PageOp::Screenshot(shot_target(args)?))?.reply else {
                    return Err(internal());
                };
                screenshot_command(&png, target, node_url.as_deref())
            }
            _ => {
                // import-to-canvas
                let selector = match shot_target(args)? {
                    ShotTarget::Query(selector) => Some(selector),
                    ShotTarget::FullPage => None,
                };
                let auto_layout = match arg(args, "layout").unwrap_or("absolute") {
                    "absolute" => false,
                    "auto" => true,
                    other => {
                        return Err(invalid(format!(
                            "unknown layout `{other}` (absolute, auto)"
                        )))
                    }
                };
                let extract = PageOp::Extract {
                    selector,
                    expand_scroll: expand_scroll(args)?,
                };
                let PageReply::Snapshot(snapshot) = run(extract)?.reply else {
                    return Err(internal());
                };
                let mut result = import_nodes(&snapshot, auto_layout);
                let fonts = self.font_host.map(|host| {
                    let fetch = |urls: Vec<String>| {
                        let count = urls.len();
                        match run(PageOp::FetchFonts(urls)).map(|done| done.reply) {
                            Ok(PageReply::Json(reply)) => decode_fetched(&reply, count),
                            _ => vec![None; count],
                        }
                    };
                    load_import_fonts(host, &mut result.nodes, &snapshot, &fetch)
                });
                Ok(with_font_report(place_import(result, target), fonts))
            }
        }
    }

    /// The node `nodeId` names, if any; refused while its pages are held.
    fn target(&self, args: &BTreeMap<String, String>) -> Result<Option<&BrowserTarget>, ToolError> {
        let Some(id) = arg(args, "nodeId") else {
            return Ok(None);
        };
        let target = self
            .targets
            .get(id)
            .ok_or_else(|| invalid(format!("`{id}` is not a browser node in this document")))?;
        if !self.pages_allowed {
            return Err(invalid(
                "this document's live pages are held until the user presses Load pages in the \
                 Browser panel; ask them, or call without nodeId to use the tool's own page"
                    .into(),
            ));
        }
        Ok(Some(target))
    }
}

/// Add what the import did about fonts to its result. Without a font host
/// nothing was loaded, and the result says so.
fn with_font_report(outcome: ToolOutcome, fonts: Option<FontReport>) -> ToolOutcome {
    let ToolOutcome::OkWithCommand(mut output, command) = outcome else {
        return outcome;
    };
    match fonts {
        Some(report) => report.write(&mut output),
        None => {
            output.insert("fonts".into(), format!("not loaded: {NO_FONT_HOST}"));
        }
    }
    ToolOutcome::OkWithCommand(output, command)
}

/// `target`: `full-page` (default) or `query` (one element). There is no
/// element picker, so `selection` is refused.
fn shot_target(args: &BTreeMap<String, String>) -> Result<ShotTarget, ToolError> {
    match arg(args, "target").unwrap_or("full-page") {
        "full-page" => Ok(ShotTarget::FullPage),
        "query" => Ok(ShotTarget::Query(
            required(args, "querySelector")?.to_string(),
        )),
        "selection" => Err(invalid(
            "this editor has no element picker: use target `query` with a querySelector".into(),
        )),
        other => Err(invalid(format!(
            "unknown target `{other}` (full-page, selection, query)"
        ))),
    }
}

fn ensure_permitted(url: &str) -> Result<(), ToolError> {
    if crate::browser_url_policy::url_permitted(url) {
        Ok(())
    } else {
        Err(invalid(format!("`{url}` is not allowed on this server")))
    }
}

fn image_outcome(png: &[u8]) -> ToolOutcome {
    let (width, height) =
        de_util::image_dimensions::encoded_image_dimensions(png).unwrap_or((0, 0));
    ToolOutcome::OkImageContent {
        image_base64: base64::engine::general_purpose::STANDARD.encode(png),
        mime_type: "image/png".into(),
        metadata_json: Some(json!({ "width": width, "height": height }).to_string()),
    }
}

/// A node's tab lays out like its canvas picture, at 1× for the client.
fn node_viewport(node: &jian_ops_schema::node::PenNode) -> Viewport {
    browser_layout(node).map_or(OWN_PAGE_VIEWPORT, |layout| Viewport {
        width: layout.width,
        height: layout.height,
        scale: 1.0,
        mobile: layout.mobile,
    })
}

/// `expandScroll` (import-to-canvas): lay scroll containers out in full for
/// the capture instead of importing them as a clip. Off unless `true`.
fn expand_scroll(args: &BTreeMap<String, String>) -> Result<bool, ToolError> {
    match arg(args, "expandScroll") {
        None | Some("false") => Ok(false),
        Some("true") => Ok(true),
        Some(other) => Err(invalid(format!(
            "expandScroll must be true or false, not `{other}`"
        ))),
    }
}

fn arg<'a>(args: &'a BTreeMap<String, String>, key: &str) -> Option<&'a str> {
    args.get(key).map(|v| v.trim()).filter(|v| !v.is_empty())
}

fn required<'a>(args: &'a BTreeMap<String, String>, key: &str) -> Result<&'a str, ToolError> {
    arg(args, key).ok_or_else(|| (ToolErrorCode::MissingArgument, format!("{key} is required")))
}

fn invalid(message: String) -> ToolError {
    (ToolErrorCode::InvalidArgument, message)
}

/// A wrong selector, method or missing page is the caller's; the rest is
/// the browser failing.
fn render_error(error: BrowserRenderError) -> ToolError {
    let code = if error.is_caller_error() {
        ToolErrorCode::InvalidArgument
    } else {
        ToolErrorCode::ToolFailed
    };
    (code, error.to_string())
}

fn internal() -> ToolError {
    (
        ToolErrorCode::Internal,
        "the page driver answered with the wrong kind of reply".into(),
    )
}

#[cfg(test)]
#[path = "browser_tool_tests.rs"]
mod tests;
