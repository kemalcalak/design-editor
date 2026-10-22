//! The `tools/list` catalog response.
//!
//! Split out of the `mcp_serve` spine at the 800-line cap.

use de_editor_core::EditorState;

#[cfg(feature = "mcp-debug-tools")]
use super::schemas::DEBUG_TOOL_SCHEMAS;
use super::schemas::TOOL_SCHEMAS;

pub(super) fn tools_list_response(
    id_raw: &str,
    state: &EditorState,
    debug_enabled: bool,
) -> String {
    let mut entries: Vec<String> = TOOL_SCHEMAS.iter().map(|s| (*s).to_string()).collect();
    entries.extend(de_mcp::element_tools::element_tool_schemas(state));
    #[cfg(not(feature = "mcp-debug-tools"))]
    let _ = debug_enabled;
    #[cfg(feature = "mcp-debug-tools")]
    if debug_enabled {
        entries.extend(DEBUG_TOOL_SCHEMAS.iter().map(|s| (*s).to_string()));
    }
    format!(
        r#"{{"jsonrpc":"2.0","id":{id_raw},"result":{{"tools":[{}]}}}}"#,
        entries.join(",")
    )
}
