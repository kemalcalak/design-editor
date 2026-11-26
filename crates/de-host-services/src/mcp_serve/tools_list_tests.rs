//! The wire parser refuses array / object arguments it does not know a
//! tool takes (`de_mcp::parser::structured_arg_allowed`). That list lives in
//! `de-mcp`, below the schema catalog, so it cannot read the schemas
//! itself; this test keeps the two from drifting (#430).

use de_mcp::element_tools::KitToolCatalog;
use de_mcp::parser::structured_arg_allowed;
use serde_json::Value;

use super::build_tools;

/// `true` when a property's `type` is `array` / `object`, or a type list
/// naming either.
fn is_structured(property: &Value) -> bool {
    let structured = |ty: &Value| matches!(ty.as_str(), Some("array" | "object"));
    match &property["type"] {
        Value::Array(types) => types.iter().any(structured),
        ty => structured(ty),
    }
}

#[test]
fn every_structured_schema_property_passes_the_wire_parser() {
    // Debug tools are included when the feature builds them in.
    let tools: Value =
        serde_json::from_str(&build_tools(KitToolCatalog::builtin(), true)).expect("tools json");
    let tools = tools.as_array().expect("tools array");
    assert!(!tools.is_empty());
    let mut refused = Vec::new();
    for tool in tools {
        let name = tool["name"].as_str().expect("tool name");
        let Some(properties) = tool["inputSchema"]["properties"].as_object() else {
            continue;
        };
        for (key, property) in properties {
            if is_structured(property) && !structured_arg_allowed(name, key) {
                refused.push(format!("{name}.{key}"));
            }
        }
    }
    assert!(
        refused.is_empty(),
        "schema-declared array/object arguments the MCP parser refuses; \
         add them to de_mcp::parser::structured_arg_allowed: {refused:?}"
    );
}

#[test]
fn a_type_list_naming_array_counts_as_structured() {
    assert!(is_structured(
        &serde_json::json!({"type": ["string", "array"]})
    ));
    assert!(is_structured(&serde_json::json!({"type": "object"})));
    assert!(!is_structured(&serde_json::json!({"type": "string"})));
}
