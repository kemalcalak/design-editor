//! MCP tool `capture_preview` (#156) — enter Preview headlessly, run a
//! list of taps / hovers / navigations, and return a PNG of the screen
//! being shown.
//!
//! The editor's own preview is never touched: the tool builds a fresh
//! `PreviewSession` from the document snapshot taken at registration
//! (sessions are `!Send`, so it is built inside `call`), drives it with
//! `de_preview_core`'s scripted steps, renders the painted scene through
//! the same raster core `get_screenshot` uses, and drops it. The preview
//! runtime never writes back, so the document is left exactly as it was.

use std::collections::BTreeMap;
use std::rc::Rc;

use base64::Engine as _;
use de_editor_core::EditorState;
use de_mcp::{McpTool, ToolErrorCode, ToolOutcome};
use de_preview_core::{CaptureStep, PreviewSession};
use jian_ops_schema::PenDocument;

use crate::export::{render_node_on_page_raster_bytes, RasterFormat};

/// The canvas size handed to the session. Preview layout is solved per
/// root from each frame's authored size, so this only seeds the API.
const CANVAS_SIZE: (f32, f32) = (1440.0, 900.0);

pub struct CapturePreview {
    document: PenDocument,
    active_theme: BTreeMap<String, String>,
    active_page_index: usize,
    preserve_authored_geometry: bool,
}

impl McpTool for CapturePreview {
    fn name(&self) -> &str {
        "capture_preview"
    }

    fn call(&self, args: &BTreeMap<String, String>) -> ToolOutcome {
        let _ = de_editor_ui::set_brand_catalog(crate::web_static::ICONIFY_BRANDS_JSON);
        match self.capture(args) {
            Ok(outcome) => outcome,
            Err((code, message)) => ToolOutcome::Err(code, message),
        }
    }
}

type ToolError = (ToolErrorCode, String);

impl CapturePreview {
    fn capture(&self, args: &BTreeMap<String, String>) -> Result<ToolOutcome, ToolError> {
        let page_index = self.page_index(args.get("pageId").map(String::as_str))?;
        let mut steps = Vec::new();
        if let Some(screen) = args
            .get("screen")
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
        {
            steps.push(CaptureStep::Navigate(screen.to_string()));
        }
        if let Some(raw) = args.get("actions") {
            steps.extend(parse_actions(raw)?);
        }
        let then_ms = de_mcp::parse_time_ms_arg(args)?
            .map(|ms| ms.round() as u64)
            .unwrap_or(0);

        let mut session = PreviewSession::enter_with_host_motion_preference(
            &self.document,
            CANVAS_SIZE,
            &self.active_theme,
            page_index,
            self.preserve_authored_geometry,
            false,
            Rc::new(jian_skia::SkiaMeasure::new()),
            de_preview_core::PreviewHostCapabilities::none(),
            de_preview_core::MotionPreference::Full,
            0,
        )
        .map_err(|error| (ToolErrorCode::ToolFailed, format!("preview: {error}")))?;
        session.begin_lifecycle(0);
        let scene = session
            .run_capture_script(&steps, then_ms)
            .map_err(|error| (ToolErrorCode::InvalidArgument, error.to_string()))?;
        let clock_ms = session.now_ms();

        let page = scene
            .active_page()
            .ok_or_else(|| (ToolErrorCode::ToolFailed, "preview has no page".to_string()))?;
        let node_id = match args
            .get("nodeId")
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
        {
            Some(id) => id.to_string(),
            None => page
                .children
                .first()
                .map(|node| node.id.clone())
                .ok_or_else(|| {
                    (
                        ToolErrorCode::ToolFailed,
                        "preview shows nothing".to_string(),
                    )
                })?,
        };
        let bytes = crate::export::with_capture_time_ms(clock_ms as f64, || {
            render_node_on_page_raster_bytes(page, &node_id, RasterFormat::Png, 1.0)
        })
        .map_err(|error| (ToolErrorCode::ToolFailed, error.to_string()))?;
        let image_base64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
        let metadata = serde_json::json!({
            "nodeId": node_id,
            "format": "png",
            "screen": session.current_path(),
            "elapsedMs": clock_ms,
            "steps": steps.len(),
            "warnings": session.warnings(),
            "image_base64": image_base64,
        });
        Ok(ToolOutcome::OkImageContent {
            image_base64,
            mime_type: "image/png".into(),
            metadata_json: Some(metadata.to_string()),
        })
    }

    /// The page to preview: `pageId` (a page id or name), else the
    /// active page.
    fn page_index(&self, page_id: Option<&str>) -> Result<usize, ToolError> {
        let Some(wanted) = page_id.map(str::trim).filter(|id| !id.is_empty()) else {
            return Ok(self.active_page_index);
        };
        self.document
            .pages
            .iter()
            .flatten()
            .position(|page| page.id == wanted || page.name == wanted)
            .ok_or_else(|| {
                (
                    ToolErrorCode::InvalidArgument,
                    format!("no page with id or name `{wanted}`"),
                )
            })
    }
}

/// Parse the `actions` array: `{"tap":"<nodeId>"}`, `{"hover":"<nodeId>"}`,
/// `{"navigate":"/path"}`, `{"back":true}` or `{"wait":<ms>}`, in order.
fn parse_actions(raw: &str) -> Result<Vec<CaptureStep>, ToolError> {
    let invalid = |message: String| (ToolErrorCode::InvalidArgument, message);
    let value: serde_json::Value = serde_json::from_str(raw.trim())
        .map_err(|error| invalid(format!("actions must be a JSON array: {error}")))?;
    let items = value
        .as_array()
        .ok_or_else(|| invalid("actions must be a JSON array".to_string()))?;
    items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let object = item.as_object().filter(|object| object.len() == 1);
            let Some((key, body)) = object.and_then(|object| object.iter().next()) else {
                return Err(invalid(format!(
                    "actions[{index}] must be an object with one of tap, hover, navigate, back, wait"
                )));
            };
            let text = || {
                body.as_str()
                    .map(str::trim)
                    .filter(|text| !text.is_empty())
                    .map(str::to_string)
                    .ok_or_else(|| invalid(format!("actions[{index}].{key} must be a string")))
            };
            Ok(match key.as_str() {
                "tap" => CaptureStep::Tap(text()?),
                "hover" => CaptureStep::Hover(text()?),
                "navigate" => CaptureStep::Navigate(text()?),
                "back" => CaptureStep::Back,
                "wait" => CaptureStep::Wait(
                    body.as_f64()
                        .filter(|ms| ms.is_finite() && *ms >= 0.0)
                        .map(|ms| ms.round() as u64)
                        .ok_or_else(|| {
                            invalid(format!("actions[{index}].wait must be milliseconds"))
                        })?,
                ),
                other => {
                    return Err(invalid(format!(
                        "actions[{index}]: unknown action `{other}` (tap, hover, navigate, back, wait)"
                    )))
                }
            })
        })
        .collect()
}

/// Snapshot constructor — copies what a preview session needs from the
/// live editor state at registration time.
pub fn capture_preview_snapshot(state: &EditorState) -> CapturePreview {
    CapturePreview {
        document: state.doc.clone(),
        active_theme: state.ui.variables.active_theme.clone(),
        active_page_index: state.ui.active_page_index,
        preserve_authored_geometry: state.editor_ui.preserve_authored_geometry,
    }
}

#[cfg(all(test, not(target_os = "windows")))]
#[path = "capture_preview_tool_tests.rs"]
mod tests;
