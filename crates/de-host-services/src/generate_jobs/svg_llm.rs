//! `generate type:"svg"`: the user's built-in chat model writes the SVG, and
//! the landing imports it through the canvas SVG importer
//! (`de_editor_core::svg_import::svg_nodes_fitted`) into the target frame.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use de_ai::chat_provider::{ChatDelta, ChatProvider, ChatRequest};
use de_editor_core::agent_settings::{AgentSettings, BuiltinAgentConfig};

use super::GenerateJobError;
use crate::chat_builtin_http::ConfiguredBuiltinProvider;

/// Wall-clock budget of one SVG answer (pen.dev: "SVGs take minutes").
pub const SVG_TIMEOUT: Duration = Duration::from_secs(300);
/// Output budget: room for a detailed illustration, bounded.
const SVG_MAX_OUTPUT_TOKENS: u32 = 16_000;
/// Longest SVG accepted from the model.
const MAX_SVG_CHARS: usize = 512 * 1024;
/// Most nodes one piece of artwork may land as (the tracer caps its own
/// paths below this); more would stall the editor that lands it.
pub const MAX_ARTWORK_NODES: usize = 5000;

const SVG_SYSTEM_PROMPT: &str = "You are a vector illustrator. Answer with exactly one \
standalone SVG document and nothing else: no prose, no Markdown fence. Use a viewBox. Draw \
with <path>, <rect>, <circle>, <ellipse>, <polygon>, <polyline>, <line> and <g>, filled with \
solid colours (#rrggbb) and optional strokes. Do not use <text>, <image>, gradients, filters, \
masks, clip paths, <use>, CSS, scripts, transforms or external references. Keep it clean: a \
few dozen well-formed shapes, not hundreds.";

/// The first built-in chat agent ready to answer (key + model), as
/// `run_design_agent` picks it.
pub fn resolve_chat_agent(
    settings: &AgentSettings,
) -> Result<BuiltinAgentConfig, GenerateJobError> {
    settings
        .builtin_agents
        .iter()
        .find(|agent| agent.ready())
        .cloned()
        .ok_or(GenerateJobError::NoChatProvider)
}

/// The user turn for one drawing.
pub fn svg_user_prompt(prompt: &str, width: f64, height: f64) -> String {
    format!(
        "Draw: {prompt}\n\nThe drawing fills a {w} x {h} frame, so use viewBox=\"0 0 {w} {h}\".",
        w = width.round().max(1.0),
        h = height.round().max(1.0),
    )
}

/// Ask the chat model for the drawing and return the SVG text.
pub fn generate_svg_blocking(
    agent: &BuiltinAgentConfig,
    screen_endpoint: bool,
    prompt: &str,
    width: f64,
    height: f64,
) -> Result<String, GenerateJobError> {
    let provider = if screen_endpoint {
        ConfiguredBuiltinProvider::from_builtin_agent_for_web(agent)
    } else {
        ConfiguredBuiltinProvider::from_builtin_agent(agent)
    }
    .ok_or(GenerateJobError::NoChatProvider)?;
    let request = ChatRequest {
        system_prompt: SVG_SYSTEM_PROMPT.to_string(),
        user_message: svg_user_prompt(prompt, width, height),
        max_output_tokens: SVG_MAX_OUTPUT_TOKENS,
        ..Default::default()
    };
    let cancel = Arc::new(AtomicBool::new(false));
    let deadline = Instant::now() + SVG_TIMEOUT;
    let watchdog_cancel = Arc::clone(&cancel);
    let watchdog = std::thread::Builder::new()
        .name("de-generate-svg-deadline".into())
        .spawn(move || {
            while Instant::now() < deadline {
                if watchdog_cancel.load(Ordering::Acquire) {
                    return;
                }
                std::thread::sleep(Duration::from_millis(250));
            }
            watchdog_cancel.store(true, Ordering::Release);
        });
    let mut text = String::new();
    let mut failure = None;
    for delta in provider.send_cancellable(request, Arc::clone(&cancel)) {
        match delta {
            ChatDelta::TextDelta(chunk) => {
                text.push_str(&chunk);
                if text.len() > MAX_SVG_CHARS {
                    failure = Some(GenerateJobError::Chat("the drawing is too large".into()));
                    break;
                }
            }
            ChatDelta::Error(message) => {
                failure = Some(GenerateJobError::Chat(message));
                break;
            }
            ChatDelta::Done { .. } => break,
            ChatDelta::Thinking(_) | ChatDelta::ToolUse { .. } => {}
        }
        if Instant::now() >= deadline {
            failure = Some(GenerateJobError::TimedOut);
            break;
        }
    }
    // `swap` stops the watchdog; it returns true when the watchdog had
    // already cancelled the stream, i.e. the deadline passed.
    let timed_out = cancel.swap(true, Ordering::AcqRel);
    if let Ok(handle) = watchdog {
        let _ = handle.join();
    }
    // A complete drawing that raced the deadline still counts; anything
    // else after a timeout is reported as the timeout, not as the provider
    // error the cancelled stream may have produced.
    match (extract_svg(&text), timed_out, failure) {
        (Some(svg), _, None) => Ok(svg),
        (_, true, _) => Err(GenerateJobError::TimedOut),
        (_, false, Some(failure)) => Err(failure),
        (None, false, None) => Err(GenerateJobError::NoSvg),
    }
}

/// Check, on the worker, that `svg` lands as at least one and at most
/// [`MAX_ARTWORK_NODES`] nodes, so a job never reports done for an empty
/// frame and the landing never floods the editor. `empty` is the error for
/// a drawing with no importable shapes.
pub fn check_artwork(svg: String, empty: GenerateJobError) -> Result<String, GenerateJobError> {
    fn count(nodes: &[jian_ops_schema::node::PenNode]) -> usize {
        use de_editor_core::PenNodeExt as _;
        nodes
            .iter()
            .map(|node| 1 + node.children().map_or(0, |kids| count(kids)))
            .sum()
    }
    let nodes = de_editor_core::svg_import::svg_nodes_fitted(&svg, 100.0, 100.0);
    match count(&nodes) {
        0 => Err(empty),
        n if n > MAX_ARTWORK_NODES => Err(GenerateJobError::Trace(format!(
            "the artwork has {n} shapes, more than {MAX_ARTWORK_NODES}"
        ))),
        _ => Ok(svg),
    }
}

/// The `<svg …>…</svg>` element in a model answer, if any. Answers that try
/// to script or embed content are refused outright.
pub fn extract_svg(answer: &str) -> Option<String> {
    let lower = answer.to_ascii_lowercase();
    let start = lower.find("<svg")?;
    let end = lower.rfind("</svg>")? + "</svg>".len();
    if end <= start {
        return None;
    }
    let svg = &lower[start..end];
    if ["<script", "<foreignobject", "javascript:"]
        .iter()
        .any(|needle| svg.contains(needle))
    {
        return None;
    }
    Some(answer[start..end].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_svg_takes_the_drawing_out_of_prose_and_fences() {
        let answer = "Here you go:\n```svg\n<SVG viewBox=\"0 0 1 1\"><rect/></SVG>\n```";
        assert_eq!(
            extract_svg(answer).as_deref(),
            Some("<SVG viewBox=\"0 0 1 1\"><rect/></SVG>")
        );
        assert_eq!(extract_svg("no drawing today"), None);
    }

    #[test]
    fn extract_svg_refuses_scripted_answers() {
        assert_eq!(extract_svg("<svg><script>alert(1)</script></svg>"), None);
        assert_eq!(extract_svg("<svg><a href=\"javascript:x\"/></svg>"), None);
    }

    #[test]
    fn the_user_prompt_names_the_frame_box() {
        let prompt = svg_user_prompt("a fox", 400.4, 199.6);
        assert!(prompt.contains("a fox"));
        assert!(prompt.contains("viewBox=\"0 0 400 200\""), "{prompt}");
    }

    #[test]
    fn artwork_without_shapes_or_with_too_many_is_refused() {
        let empty = "<svg viewBox=\"0 0 10 10\"><text>hi</text></svg>".to_string();
        assert_eq!(
            check_artwork(empty, GenerateJobError::NoSvg),
            Err(GenerateJobError::NoSvg)
        );
        let one = "<svg viewBox=\"0 0 10 10\"><rect width=\"10\" height=\"10\"/></svg>".to_string();
        assert!(check_artwork(one, GenerateJobError::NoSvg).is_ok());
        let many = format!(
            "<svg viewBox=\"0 0 10 10\">{}</svg>",
            "<rect width=\"1\" height=\"1\"/>".repeat(MAX_ARTWORK_NODES + 1)
        );
        assert!(matches!(
            check_artwork(many, GenerateJobError::NoSvg),
            Err(GenerateJobError::Trace(_))
        ));
    }

    #[test]
    fn no_ready_agent_is_no_chat_provider() {
        assert_eq!(
            resolve_chat_agent(&AgentSettings::default()).map(|a| a.id),
            Err(GenerateJobError::NoChatProvider)
        );
    }
}
