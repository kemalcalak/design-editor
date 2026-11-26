//! Compact summaries of mentioned document items, and the context block
//! they are folded into.
//!
//! A mention resolves to a few lines, never a serialized subtree: the
//! agent learns what the thing is, where it is, how big it is and what
//! is directly inside it, and can read more through its own tools when it
//! needs to. Every summary is capped at [`MAX_SUMMARY_CHARS`].

use jian_ops_schema::node::PenNode;
use jian_ops_schema::style::PenFill;
use jian_ops_schema::variable::VariableValue;

use super::candidates::scalar_preview;
use super::{ChatMention, MentionKind};
use crate::fills::node_fills;
use crate::find_layers::{document_pages, node_text_content};
use crate::pen_node_ext::PenNodeExt;
use crate::{EditorState, NodeId};

/// Longest summary handed to the model for one mention.
pub const MAX_SUMMARY_CHARS: usize = 1_200;
/// Direct children named in a container's summary.
const MAX_LISTED_CHILDREN: usize = 12;
/// Text content quoted from a text layer or note.
const MAX_TEXT_CHARS: usize = 240;

/// A resolved mention: what it was and what the model is told about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MentionSummary {
    pub mention: ChatMention,
    /// `None` when the target no longer exists (deleted since it was
    /// picked); the block says so instead of silently dropping it.
    pub text: Option<String>,
}

fn clip(text: &str, max: usize) -> String {
    let mut out: String = text.chars().take(max).collect();
    if text.chars().nth(max).is_some() {
        out.push('…');
    }
    out
}

fn fmt_num(value: f64) -> String {
    if value.fract().abs() < 0.005 {
        format!("{}", value.round() as i64)
    } else {
        format!("{value:.1}")
    }
}

fn node_name(node: &PenNode) -> &str {
    node.base()
        .name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| node.type_tag())
}

fn count_descendants(node: &PenNode) -> usize {
    node.children()
        .map(|kids| kids.iter().map(|kid| 1 + count_descendants(kid)).sum())
        .unwrap_or(0)
}

fn first_solid_fill(node: &PenNode) -> Option<String> {
    node_fills(node)?.iter().find_map(|fill| match fill {
        PenFill::Solid(body) => Some(body.color.clone()),
        _ => None,
    })
}

/// `frame "Hero" (id n1) — 1440×900 at (0, 0); fill #FFFFFF; 3 children: …`
fn node_summary(state: &EditorState, node: &PenNode, page: Option<&str>) -> String {
    let mut out = format!(
        "{} \"{}\" (id {})",
        node.type_tag(),
        node_name(node),
        node.id_str()
    );
    if let Some(page) = page {
        out.push_str(&format!(" on page \"{page}\""));
    }
    let base = node.base();
    match (node.width_px(), node.height_px()) {
        (Some(w), Some(h)) => out.push_str(&format!(" — {}×{}", fmt_num(w), fmt_num(h))),
        _ => out.push_str(" — auto-sized"),
    }
    if let (Some(x), Some(y)) = (base.x, base.y) {
        out.push_str(&format!(" at ({}, {})", fmt_num(x), fmt_num(y)));
    }
    if let Some(fill) = first_solid_fill(node) {
        out.push_str(&format!("; fill {fill}"));
    }
    if let PenNode::Ref(instance) = node {
        let target = NodeId::new(instance.target.as_str());
        let name = state
            .components
            .find_by_id(&target)
            .map(|c| c.name.as_str())
            .unwrap_or(instance.target.as_str());
        out.push_str(&format!("; instance of component \"{name}\""));
    }
    if let Some(text) = node_text_content(node) {
        let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
        out.push_str(&format!("; text \"{}\"", clip(&text, MAX_TEXT_CHARS)));
    }
    if let Some(kids) = node.children() {
        if !kids.is_empty() {
            let listed: Vec<String> = kids
                .iter()
                .take(MAX_LISTED_CHILDREN)
                .map(|kid| format!("{} \"{}\"", kid.type_tag(), node_name(kid)))
                .collect();
            out.push_str(&format!(
                "; {} direct children ({} layers inside): {}",
                kids.len(),
                count_descendants(node),
                listed.join(", ")
            ));
            if kids.len() > MAX_LISTED_CHILDREN {
                out.push_str(", …");
            }
        }
    }
    if let Some(explain) = base.explain.as_deref().filter(|e| !e.trim().is_empty()) {
        out.push_str(&format!("; note: {}", clip(explain.trim(), MAX_TEXT_CHARS)));
    }
    out
}

fn page_summary(index: usize, name: &str, children: &[PenNode]) -> String {
    let label = if name.trim().is_empty() {
        format!("Page {}", index + 1)
    } else {
        name.to_string()
    };
    let layers: usize = children.iter().map(|n| 1 + count_descendants(n)).sum();
    let listed: Vec<String> = children
        .iter()
        .take(MAX_LISTED_CHILDREN)
        .map(|n| {
            let size = match (n.width_px(), n.height_px()) {
                (Some(w), Some(h)) => format!(" {}×{}", fmt_num(w), fmt_num(h)),
                _ => String::new(),
            };
            format!("{} \"{}\"{size}", n.type_tag(), node_name(n))
        })
        .collect();
    let mut out = format!(
        "page \"{label}\" (index {index}) — {} top-level layers, {layers} layers in all",
        children.len()
    );
    if !listed.is_empty() {
        out.push_str(&format!(": {}", listed.join(", ")));
        if children.len() > MAX_LISTED_CHILDREN {
            out.push_str(", …");
        }
    }
    out
}

fn variable_summary(state: &EditorState, name: &str) -> Option<String> {
    let def = state.doc.variables.as_ref()?.get(name)?;
    let kind = super::candidates::variable_kind(&def.kind);
    let values = match &def.value {
        VariableValue::Scalar(scalar) => scalar_preview(scalar),
        VariableValue::Themed(values) => values
            .iter()
            .map(|themed| {
                let theme = themed
                    .theme
                    .as_ref()
                    .map(|t| {
                        t.iter()
                            .map(|(axis, value)| format!("{axis}={value}"))
                            .collect::<Vec<_>>()
                            .join(",")
                    })
                    .filter(|t| !t.is_empty())
                    .unwrap_or_else(|| "default".to_string());
                format!("{theme}: {}", scalar_preview(&themed.value))
            })
            .collect::<Vec<_>>()
            .join("; "),
    };
    Some(format!("{kind} variable ${name} = {values}"))
}

/// Summarize a document mention (page, layer, component or variable).
/// `None` when the target is gone, or for a file mention (the host reads
/// files).
pub fn summarize_document_mention(state: &EditorState, mention: &ChatMention) -> Option<String> {
    let text = match mention.kind {
        MentionKind::Page => {
            let index: usize = mention.key.parse().ok()?;
            let pages = document_pages(state);
            let (index, name, children) = pages.get(index)?;
            page_summary(*index, name, children)
        }
        MentionKind::Node => {
            let id = NodeId::new(mention.key.as_str());
            let pages = document_pages(state);
            let multi_page = pages.len() > 1;
            let (name, node) = pages.iter().find_map(|(_, name, children)| {
                crate::walkers::find_node(children, &id).map(|node| (*name, node))
            })?;
            node_summary(state, node, multi_page.then_some(name))
        }
        MentionKind::Component => {
            let id = NodeId::new(mention.key.as_str());
            let component = state.components.find_by_id(&id)?;
            let root = state
                .components
                .resolved_root(&state.doc, &id)
                .unwrap_or(&component.root);
            format!(
                "component \"{}\" — master: {}",
                component.name,
                node_summary(state, root, None)
            )
        }
        MentionKind::Variable => variable_summary(state, &mention.key)?,
        MentionKind::File => return None,
    };
    Some(clip(&text, MAX_SUMMARY_CHARS))
}

/// The block appended to the provider-bound user message, or an empty
/// string when nothing was mentioned. English on purpose: it is model
/// input, like the system prompt, not chrome.
pub fn context_block(summaries: &[MentionSummary]) -> String {
    if summaries.is_empty() {
        return String::new();
    }
    let mut out = String::from(
        "\n\n---\nReferenced context (the user @-mentioned these; summaries, not full dumps — \
         read more with your tools if needed):",
    );
    for summary in summaries {
        out.push_str(&format!(
            "\n- @{} ({}): ",
            summary.mention.label,
            summary.mention.kind.wire_name()
        ));
        match summary.text.as_deref() {
            Some(text) => out.push_str(text),
            None if summary.mention.kind == MentionKind::File => {
                out.push_str("could not be read from the project folder")
            }
            None => out.push_str("no longer exists in the document"),
        }
    }
    out
}
