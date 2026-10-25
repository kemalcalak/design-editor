//! The page side of the MCP `browser` tool: one call against a kept-open
//! tab ([`de_browser_render::PageSessions`]), behind [`PageDriver`] so the
//! tool's tests run without a browser. Also the wrapper that runs the web
//! snapshot extractor inside the page and hands its JSON back.

use serde_json::Value;

use de_browser_render::{BrowserRenderError, PageSession, PageSessions, ShotTarget, Viewport};

/// One thing to do in a tab.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum PageOp {
    Load(String),
    Screenshot(ShotTarget),
    Element(String),
    Cdp(String, Value),
    /// The web snapshot extractor, rooted at a selector or the whole page.
    Extract(Option<String>),
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum PageReply {
    /// The page loaded: its address and title.
    Loaded {
        url: String,
        title: String,
    },
    Png(Vec<u8>),
    Json(Value),
    /// The extractor's snapshot JSON.
    Snapshot(String),
}

/// Where a call runs.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PageCall {
    /// The tab: a node's own, or the tool's.
    pub key: String,
    pub viewport: Viewport,
    /// A browser node's address: its tab shows it before anything else
    /// runs (also after the user changed the address).
    pub node_url: Option<String>,
    /// For a node: the output scale of its canvas picture. A load or a
    /// `cdp` command then also returns what the tab shows, so the canvas
    /// follows the page the agent drives.
    pub canvas_scale: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PageResult {
    pub reply: PageReply,
    /// The node's new canvas picture (see [`PageCall::canvas_scale`]).
    pub canvas: Option<Vec<u8>>,
}

pub(crate) trait PageDriver: Send + Sync {
    fn run(&self, call: &PageCall, op: PageOp) -> Result<PageResult, BrowserRenderError>;
}

/// The real driver: the process-wide kept-open tabs.
pub(crate) struct SessionDriver;

impl PageDriver for SessionDriver {
    fn run(&self, call: &PageCall, op: PageOp) -> Result<PageResult, BrowserRenderError> {
        de_browser_render::start_idle_reaper();
        PageSessions::shared().with(&call.key, call.viewport, |session| {
            run_in_session(session, call, &op)
        })
    }
}

fn run_in_session(
    session: &mut PageSession,
    call: &PageCall,
    op: &PageOp,
) -> Result<PageResult, BrowserRenderError> {
    if !matches!(op, PageOp::Load(_)) {
        match call.node_url.as_deref() {
            Some(url) if session.url() != Some(url) => session.load(url)?,
            None if session.url().is_none() => return Err(BrowserRenderError::NotLoaded),
            _ => {}
        }
    }
    let reply = match op {
        PageOp::Load(url) => {
            session.load(url)?;
            loaded(session, url)
        }
        PageOp::Screenshot(target) => PageReply::Png(session.screenshot(target)?),
        PageOp::Element(selector) => PageReply::Json(session.element(selector)?),
        PageOp::Cdp(method, params) => PageReply::Json(session.cdp(method, params.clone())?),
        PageOp::Extract(selector) => {
            match session.evaluate(&extractor_script(selector.as_deref()))? {
                Value::String(snapshot) => PageReply::Snapshot(snapshot),
                _ => {
                    return Err(BrowserRenderError::Protocol {
                        method: "import-to-canvas".into(),
                        message: "the page snapshot came back empty".into(),
                    })
                }
            }
        }
    };
    let changes_page = matches!(op, PageOp::Load(_) | PageOp::Cdp(..));
    let canvas = match call.canvas_scale.filter(|_| changes_page) {
        // Best effort: the call itself succeeded.
        Some(scale) => session.view_snapshot(scale).ok(),
        None => None,
    };
    Ok(PageResult { reply, canvas })
}

fn loaded(session: &mut PageSession, url: &str) -> PageReply {
    let title = session
        .evaluate("document.title")
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_default();
    PageReply::Loaded {
        url: url.to_string(),
        title,
    }
}

/// The snapshot extractor (`de_html::snapshot::SNAPSHOT_EXTRACTOR_JS`, the
/// Chrome extension's capture engine) wrapped to run headless: its
/// download and clipboard hand-off are caught and the JSON is returned
/// instead, rooted at `selector` when given.
pub(crate) fn extractor_script(selector: Option<&str>) -> String {
    let selector = selector.map_or("null".to_string(), |s| {
        serde_json::to_string(s).unwrap_or_else(|_| "null".into())
    });
    format!(
        r#"(async () => {{
  const selector = {selector};
  const root = selector ? document.querySelector(selector) : null;
  if (selector && !root) throw new Error("no element matches " + selector);
  globalThis.designEditorSnapshotOptions = root ? {{ root }} : undefined;
  let captured = null;
  const createObjectURL = URL.createObjectURL;
  const click = HTMLAnchorElement.prototype.click;
  const clipboard = navigator.clipboard;
  const writeText = clipboard && clipboard.writeText;
  URL.createObjectURL = (blob) => {{ captured = blob; return "blob:design-editor"; }};
  HTMLAnchorElement.prototype.click = function () {{}};
  if (clipboard) clipboard.writeText = () => Promise.resolve();
  try {{
{extractor}
  }} finally {{
    URL.createObjectURL = createObjectURL;
    HTMLAnchorElement.prototype.click = click;
    if (clipboard) clipboard.writeText = writeText;
    delete globalThis.designEditorSnapshotOptions;
  }}
  if (!captured) throw new Error("the snapshot extractor produced nothing");
  return await captured.text();
}})()"#,
        extractor = de_html::snapshot::SNAPSHOT_EXTRACTOR_JS,
    )
}
