//! The debug-tool catalog (#456): split out of `tests.rs` at the 800-line
//! cap.

/// Every debug tool, listed only behind the debug gate.
pub(super) const DEBUG_TOOL_NAMES: [&str; 4] = [
    "debug_validation_report",
    "debug_logs_tail",
    "debug_screenshot",
    "debug_memory_report",
];

/// A debug tool added without a name here (or a name without a schema)
/// trips this before the catalog tests miss it.
#[cfg(feature = "mcp-debug-tools")]
#[test]
fn every_debug_schema_has_a_name_here() {
    let schemas = crate::mcp_serve::DEBUG_TOOL_SCHEMAS;
    assert_eq!(DEBUG_TOOL_NAMES.len(), schemas.len());
    for name in DEBUG_TOOL_NAMES {
        let named = format!(r#""name":"{name}""#);
        assert!(
            schemas.iter().any(|schema| schema.contains(&named)),
            "{name} has a schema"
        );
    }
}
