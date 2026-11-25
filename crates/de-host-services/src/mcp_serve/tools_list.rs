//! The `tools/list` catalog response.
//!
//! Split out of the `mcp_serve` spine at the 800-line cap.
//!
//! The catalog depends only on the kit components (the `insert_*` tools)
//! and the debug-tools gate, never on the document, so it is built once
//! per kit catalog and served from a cache (#406). That is what lets the
//! live server answer `tools/list` without its state lock or a UI-thread
//! snapshot, and keeps every transport from re-serializing ~130 KB of
//! schemas per request.

use std::sync::{Arc, Mutex, OnceLock};

use de_editor_core::EditorState;
use de_mcp::element_tools::KitToolCatalog;

#[cfg(feature = "mcp-debug-tools")]
use super::schemas::DEBUG_TOOL_SCHEMAS;
use super::schemas::TOOL_SCHEMAS;

/// Cached `tools` arrays kept at once. One per (kit catalog, debug gate)
/// pair; a process sees the built-in catalog plus the odd imported kit, so
/// this is a bound against churn, not a working set.
const CACHED_CATALOGS: usize = 4;

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

/// `tools/list` for the kits `state` holds.
pub(super) fn tools_list_response(
    id_raw: &str,
    state: &EditorState,
    debug_enabled: bool,
) -> String {
    let tools = match cached_tools(debug_enabled, |catalog| catalog.matches(&state.ui_kits)) {
        Some(tools) => tools,
        None => cache_tools(KitToolCatalog::of(&state.ui_kits), debug_enabled),
    };
    wrap_tools(id_raw, &tools)
}

/// `tools/list` for a kit catalog, with no editor state at all: the live
/// server's lock-free answer (#406).
pub fn tools_list_response_for(
    id_raw: &str,
    catalog: &KitToolCatalog,
    debug_enabled: bool,
) -> String {
    let tools = match cached_tools(debug_enabled, |cached| cached == catalog) {
        Some(tools) => tools,
        None => cache_tools(catalog.clone(), debug_enabled),
    };
    wrap_tools(id_raw, &tools)
}

struct CachedTools {
    catalog: KitToolCatalog,
    debug_enabled: bool,
    /// The serialized `tools` array, `[` … `]`.
    tools: Arc<str>,
}

fn cache() -> &'static Mutex<Vec<CachedTools>> {
    static CACHE: OnceLock<Mutex<Vec<CachedTools>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(Vec::new()))
}

fn cached_tools(
    debug_enabled: bool,
    is_catalog: impl Fn(&KitToolCatalog) -> bool,
) -> Option<Arc<str>> {
    let cache = cache().lock().unwrap_or_else(|poison| poison.into_inner());
    cache
        .iter()
        .find(|entry| entry.debug_enabled == debug_enabled && is_catalog(&entry.catalog))
        .map(|entry| Arc::clone(&entry.tools))
}

fn cache_tools(catalog: KitToolCatalog, debug_enabled: bool) -> Arc<str> {
    // Built outside the lock: two threads racing on a cold entry both
    // build it, which is cheaper than serializing every reader behind one.
    let tools: Arc<str> = build_tools(&catalog, debug_enabled).into();
    let mut cache = cache().lock().unwrap_or_else(|poison| poison.into_inner());
    if !cache
        .iter()
        .any(|entry| entry.debug_enabled == debug_enabled && entry.catalog == catalog)
    {
        if cache.len() >= CACHED_CATALOGS {
            cache.remove(0);
        }
        cache.push(CachedTools {
            catalog,
            debug_enabled,
            tools: Arc::clone(&tools),
        });
    }
    tools
}

fn build_tools(catalog: &KitToolCatalog, debug_enabled: bool) -> String {
    let mut entries: Vec<String> = mcp_tool_schemas()
        .iter()
        .map(|s| (*s).to_string())
        .collect();
    entries.extend(
        catalog
            .schemas()
            .iter()
            .map(|schema| de_mcp::root_placement::advertise_root_placement(schema)),
    );
    // pen.dev's names (#392): MCP clients only, never the in-app agent's
    // `TOOL_SCHEMAS`.
    entries.extend(
        de_mcp::pen_dev_aliases::PEN_DEV_ALIAS_SCHEMAS
            .iter()
            .map(|s| (*s).to_string()),
    );
    #[cfg(not(feature = "mcp-debug-tools"))]
    let _ = debug_enabled;
    #[cfg(feature = "mcp-debug-tools")]
    if debug_enabled {
        entries.extend(DEBUG_TOOL_SCHEMAS.iter().map(|s| (*s).to_string()));
    }
    format!("[{}]", entries.join(","))
}

fn wrap_tools(id_raw: &str, tools: &str) -> String {
    format!(r#"{{"jsonrpc":"2.0","id":{id_raw},"result":{{"tools":{tools}}}}}"#)
}
