//! MCP `find_nodes` (#395): find layers across every page by name, text
//! content, id, node type, component, fill colour or variable, with the
//! same ranking as the editor's Find layers (`de_editor_core::find_layers`).
//!
//! `query` takes the editor's syntax (free text plus `type:` `id:` `fill:`
//! `var:` `component:` `name:` `text:` filters); the named arguments set or
//! override a filter. `page` narrows the search to one page (index, id or
//! name; `active` for the active page). Read-only.

use std::collections::BTreeMap;

use de_editor_core::find_layers::{
    document_pages, find_layers_on, is_known_kind, node_text_content, normalize_hex, FindQuery,
    MAX_FIND_RESULTS,
};
use de_editor_core::shared_state::SharedState;
use de_editor_core::walkers::find_node;
use de_editor_core::EditorState;
use serde_json::json;

use super::{McpTool, ToolErrorCode, ToolOutcome};

/// Matches returned when `limit` is not given.
const DEFAULT_LIMIT: usize = 100;

pub struct FindNodes {
    /// The request's state, shared (#456): the document, component registry
    /// and active page the search reads.
    state: SharedState,
    page_ids: Vec<String>,
}

pub fn find_nodes_snapshot(state: &EditorState) -> FindNodes {
    // Only the document, the registry and the active page are read: copy
    // those, not the session state around them.
    let mut detached = EditorState::new();
    detached.doc = state.doc.clone();
    detached.components = state.components.clone();
    detached.ui.active_page_index = state.ui.active_page_index;
    find_nodes_shared(SharedState::new(detached))
}

/// [`find_nodes_snapshot`] over a state the caller already shares: no
/// copy (#456).
pub fn find_nodes_shared(state: SharedState) -> FindNodes {
    let page_ids = state
        .doc
        .pages
        .as_ref()
        .map(|pages| pages.iter().map(|page| page.id.clone()).collect())
        .unwrap_or_default();
    FindNodes { state, page_ids }
}

type ToolError = (ToolErrorCode, String);

fn lowered(args: &BTreeMap<String, String>, key: &str) -> Option<String> {
    args.get(key)
        .map(|value| value.trim().to_lowercase())
        .filter(|value| !value.is_empty())
}

impl FindNodes {
    fn page_filter(&self, raw: Option<&str>) -> Result<Option<usize>, ToolError> {
        let Some(raw) = raw.map(str::trim).filter(|raw| !raw.is_empty()) else {
            return Ok(None);
        };
        if raw.eq_ignore_ascii_case("all") {
            return Ok(None);
        }
        if raw.eq_ignore_ascii_case("active") {
            return Ok(Some(self.state.ui.active_page_index));
        }
        let pages = document_pages(&self.state);
        if let Ok(index) = raw.parse::<usize>() {
            if index < pages.len() {
                return Ok(Some(index));
            }
        }
        if let Some(index) = self.page_ids.iter().position(|id| id == raw) {
            return Ok(Some(index));
        }
        if let Some((index, _, _)) = pages
            .iter()
            .find(|(_, name, _)| name.eq_ignore_ascii_case(raw))
        {
            return Ok(Some(*index));
        }
        Err((
            ToolErrorCode::InvalidArgument,
            format!("page not found: {raw} (use an index, a page id, a page name, active or all)"),
        ))
    }

    fn run(&self, args: &BTreeMap<String, String>) -> Result<ToolOutcome, ToolError> {
        let mut query = FindQuery::parse(args.get("query").map(String::as_str).unwrap_or(""));
        let overrides = [
            ("name", &mut query.name),
            ("text", &mut query.text),
            ("type", &mut query.kind),
            ("id", &mut query.id),
            ("componentId", &mut query.component),
            ("fill", &mut query.fill),
        ];
        for (key, slot) in overrides {
            if let Some(value) = lowered(args, key) {
                *slot = Some(value);
            }
        }
        if let Some(variable) = lowered(args, "variable") {
            query.variable = Some(variable.trim_start_matches('$').to_string());
        }
        if query.is_empty() {
            return Err((
                ToolErrorCode::MissingArgument,
                "give a query or at least one filter (name, text, type, id, componentId, fill, \
                 variable)"
                    .to_string(),
            ));
        }
        if let Some(fill) = query.fill.as_deref() {
            if normalize_hex(fill).is_none() {
                return Err((
                    ToolErrorCode::InvalidArgument,
                    format!("fill must be a #RRGGBB colour, got {fill}"),
                ));
            }
        }
        if let Some(kind) = query.kind.as_deref() {
            if !is_known_kind(kind) {
                return Err((
                    ToolErrorCode::InvalidArgument,
                    format!(
                        "unknown type {kind} (a node type tag such as frame, text, rectangle, \
                         ellipse, path, image, ref)"
                    ),
                ));
            }
        }
        let limit = match args.get("limit") {
            None => DEFAULT_LIMIT,
            Some(raw) => raw
                .trim()
                .parse::<usize>()
                .ok()
                .filter(|n| (1..=MAX_FIND_RESULTS).contains(n))
                .ok_or((
                    ToolErrorCode::InvalidArgument,
                    format!("limit must be an integer from 1 to {MAX_FIND_RESULTS}, got {raw}"),
                ))?,
        };
        let page = self.page_filter(args.get("page").map(String::as_str))?;
        let results = find_layers_on(&self.state, &query, page);
        let pages = document_pages(&self.state);
        let matches: Vec<_> = results
            .matches
            .iter()
            .take(limit)
            .map(|found| {
                let (page_name, children) = pages
                    .get(found.page_index)
                    .map(|(_, name, children)| (*name, *children))
                    .unwrap_or(("", &[]));
                let text = find_node(children, &found.node_id).and_then(node_text_content);
                json!({
                    "id": found.node_id.as_str(),
                    "name": found.name,
                    "type": found.kind,
                    "page": found.page_index,
                    "pageId": self.page_ids.get(found.page_index),
                    "pageName": page_name,
                    "matchedOn": found.field.as_str(),
                    "text": text,
                })
            })
            .collect();
        Ok(ToolOutcome::OkJson(
            json!({
                "count": matches.len(),
                "total": results.total,
                "pages": results.pages,
                "truncated": results.total > matches.len(),
                "matches": matches,
            })
            .to_string(),
        ))
    }
}

impl McpTool for FindNodes {
    fn name(&self) -> &str {
        "find_nodes"
    }

    fn call(&self, args: &BTreeMap<String, String>) -> ToolOutcome {
        match self.run(args) {
            Ok(outcome) => outcome,
            Err((code, message)) => ToolOutcome::Err(code, message),
        }
    }
}

#[cfg(test)]
#[path = "find_nodes_tool_tests.rs"]
mod tests;
