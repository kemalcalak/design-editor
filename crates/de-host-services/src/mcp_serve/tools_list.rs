//! The `tools/list` catalog response.
//!
//! Split out of the `mcp_serve` spine at the 800-line cap.

use de_editor_core::EditorState;

#[cfg(feature = "mcp-debug-tools")]
use super::schemas::DEBUG_TOOL_SCHEMAS;
use super::schemas::TOOL_SCHEMAS;

/// `TOOL_SCHEMAS` as this MCP server describes them: with the root-placement
/// wording (#37) the shared catalog leaves out because the in-app design
/// agent, which also reads it, does not place roots. Built once; `tools/list`
/// and `ToolSearch` both serve it.
pub(super) fn mcp_tool_schemas() -> &'static [&'static str] {
    static SCHEMAS: std::sync::OnceLock<Vec<&'static str>> = std::sync::OnceLock::new();
    SCHEMAS.get_or_init(|| {
        TOOL_SCHEMAS
            .iter()
            .map(|schema| {
                let advertised = de_mcp::root_placement::advertise_root_placement(schema);
                &*Box::leak(advertised.into_boxed_str())
            })
            .collect()
    })
}

pub(super) fn tools_list_response(
    id_raw: &str,
    state: &EditorState,
    debug_enabled: bool,
) -> String {
    let mut entries: Vec<String> = mcp_tool_schemas()
        .iter()
        .map(|s| (*s).to_string())
        .collect();
    entries.extend(
        de_mcp::element_tools::element_tool_schemas(state)
            .iter()
            .map(|schema| de_mcp::root_placement::advertise_root_placement(schema)),
    );
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
