//! The `browser` actions that touch the canvas: the browser nodes the tool
//! can drive (with where output goes beside them), a page screenshot as an
//! image node, the page as editable layers (the web snapshot importer the
//! Chrome extension uses), and a node's canvas picture after the agent
//! changed its page.

use std::collections::BTreeMap;

use base64::Engine as _;
use de_editor_core::browser_live::{browser_layout, browser_render_request};
use de_editor_core::pen_node_ext::PenNodeExt;
use de_editor_core::{EditorCommand, EditorState, NodeId};
use de_editor_ui::widgets::browser_textures;
use de_html::{import_snapshot, HtmlImportOptions};
use de_mcp::import_common::{import_result_to_outcome, ImportPlacement};
use de_mcp::{ToolErrorCode, ToolOutcome};
use jian_ops_schema::node::PenNode;
use serde_json::json;

use super::browser_tool::ToolError;

/// Gap between a browser node and what is placed beside it (doc px).
const PLACE_GAP: f64 = 80.0;

/// A browser node the tool can drive.
#[derive(Debug, Clone)]
pub struct BrowserTarget {
    pub id: String,
    /// Its page; `None` for a single-page document.
    pub page_id: Option<String>,
    pub node: PenNode,
    /// Its reload count, part of its canvas picture's key.
    pub reload: u32,
    /// Where output lands beside it; `None` for a nested node, whose
    /// output goes wherever root placement puts it.
    pub beside: Option<(f64, f64)>,
}

impl BrowserTarget {
    /// The node as it will be once its address is `url`.
    pub fn with_url(&self, url: &str) -> PenNode {
        let mut node = self.node.clone();
        if let PenNode::Browser(browser) = &mut node {
            browser.url = Some(url.to_string());
        }
        node
    }

    /// The output scale of this node's canvas picture.
    pub fn canvas_scale(&self) -> Option<f64> {
        browser_layout(&self.node).map(|layout| layout.scale)
    }
}

/// Every browser node in the document, by id.
pub fn collect_targets(state: &EditorState) -> BTreeMap<String, BrowserTarget> {
    let mut out = BTreeMap::new();
    let reloads = |id: &str| state.editor_ui.browser_live.reload_count(id);
    match state.doc.pages.as_deref() {
        Some(pages) => {
            for page in pages {
                walk(&page.children, Some(&page.id), true, &reloads, &mut out);
            }
        }
        None => walk(&state.doc.children, None, true, &reloads, &mut out),
    }
    out
}

fn walk(
    children: &[PenNode],
    page_id: Option<&String>,
    roots: bool,
    reloads: &dyn Fn(&str) -> u32,
    out: &mut BTreeMap<String, BrowserTarget>,
) {
    for node in children {
        if matches!(node, PenNode::Browser(_)) {
            let base = node.base();
            let beside = roots.then(|| {
                let width = node.width_px().unwrap_or(0.0);
                (
                    base.x.unwrap_or(0.0) + width + PLACE_GAP,
                    base.y.unwrap_or(0.0),
                )
            });
            out.insert(
                base.id.clone(),
                BrowserTarget {
                    id: base.id.clone(),
                    page_id: page_id.cloned(),
                    node: node.clone(),
                    reload: reloads(&base.id),
                    beside,
                },
            );
        }
        if let Some(kids) = node.children_ref() {
            walk(kids, page_id, false, reloads, out);
        }
    }
}

/// Show `png` as `node`'s canvas picture (shaped like the node, at its
/// canvas scale). The render queue sees the key up to date and leaves it
/// until the node changes or is reloaded.
pub fn install_canvas_picture(node: &PenNode, reload: u32, png: Vec<u8>) {
    let Some(request) = browser_render_request(node, reload) else {
        return;
    };
    browser_textures::install(&request.node_id, request.key, png);
}

/// Insert `png` as an image node beside `target` (at its pixel size, which
/// is CSS px for a 1× capture).
pub fn screenshot_command(
    png: &[u8],
    target: Option<&BrowserTarget>,
    url: Option<&str>,
) -> Result<ToolOutcome, ToolError> {
    let (width, height) = de_util::image_dimensions::encoded_image_dimensions(png)
        .filter(|(w, h)| *w > 0 && *h > 0)
        .ok_or((
            ToolErrorCode::ToolFailed,
            "the screenshot is not a readable image".to_string(),
        ))?;
    let name = url.map_or("Page screenshot".to_string(), |url| {
        format!("Screenshot of {url}")
    });
    // The host gives inserted nodes ids of its own.
    let mut image = json!({
        "type": "image",
        "id": "browser-screenshot",
        "name": name,
        "width": width,
        "height": height,
        "src": format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(png)
        ),
    });
    if let Some((x, y)) = target.and_then(|t| t.beside) {
        image["x"] = json!(x);
        image["y"] = json!(y);
    }
    let image: PenNode = serde_json::from_value(image).map_err(|e| {
        (
            ToolErrorCode::Internal,
            format!("could not build the image node: {e}"),
        )
    })?;
    let mut output = BTreeMap::new();
    output.insert("wrote".into(), "true".into());
    output.insert("width".into(), width.to_string());
    output.insert("height".into(), height.to_string());
    Ok(ToolOutcome::OkWithCommand(
        output,
        EditorCommand::InsertSubtree {
            nodes: vec![image],
            parent_id: NodeId::NONE,
            page_id: target.and_then(|t| t.page_id.clone()),
        },
    ))
}

/// Turn the extractor's `snapshot` into editable layers beside `target`.
pub fn import_command(snapshot: &str, target: Option<&BrowserTarget>) -> ToolOutcome {
    let result = import_snapshot(snapshot, &HtmlImportOptions::default());
    let (x, y) = target.and_then(|t| t.beside).unwrap_or((0.0, 0.0));
    import_result_to_outcome(
        result,
        ImportPlacement {
            x: x.round() as i32,
            y: y.round() as i32,
            parent_id: NodeId::NONE,
            page_id: target.and_then(|t| t.page_id.clone()),
        },
    )
}
