//! The agent half of rename layers: a compact brief of each layer to name
//! (kind, text, children, where it sits, the rules' suggestion), the prompt
//! that asks for names in the document's language, and the parser for the
//! `{ "<id>": "<name>" }` reply. Transport-free: the desktop host sends the
//! prompt through the selected chat provider, and MCP hands the brief to
//! the calling agent.

use std::collections::{BTreeMap, HashMap};

use de_i18n::Locale;
use jian_ops_schema::node::text::TextContent;
use jian_ops_schema::node::PenNode;
use serde_json::{json, Value};

use crate::pen_node_ext::PenNodeExt;

use super::LayerNameProposal;

/// Most layers one brief describes; a bigger selection is named in the
/// order the layers appear and the rest keep the rules' names.
pub const MAX_BRIEF_LAYERS: usize = 300;
/// Longest text excerpt a brief carries per layer.
const MAX_TEXT_CHARS: usize = 80;
/// Longest name accepted from a reply.
const MAX_NAME_CHARS: usize = 80;

fn text_of(node: &PenNode) -> Option<String> {
    let PenNode::Text(text) = node else {
        return None;
    };
    let raw: String = match &text.content {
        TextContent::Plain(text) => text.clone(),
        TextContent::Styled(segments) => segments.iter().map(|s| s.text.as_str()).collect(),
    };
    let collapsed = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    Some(collapsed.chars().take(MAX_TEXT_CHARS).collect())
}

fn kind_of(node: &PenNode) -> String {
    de_design_lint::node_util::node_kind_str(node).to_string()
}

fn child_summary(node: &PenNode) -> Vec<String> {
    node.children()
        .map_or(&[][..], Vec::as_slice)
        .iter()
        .take(8)
        .map(|child| match text_of(child) {
            Some(text) => format!("text \"{text}\""),
            None => kind_of(child),
        })
        .collect()
}

/// `id → (node, parent names from the page down)` for the active page.
fn index_page(roots: &[PenNode]) -> HashMap<&str, (&PenNode, Vec<String>)> {
    fn walk<'a>(
        node: &'a PenNode,
        path: &mut Vec<String>,
        out: &mut HashMap<&'a str, (&'a PenNode, Vec<String>)>,
    ) {
        out.insert(node.id_str(), (node, path.clone()));
        let name = node.base().name.clone().unwrap_or_else(|| kind_of(node));
        path.push(name);
        for child in node.children().map_or(&[][..], Vec::as_slice) {
            walk(child, path, out);
        }
        path.pop();
    }
    let mut out = HashMap::new();
    for root in roots {
        walk(root, &mut Vec::new(), &mut out);
    }
    out
}

/// One JSON object per proposal (at most [`MAX_BRIEF_LAYERS`]): what an
/// agent needs to name the layer without reading the whole document.
/// `roots` are the top-level layers of the page the proposals were made on.
pub fn rename_layer_briefs(roots: &[PenNode], proposals: &[LayerNameProposal]) -> Vec<Value> {
    let index = index_page(roots);
    proposals
        .iter()
        .take(MAX_BRIEF_LAYERS)
        .filter_map(|proposal| {
            let (node, path) = index.get(proposal.node_id.as_str())?;
            let mut brief = json!({
                "id": proposal.node_id,
                "kind": kind_of(node),
                "current": proposal.current,
                "suggested": proposal.proposed,
                "path": path.join(" / "),
            });
            if let Some(text) = text_of(node) {
                brief["text"] = Value::String(text);
            }
            let children = child_summary(node);
            if !children.is_empty() {
                brief["children"] = json!(children);
            }
            Some(brief)
        })
        .collect()
}

/// The language line of the prompt, e.g. `Türkçe (tr)`.
pub fn language_label(locale: Locale) -> String {
    format!("{} ({})", locale.display_name(), locale.code())
}

/// The prompt asking an agent to name the layers in `briefs`.
pub fn rename_layers_prompt(briefs: &[Value], locale: Locale) -> String {
    let layers = serde_json::to_string_pretty(briefs).unwrap_or_else(|_| "[]".to_string());
    format!(
        "You name layers in a UI design file so designers, code generators and other agents \
         can find them.\n\
         For every layer below, propose a short semantic name (1-4 words) that says what the \
         layer is for: its role and, when useful, the section it belongs to, e.g. \
         \"Primary button\", \"Pricing card\", \"Hero / Title\". Text layers may be named after \
         what they say. Never use generic kind names like Frame, Group, Rectangle or Text, \
         and never add numbers to tell layers apart.\n\
         Write the names in {language}, the language of the document.\n\
         `suggested` is an offline guess; improve on it. `path` lists the parent layers.\n\
         Reply with ONLY a JSON object mapping each layer id to its new name, no prose:\n\
         {{\"<id>\": \"<name>\"}}\n\n\
         Layers:\n{layers}",
        language = language_label(locale),
    )
}

/// The `{ id: name }` map in an agent's reply. Tolerates prose or a code
/// fence around the object, and an array of `{ "id", "name" }` objects.
/// Empty and over-long names are dropped.
pub fn parse_rename_layers_reply(reply: &str) -> BTreeMap<String, String> {
    let mut names = BTreeMap::new();
    let mut keep = |id: &str, name: &str| {
        let name = name.trim();
        if !id.is_empty() && !name.is_empty() && name.chars().count() <= MAX_NAME_CHARS {
            names.insert(id.to_string(), name.to_string());
        }
    };
    let object = reply
        .find('{')
        .zip(reply.rfind('}'))
        .filter(|(start, end)| start < end)
        .and_then(|(start, end)| serde_json::from_str::<Value>(&reply[start..=end]).ok());
    let array = reply
        .find('[')
        .zip(reply.rfind(']'))
        .filter(|(start, end)| start < end)
        .and_then(|(start, end)| serde_json::from_str::<Value>(&reply[start..=end]).ok());
    match (object, array) {
        (Some(Value::Object(map)), _) if !map.contains_key("id") => {
            for (id, name) in &map {
                if let Some(name) = name.as_str() {
                    keep(id, name);
                }
            }
        }
        (_, Some(Value::Array(items))) => {
            for item in &items {
                if let (Some(id), Some(name)) = (
                    item.get("id").and_then(Value::as_str),
                    item.get("name").and_then(Value::as_str),
                ) {
                    keep(id, name);
                }
            }
        }
        _ => {}
    }
    names
}
