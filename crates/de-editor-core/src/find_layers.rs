//! Find layers (#395): search every page of the document by layer name,
//! text content, id, node type, component, fill colour or variable.
//!
//! A query is free text plus optional `key:value` filters:
//!
//! | Filter                     | Matches                                         |
//! | -------------------------- | ----------------------------------------------- |
//! | `type:text` / `kind:`      | node type tag (`frame`, `text`, `ref`, …)        |
//! | `id:abc`                   | id contains                                     |
//! | `name:hero`                | layer name contains                             |
//! | `text:sign in`             | text content contains (until the next filter)   |
//! | `fill:#2563eb` / `color:`  | a solid fill of that colour                      |
//! | `var:primary` / `variable:`| a `$variable` reference contains                 |
//! | `component:Button`         | instances of a component (by id or name), and   |
//! |                            | the component masters themselves                |
//!
//! Free text matches the name first, then the text content, then the id,
//! case-insensitively. Every filter must hold. Results are ranked (exact
//! name, name prefix, word start, contains, then text and id hits) and keep
//! document order within a rank, page by page.
//!
//! The search, the select-all and the text replace are shared by the
//! command palette and the MCP `find_nodes` tool.

mod query;
mod replace;

use std::collections::{BTreeMap, BTreeSet};

use jian_ops_schema::node::{PenNode, TextContent};

use crate::fills::node_fills;
use crate::pen_node_ext::PenNodeExt;
use crate::{EditorState, NodeId};
use jian_ops_schema::style::PenFill;

pub use query::FindQuery;
pub use replace::replace_case_insensitive;

/// Results kept per search; the total is still counted past it.
pub const MAX_FIND_RESULTS: usize = 500;

/// Which property a layer matched on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchField {
    Name,
    Text,
    Id,
    /// Only `key:value` filters were given.
    Filter,
}

impl MatchField {
    /// Wire name (MCP `find_nodes`).
    pub fn as_str(self) -> &'static str {
        match self {
            MatchField::Name => "name",
            MatchField::Text => "text",
            MatchField::Id => "id",
            MatchField::Filter => "filter",
        }
    }
}

/// One layer that matched.
#[derive(Debug, Clone, PartialEq)]
pub struct LayerMatch {
    pub page_index: usize,
    pub node_id: NodeId,
    /// The layer name, or its type tag when it has none.
    pub name: String,
    pub kind: &'static str,
    pub field: MatchField,
    /// The text content when it is what matched (collapsed whitespace).
    pub snippet: Option<String>,
    pub score: u32,
}

/// A search result: the ranked matches (at most [`MAX_FIND_RESULTS`]) and
/// how many there were in all.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FindResults {
    pub matches: Vec<LayerMatch>,
    pub total: usize,
    /// Distinct pages holding a match.
    pub pages: usize,
}

/// One page of the document: its index, name and top-level layers. A
/// single-page document (no `pages` list) is page 0 with an empty name.
pub fn document_pages(state: &EditorState) -> Vec<(usize, &str, &[PenNode])> {
    match state.doc.pages.as_ref() {
        Some(pages) if !pages.is_empty() => pages
            .iter()
            .enumerate()
            .map(|(index, page)| (index, page.name.as_str(), page.children.as_slice()))
            .collect(),
        _ => vec![(0, "", state.doc.children.as_slice())],
    }
}

/// The plain text of a text-bearing layer (text and sticky notes).
pub fn node_text_content(node: &PenNode) -> Option<String> {
    let content = match node {
        PenNode::Text(text) => &text.content,
        PenNode::Note(note) | PenNode::Context(note) | PenNode::Prompt(note) => {
            note.content.as_ref()?
        }
        _ => return None,
    };
    Some(match content {
        TextContent::Plain(text) => text.clone(),
        TextContent::Styled(segments) => segments.iter().map(|s| s.text.as_str()).collect(),
    })
}

fn collapse_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `#2563eb`, `2563EB`, `#2563EBFF` → `2563EB` (alpha dropped). `None`
/// when `raw` is not a colour.
pub fn normalize_hex(raw: &str) -> Option<String> {
    let hex = raw.trim().trim_start_matches('#');
    if !(hex.len() == 6 || hex.len() == 8) || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    Some(hex[..6].to_ascii_uppercase())
}

/// Component masters by id → lowercased name, document-wide.
fn component_names(state: &EditorState) -> BTreeMap<String, String> {
    fn walk(state: &EditorState, nodes: &[PenNode], out: &mut BTreeMap<String, String>) {
        for node in nodes {
            if state.instance_view().is_component_master(node) {
                out.insert(
                    node.id_str().to_string(),
                    node.base().name.clone().unwrap_or_default().to_lowercase(),
                );
            }
            if let Some(children) = node.children() {
                walk(state, children, out);
            }
        }
    }
    let mut out = BTreeMap::new();
    for (_, _, children) in document_pages(state) {
        walk(state, children, &mut out);
    }
    for component in state.components.sorted_options().iter() {
        out.entry(component.id.clone())
            .or_insert_with(|| component.name.to_lowercase());
    }
    out
}

struct Matcher<'a> {
    query: &'a FindQuery,
    components: BTreeMap<String, String>,
    fill: Option<String>,
}

impl Matcher<'_> {
    fn component_matches(&self, node: &PenNode, wanted: &str) -> bool {
        let hit = |id: &str| {
            id.to_lowercase().contains(wanted)
                || self
                    .components
                    .get(id)
                    .is_some_and(|name| name.contains(wanted))
        };
        match node {
            PenNode::Ref(reference) => hit(&reference.target),
            _ => self.components.contains_key(node.id_str()) && hit(node.id_str()),
        }
    }

    fn fill_matches(&self, node: &PenNode, wanted: &str) -> bool {
        node_fills(node).is_some_and(|fills| {
            fills.iter().any(|fill| match fill {
                PenFill::Solid(body) => normalize_hex(&body.color).as_deref() == Some(wanted),
                _ => false,
            })
        })
    }

    /// `None` when the node does not match; else the field and rank.
    fn test(&self, node: &PenNode) -> Option<(MatchField, u32, Option<String>)> {
        let q = self.query;
        let name = node.base().name.as_deref().unwrap_or("").to_lowercase();
        let id = node.id_str().to_lowercase();
        // Matched on the raw content (lowercased), the same text a replace
        // rewrites, so every text match is one the replace can change.
        let mut text: Option<Option<String>> = None;
        let mut content_of = |node: &PenNode| -> Option<String> {
            text.get_or_insert_with(|| node_text_content(node).map(|t| t.to_lowercase()))
                .clone()
        };
        if let Some(kind) = &q.kind {
            if !kind_matches(node.type_tag(), kind) {
                return None;
            }
        }
        if q.id.as_ref().is_some_and(|wanted| !id.contains(wanted)) {
            return None;
        }
        if q.name.as_ref().is_some_and(|wanted| !name.contains(wanted)) {
            return None;
        }
        if let Some(wanted) = &q.text {
            let content = content_of(node)?;
            if !content.contains(wanted) {
                return None;
            }
        }
        if let Some(wanted) = &q.component {
            if !self.component_matches(node, wanted) {
                return None;
            }
        }
        if let Some(wanted) = &self.fill {
            if !self.fill_matches(node, wanted) {
                return None;
            }
        }
        if q.fill.is_some() && self.fill.is_none() {
            // An unparsable colour matches nothing rather than everything.
            return None;
        }
        if let Some(wanted) = &q.variable {
            let mut refs = BTreeSet::new();
            crate::uikit_io::node_variable_refs(node, &mut refs);
            if !refs.iter().any(|r| r.to_lowercase().contains(wanted)) {
                return None;
            }
        }
        let terms = q.terms.as_str();
        if terms.is_empty() {
            let snippet = q.text.as_ref().and_then(|_| display_text(node));
            let field = if snippet.is_some() {
                MatchField::Text
            } else {
                MatchField::Filter
            };
            return Some((field, 50, snippet));
        }
        if let Some(rank) = name_rank(&name, terms) {
            return Some((MatchField::Name, rank, None));
        }
        if content_of(node).is_some_and(|content| content.contains(terms)) {
            return Some((MatchField::Text, 40, display_text(node)));
        }
        if id.contains(terms) {
            return Some((MatchField::Id, if id == terms { 45 } else { 30 }, None));
        }
        None
    }
}

/// A text match's content for display: whitespace collapsed.
fn display_text(node: &PenNode) -> Option<String> {
    node_text_content(node).map(|t| collapse_whitespace(&t))
}

/// Every node type tag (`PenNode::type_tag`).
const TYPE_TAGS: [&str; 27] = [
    "frame",
    "group",
    "rectangle",
    "ellipse",
    "line",
    "polygon",
    "path",
    "text",
    "text_input",
    "image",
    "icon_font",
    "text_area",
    "select",
    "switch",
    "checkbox",
    "slider",
    "radio_group",
    "number_input",
    "progress",
    "tabs",
    "ref",
    "note",
    "context",
    "prompt",
    "script",
    "browser",
    "boolean",
];

/// The type tag a `type:` value names: the tag itself or a friendly alias.
fn kind_alias(wanted: &str) -> &str {
    match wanted {
        "rect" => "rectangle",
        "instance" | "component" => "ref",
        "img" => "image",
        "icon" => "icon_font",
        "sticky" => "note",
        other => other,
    }
}

/// Whether a `type:` value names a node type (MCP rejects unknown ones).
pub fn is_known_kind(wanted: &str) -> bool {
    let lower = wanted.to_ascii_lowercase();
    TYPE_TAGS.contains(&kind_alias(&lower))
}

/// Type filter: the tag itself, or a friendly alias.
fn kind_matches(tag: &str, wanted: &str) -> bool {
    tag.eq_ignore_ascii_case(kind_alias(wanted))
}

/// Rank of a name hit: exact 100, prefix 80, word start 70, contains 60.
fn name_rank(name: &str, terms: &str) -> Option<u32> {
    if name.is_empty() {
        return None;
    }
    if name == terms {
        return Some(100);
    }
    if name.starts_with(terms) {
        return Some(80);
    }
    let pos = name.find(terms)?;
    let at_word = name[..pos]
        .chars()
        .next_back()
        .is_some_and(|c| !c.is_alphanumeric());
    Some(if at_word { 70 } else { 60 })
}

/// Search every page for `query`. An empty query finds nothing.
pub fn find_layers(state: &EditorState, query: &FindQuery) -> FindResults {
    find_layers_on(state, query, None)
}

/// [`find_layers`] limited to one page when `page` is set.
pub fn find_layers_on(state: &EditorState, query: &FindQuery, page: Option<usize>) -> FindResults {
    let mut found = collect(state, query, page);
    // Stable: document order (page by page) within a rank.
    found.sort_by(|a, b| b.score.cmp(&a.score));
    let total = found.len();
    let pages = found
        .iter()
        .map(|m| m.page_index)
        .collect::<BTreeSet<_>>()
        .len();
    found.truncate(MAX_FIND_RESULTS);
    FindResults {
        matches: found,
        total,
        pages,
    }
}

/// Every match as `(page, id)`, uncapped and in document order: what
/// Shift+Enter selects and Replace rewrites, however many there are.
pub fn find_all_layer_matches(
    state: &EditorState,
    query: &FindQuery,
    page: Option<usize>,
) -> Vec<(usize, NodeId)> {
    collect(state, query, page)
        .into_iter()
        .map(|found| (found.page_index, found.node_id))
        .collect()
}

/// Every matching layer, unsorted (document order) and uncapped.
fn collect(state: &EditorState, query: &FindQuery, page: Option<usize>) -> Vec<LayerMatch> {
    if query.is_empty() {
        return Vec::new();
    }
    let matcher = Matcher {
        query,
        components: if query.component.is_some() {
            component_names(state)
        } else {
            BTreeMap::new()
        },
        fill: query.fill.as_deref().and_then(normalize_hex),
    };
    let mut found: Vec<LayerMatch> = Vec::new();
    for (page_index, _, children) in document_pages(state) {
        if page.is_some_and(|wanted| wanted != page_index) {
            continue;
        }
        let mut stack: Vec<&PenNode> = children.iter().rev().collect();
        while let Some(node) = stack.pop() {
            if let Some((field, score, snippet)) = matcher.test(node) {
                found.push(LayerMatch {
                    page_index,
                    node_id: NodeId::new(node.id_str()),
                    name: node
                        .base()
                        .name
                        .clone()
                        .filter(|name| !name.trim().is_empty())
                        .unwrap_or_else(|| node.type_tag().to_string()),
                    kind: node.type_tag(),
                    field,
                    snippet,
                    score,
                });
            }
            if let Some(children) = node.children() {
                stack.extend(children.iter().rev());
            }
        }
    }
    found
}

/// Whether `id` is a layer on page `page`.
fn on_page(state: &EditorState, page: usize, id: &NodeId) -> bool {
    document_pages(state)
        .get(page)
        .is_some_and(|(_, _, children)| crate::walkers::find_node(children, id).is_some())
}

impl EditorState {
    /// Shift+Enter in find: select every match on `page` (selection is per
    /// page), switching to it first. Returns the selected ids.
    pub fn select_layer_matches(&mut self, page: usize, ids: &[NodeId]) -> Vec<NodeId> {
        // Never switch pages for ids that are not (or no longer) there.
        if !ids.iter().any(|id| on_page(self, page, id)) {
            return Vec::new();
        }
        if page != self.ui.active_page_index && !self.set_active_page(page) {
            return Vec::new();
        }
        let present: Vec<NodeId> = ids
            .iter()
            .filter(|id| crate::walkers::find_node(self.active_children(), id).is_some())
            .cloned()
            .collect();
        let Some(anchor) = present.last().cloned() else {
            return Vec::new();
        };
        self.editor_ui.image_panel.close_popovers();
        self.editor_ui.image_crop_editing = None;
        self.selection = crate::SelectionState {
            anchor,
            set: present.clone(),
        };
        present
    }

    /// Enter in find: switch to the match's page and select it.
    pub fn select_layer_match(&mut self, page: usize, id: &NodeId) -> bool {
        if !on_page(self, page, id) {
            return false;
        }
        if page != self.ui.active_page_index && !self.set_active_page(page) {
            return false;
        }
        if crate::walkers::find_node(self.active_children(), id).is_none() {
            return false;
        }
        self.set_single_selection(id.clone());
        true
    }
}

#[cfg(test)]
#[path = "find_layers_tests.rs"]
pub(crate) mod tests;
