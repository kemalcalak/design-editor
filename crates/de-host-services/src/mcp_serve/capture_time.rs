//! Shared `timeMs` handling for the screenshot / export tools (#8).
//!
//! Each tool renders through the export painter, whose shader clock is pinned
//! for the whole call. This wrapper validates `timeMs` once
//! (`de_mcp::parse_time_ms_arg`) and pins the clock to it around the tool's
//! own render, so no render signature has to carry the time.

use std::collections::BTreeMap;

use de_mcp::ToolOutcome;

/// Run `render` with the export shader clock at the call's `timeMs`
/// (`time = 0` when absent). An invalid `timeMs` fails the call before
/// anything renders. `render` receives the validated time so a tool can
/// echo it in its response.
pub(super) fn at_requested_time(
    args: &BTreeMap<String, String>,
    render: impl FnOnce(Option<f64>) -> ToolOutcome,
) -> ToolOutcome {
    match de_mcp::parse_time_ms_arg(args) {
        Err((code, message)) => ToolOutcome::Err(code, message),
        Ok(time_ms) => {
            crate::export::with_capture_time_ms(time_ms.unwrap_or(0.0), || render(time_ms))
        }
    }
}
