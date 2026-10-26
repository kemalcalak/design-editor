//! Claude artifact links (`claude.ai/artifact/<id>`) for `import_html_url`.
//!
//! The link serves a script shell, not the artifact: the shell loads the
//! artifact's HTML into a frame on `*.claudeusercontent.com`. A plain fetch
//! therefore imports an empty shell, so these links open in the local
//! headless browser (`de-browser-render`), and the importer reads the HTML of
//! the frame the artifact rendered into. Only a publicly shared artifact
//! opens without the user's Claude sign-in; anything else reports
//! [`ImportHtmlUrlError::ArtifactUnavailable`] and points at the saved-HTML
//! route, which the HTML file import already handles.

use std::time::{Duration, Instant};

use de_browser_render::{BrowserRenderError, PageSession, PageSessions, Viewport};
use serde_json::{json, Value};

use crate::import_html_url_error::ImportHtmlUrlError;

/// The tab used to open artifact links; closed after each import.
const SESSION_KEY: &str = "import-html-url:claude-artifact";
/// How long the shell gets to render the artifact.
const LOAD_DEADLINE: Duration = Duration::from_secs(20);
const POLL_INTERVAL: Duration = Duration::from_millis(500);
/// The host the shell renders artifact content on.
const CONTENT_HOST_SUFFIX: &str = "claudeusercontent.com";
const OUTER_HTML: &str = "document.documentElement ? document.documentElement.outerHTML : ''";
const MAIN_TEXT: &str = "document.body ? document.body.innerText.slice(0, 2000) : ''";

/// What the artifact rendered: its HTML and the address of the frame it
/// came from, which relative resources resolve against.
pub(crate) struct ArtifactPage {
    pub html: String,
    pub base_url: Option<String>,
}

/// A Claude artifact link: `claude.ai/artifact/<id>`,
/// `claude.ai/code/artifact/<id>`, `claude.ai/public/artifacts/<id>`, or
/// the older `claude.site/artifacts/<id>`.
pub(crate) fn is_claude_artifact(url: &reqwest::Url) -> bool {
    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    let segments: Vec<&str> = url
        .path_segments()
        .map(|segments| segments.filter(|segment| !segment.is_empty()).collect())
        .unwrap_or_default();
    match host.trim_start_matches("www.") {
        "claude.ai" => matches!(
            segments.as_slice(),
            ["artifact", _] | ["code", "artifact", _] | ["public", "artifacts", _]
        ),
        "claude.site" => matches!(segments.as_slice(), ["artifacts", _, ..]),
        _ => false,
    }
}

/// Open `url` in the local browser and read the artifact it renders.
pub(crate) fn read_artifact(url: &str) -> Result<ArtifactPage, ImportHtmlUrlError> {
    let viewport = Viewport {
        width: 1440,
        height: 900,
        scale: 1.0,
        mobile: false,
    };
    let sessions = PageSessions::shared();
    let outcome = sessions.with(SESSION_KEY, viewport, |session| read_in(session, url));
    sessions.close(SESSION_KEY);
    match outcome {
        Ok(Some(page)) => Ok(page),
        Ok(None) => Err(ImportHtmlUrlError::ArtifactUnavailable),
        Err(error) => Err(ImportHtmlUrlError::ArtifactBrowser(error.to_string())),
    }
}

fn read_in(
    session: &mut PageSession,
    url: &str,
) -> Result<Option<ArtifactPage>, BrowserRenderError> {
    session.load(url)?;
    let deadline = Instant::now() + LOAD_DEADLINE;
    let mut frame_seen = None;
    // A script artifact paints a placeholder before it mounts, so the HTML
    // is taken once two reads in a row agree.
    let mut last_read: Option<String> = None;
    loop {
        let tree = session.cdp("Page.getFrameTree", json!({}))?;
        if let Some(frame) = content_frame(&tree) {
            match frame_html(session, &frame.id) {
                Ok(html) if has_body_content(&html) => {
                    if last_read.as_deref() == Some(html.as_str()) {
                        return Ok(Some(ArtifactPage {
                            html,
                            base_url: frame.base_url,
                        }));
                    }
                    last_read = Some(html);
                }
                _ => last_read = None,
            }
            frame_seen = Some(frame);
        }
        let main_text = session.evaluate(MAIN_TEXT)?;
        if main_text
            .as_str()
            .is_some_and(|text| text.to_ascii_lowercase().contains("page not found"))
            || Instant::now() >= deadline
        {
            break;
        }
        std::thread::sleep(POLL_INTERVAL);
    }
    // A frame running in another renderer process (site isolation) cannot
    // be read from the shell's tab; its own address still renders it.
    let Some(base_url) = frame_seen.and_then(|frame| frame.base_url) else {
        return Ok(None);
    };
    session.load(&base_url)?;
    let html = session.evaluate(OUTER_HTML)?;
    Ok(html
        .as_str()
        .filter(|html| has_body_content(html))
        .map(|html| ArtifactPage {
            html: html.to_string(),
            base_url: Some(base_url.clone()),
        }))
}

/// The HTML of one frame, read through an isolated world so the page's own
/// scripts are not involved.
fn frame_html(session: &mut PageSession, frame_id: &str) -> Result<String, BrowserRenderError> {
    let world = session.cdp(
        "Page.createIsolatedWorld",
        json!({ "frameId": frame_id, "worldName": "design-editor-import" }),
    )?;
    let context = world
        .get("executionContextId")
        .cloned()
        .unwrap_or(Value::Null);
    let reply = session.cdp(
        "Runtime.evaluate",
        json!({ "expression": OUTER_HTML, "contextId": context, "returnByValue": true }),
    )?;
    Ok(reply
        .pointer("/result/value")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string())
}

#[derive(Debug, PartialEq)]
pub(crate) struct ContentFrame {
    pub id: String,
    /// The nearest http(s) address at or above the frame (a `srcdoc`
    /// frame has none of its own).
    pub base_url: Option<String>,
}

/// The deepest frame at or under a `*.claudeusercontent.com` frame in a
/// `Page.getFrameTree` reply, not counting frames from other hosts the
/// artifact embeds: the shell may nest the artifact one frame further down
/// (a `srcdoc` document).
pub(crate) fn content_frame(reply: &Value) -> Option<ContentFrame> {
    fn walk(
        node: &Value,
        depth: usize,
        inside: bool,
        base: Option<&str>,
        best: &mut Option<(usize, ContentFrame)>,
    ) {
        let url = node
            .pointer("/frame/url")
            .and_then(Value::as_str)
            .unwrap_or("");
        let is_http = url.starts_with("https://") || url.starts_with("http://");
        let on_content_host = is_http && host_of(url).ends_with(CONTENT_HOST_SUFFIX);
        // Below the content host, only its own documents (`srcdoc`,
        // `about:`) are the artifact; an http frame on another host is
        // something the artifact embeds (a video, a map).
        if inside && is_http && !on_content_host {
            return;
        }
        let inside = inside || on_content_host;
        let base = if is_http { Some(url) } else { base };
        if inside && best.as_ref().is_none_or(|(deepest, _)| depth > *deepest) {
            if let Some(id) = node.pointer("/frame/id").and_then(Value::as_str) {
                *best = Some((
                    depth,
                    ContentFrame {
                        id: id.to_string(),
                        base_url: base.map(str::to_string),
                    },
                ));
            }
        }
        for child in node
            .get("childFrames")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            walk(child, depth + 1, inside, base, best);
        }
    }
    let root = reply.get("frameTree")?;
    let mut best = None;
    for child in root
        .get("childFrames")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        walk(child, 1, false, None, &mut best);
    }
    best.map(|(_, frame)| frame)
}

fn host_of(url: &str) -> String {
    reqwest::Url::parse(url)
        .ok()
        .and_then(|url| url.host_str().map(str::to_ascii_lowercase))
        .unwrap_or_default()
}

/// The document has rendered something: its `<body>` holds more than
/// whitespace.
pub(crate) fn has_body_content(html: &str) -> bool {
    let lower = html.to_ascii_lowercase();
    let Some(open) = lower.find("<body") else {
        return false;
    };
    let Some(start) = lower[open..].find('>').map(|end| open + end + 1) else {
        return false;
    };
    let end = lower
        .rfind("</body>")
        .filter(|end| *end >= start)
        .unwrap_or(lower.len());
    !lower[start..end].trim().is_empty()
}

#[cfg(test)]
#[path = "claude_artifact_tests.rs"]
mod tests;
