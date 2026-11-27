//! What a tool call reads, so a live host can snapshot only that (#456).
//!
//! [`for_call`] is an allowlist: a tool listed here is read-only and its
//! result depends on the page it names (or the active page) plus what
//! instances on it resolve against, or on no page at all. Everything else —
//! every write, every read that walks the document (`get_node`, `find_nodes`,
//! `get_document_info`, exports, lint) — gets [`SnapshotPages::All`], the
//! snapshot every call had before. A host that narrows a snapshot must also
//! refuse writes from it and retry with the whole document
//! (`de-host-services` `mcp_live`), so an unlisted write path can never act
//! on a partial document.
//!
//! `de-host-services/src/mcp_live_pages_tests.rs` runs each listed call on
//! the whole and on the narrowed state and requires byte-identical replies,
//! and checks that every tool listed here has a call there.

use std::collections::BTreeMap;

pub use de_editor_core::request_snapshot_pages::SnapshotPages;

/// Read-only tools that touch no page's nodes.
pub const NO_NODE_TOOLS: &[&str] = &[
    "get_history_depth",
    "get_viewport",
    "get_variables",
    "list_variables",
    "get_active_theme",
];

/// Read-only tools that read one page: `pageId` (`page_id`, `page`), else
/// the active page.
pub const PAGE_TOOLS: &[&str] = &[
    "snapshot_layout",
    "read_nodes",
    "batch_get",
    "find_empty_space",
];

/// The `execute` API calls that write. A script naming none of them only
/// reads; one that does gets the whole document.
const EXECUTE_WRITES: &[&str] = &[
    "Insert",
    "Copy",
    "Replace",
    "Update",
    "Move",
    "Delete",
    "Generate",
    "SetVariables",
];

/// What the call `tool` with `args` reads.
pub fn for_call(tool: &str, args: &BTreeMap<String, String>) -> SnapshotPages {
    if NO_NODE_TOOLS.contains(&tool) {
        return SnapshotPages::NoNodes;
    }
    if PAGE_TOOLS.contains(&tool) {
        return SnapshotPages::Page(page_arg(args));
    }
    match tool {
        // Renders a node of the active page.
        "get_screenshot" => SnapshotPages::Page(None),
        "execute" => match args.get("script") {
            Some(script) if !names_a_write(script) => SnapshotPages::Page(page_arg(args)),
            _ => SnapshotPages::All,
        },
        _ => SnapshotPages::All,
    }
}

/// What the JSON-RPC request `line` reads; [`SnapshotPages::All`] for
/// anything that is not a parsable tool call.
pub fn for_request(line: &str) -> SnapshotPages {
    // A line that names no listed tool is not parsed twice: writes carry
    // the large payloads.
    let listed = NO_NODE_TOOLS
        .iter()
        .chain(PAGE_TOOLS)
        .chain(&["get_screenshot", "execute"])
        .any(|tool| line.contains(tool));
    if !listed {
        return SnapshotPages::All;
    }
    match crate::parse_tool_call(line) {
        Some(call) => for_call(&call.tool, &call.arguments),
        None => SnapshotPages::All,
    }
}

fn page_arg(args: &BTreeMap<String, String>) -> Option<String> {
    ["pageId", "page_id", "page"]
        .iter()
        .find_map(|key| args.get(*key))
        .cloned()
}

/// Whether `script` names a write call as an identifier (`Insert(`,
/// `Insert (`, `const f = Insert`): any whole-word occurrence counts, so a
/// read-only verdict errs on the whole document.
fn names_a_write(script: &str) -> bool {
    let bytes = script.as_bytes();
    EXECUTE_WRITES.iter().any(|name| {
        script.match_indices(name).any(|(at, _)| {
            let before = at.checked_sub(1).map(|i| bytes[i]);
            let after = bytes.get(at + name.len()).copied();
            !before.is_some_and(is_ident_byte) && !after.is_some_and(is_ident_byte)
        })
    })
}

fn is_ident_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'$'
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn reads_name_their_page_and_everything_else_takes_the_document() {
        assert_eq!(
            for_call("get_history_depth", &args(&[])),
            SnapshotPages::NoNodes
        );
        assert_eq!(
            for_call("snapshot_layout", &args(&[])),
            SnapshotPages::Page(None)
        );
        assert_eq!(
            for_call("read_nodes", &args(&[("pageId", "p2")])),
            SnapshotPages::Page(Some("p2".into()))
        );
        assert_eq!(
            for_call("get_screenshot", &args(&[("nodeId", "n")])),
            SnapshotPages::Page(None)
        );
        for tool in [
            "get_node",
            "find_nodes",
            "update_node",
            "batch_design",
            "lint_document",
        ] {
            assert_eq!(for_call(tool, &args(&[])), SnapshotPages::All, "{tool}");
        }
    }

    #[test]
    fn an_execute_script_that_names_a_write_takes_the_document() {
        let read = "const n = Get(\"a\", {depth: 1}); Print(n.name, Updated);";
        assert_eq!(
            for_call("execute", &args(&[("script", read)])),
            SnapshotPages::Page(None),
            "Updated is not Update"
        );
        for script in [
            "Insert(document, {type: 'frame'})",
            "const w = Update; w('a', {})",
            "x.Delete ('a')",
            "SetVariables({})",
        ] {
            assert_eq!(
                for_call("execute", &args(&[("script", script), ("pageId", "p")])),
                SnapshotPages::All,
                "{script}"
            );
        }
        assert_eq!(for_call("execute", &args(&[])), SnapshotPages::All);
    }
}
