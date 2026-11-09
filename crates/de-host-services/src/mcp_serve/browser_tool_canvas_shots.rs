//! Canvases an `import-to-canvas` cannot read from inside the page (#323).
//! A WebGL / WebGPU canvas without `preserveDrawingBuffer` hands the
//! snapshot extractor a blank PNG, and a tainted 2D canvas none at all; the
//! extractor tags such a canvas `canvasShot: {id, x, y, w, h}` (document
//! CSS px). Here each tagged canvas gets a clipped screenshot of the page as
//! its image before the snapshot is imported. A shot that fails or does not
//! fit the caps leaves the extractor's own image and becomes a warning; the
//! import goes on. The Chrome extension has no screenshot, so its tagged
//! canvases import as before.

use std::collections::BTreeMap;

use base64::Engine as _;
use de_browser_render::ShotTarget;
use de_mcp::ToolOutcome;
use serde_json::{Map, Value};

/// Most canvases one import screenshots.
pub(crate) const MAX_CANVAS_SHOTS: usize = 16;
/// Most data-URL bytes the shots add to one snapshot.
pub(crate) const MAX_CANVAS_SHOT_BYTES: usize = 24 * 1024 * 1024;

const SHOT_KEY: &str = "canvasShot";
const DATA_URL_PREFIX: &str = "data:image/png;base64,";

/// What the shots did, for the tool's result.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct CanvasShotReport {
    /// Canvases the extractor tagged.
    pub tagged: usize,
    /// Of those, the ones that now show the screenshot.
    pub captured: usize,
    /// One line per canvas (or group past the cap) left as the page read it.
    pub warnings: Vec<String>,
}

impl CanvasShotReport {
    /// Add the report to an import's result: `canvasShots` (captured of
    /// tagged) and its lines after the import's own `warnings`.
    pub(crate) fn write(self, outcome: ToolOutcome) -> ToolOutcome {
        let ToolOutcome::OkWithCommand(mut output, command) = outcome else {
            return outcome;
        };
        if self.tagged == 0 {
            return ToolOutcome::OkWithCommand(output, command);
        }
        output.insert(
            "canvasShots".into(),
            format!("{} of {}", self.captured, self.tagged),
        );
        if !self.warnings.is_empty() {
            let mut lines: Vec<String> = output
                .remove("warnings")
                .map(|text| text.lines().map(str::to_string).collect())
                .unwrap_or_default();
            lines.extend(self.warnings);
            output.insert("warnings".into(), lines.join("\n"));
        }
        ToolOutcome::OkWithCommand(output, command)
    }
}

/// One tagged canvas.
#[derive(Debug, Clone, PartialEq)]
struct Tagged {
    id: u64,
    target: ShotTarget,
}

/// Put `shoot`'s PNG into every canvas `snapshot` tags, as its `src`. A
/// snapshot without tags comes back unchanged without being parsed.
pub(crate) fn patch_canvas_shots(
    snapshot: String,
    shoot: &mut dyn FnMut(ShotTarget) -> Result<Vec<u8>, String>,
) -> (String, CanvasShotReport) {
    let mut report = CanvasShotReport::default();
    if !snapshot.contains(&format!("\"{SHOT_KEY}\"")) {
        return (snapshot, report);
    }
    let Ok(mut value) = serde_json::from_str::<Value>(&snapshot) else {
        // The importer reports the broken JSON itself.
        return (snapshot, report);
    };
    let mut tagged = Vec::new();
    collect(&value, &mut tagged);
    report.tagged = tagged.len();
    if tagged.is_empty() {
        return (snapshot, report);
    }
    // The patched snapshot must still fit what the importer reads.
    let mut budget = MAX_CANVAS_SHOT_BYTES
        .min(de_html::snapshot::MAX_SNAPSHOT_BYTES.saturating_sub(snapshot.len()));
    let mut images = BTreeMap::new();
    for canvas in tagged.iter().take(MAX_CANVAS_SHOTS) {
        let png = match shoot(canvas.target.clone()) {
            Ok(png) => png,
            Err(error) => {
                report.warnings.push(format!(
                    "canvas {} could not be captured ({error}); it keeps the page's own read-back",
                    canvas.id
                ));
                continue;
            }
        };
        let url = format!(
            "{DATA_URL_PREFIX}{}",
            base64::engine::general_purpose::STANDARD.encode(png)
        );
        if url.len() > budget {
            report.warnings.push(format!(
                "canvas {} is too large to embed; it keeps the page's own read-back",
                canvas.id
            ));
            continue;
        }
        budget -= url.len();
        images.insert(canvas.id, url);
    }
    if tagged.len() > MAX_CANVAS_SHOTS {
        report.warnings.push(format!(
            "{} more canvases were not captured (at most {MAX_CANVAS_SHOTS} per import); \
             they keep the page's own read-back",
            tagged.len() - MAX_CANVAS_SHOTS
        ));
    }
    report.captured = images.len();
    apply(&mut value, &images);
    let patched = serde_json::to_string(&value).unwrap_or(snapshot);
    (patched, report)
}

/// Every tagged canvas under `value`, in document order.
fn collect(value: &Value, out: &mut Vec<Tagged>) {
    match value {
        Value::Object(node) => {
            if let Some(tagged) = tagged(node) {
                out.push(tagged);
            }
            for child in node.values() {
                collect(child, out);
            }
        }
        Value::Array(items) => items.iter().for_each(|item| collect(item, out)),
        _ => {}
    }
}

fn tagged(node: &Map<String, Value>) -> Option<Tagged> {
    let shot = node.get(SHOT_KEY)?.as_object()?;
    let number = |key: &str| shot.get(key).and_then(Value::as_f64);
    Some(Tagged {
        id: shot.get("id")?.as_u64()?,
        target: ShotTarget::Rect {
            x: number("x")?,
            y: number("y")?,
            w: number("w")?,
            h: number("h")?,
        },
    })
}

/// Give each captured canvas its image (no longer tainted: the shot is
/// what the page showed) and drop every tag.
fn apply(value: &mut Value, images: &BTreeMap<u64, String>) {
    match value {
        Value::Object(node) => {
            let shot = node.remove(SHOT_KEY);
            let id = shot
                .as_ref()
                .and_then(|s| s.get("id"))
                .and_then(Value::as_u64);
            if let Some(url) = id.and_then(|id| images.get(&id)) {
                node.insert("src".into(), Value::String(url.clone()));
                node.remove("tainted");
            }
            for child in node.values_mut() {
                apply(child, images);
            }
        }
        Value::Array(items) => items.iter_mut().for_each(|item| apply(item, images)),
        _ => {}
    }
}

#[cfg(test)]
#[path = "browser_tool_canvas_shots_tests.rs"]
mod tests;
