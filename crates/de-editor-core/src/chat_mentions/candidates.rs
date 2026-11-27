//! Ranking the `@` popup's rows.
//!
//! Scoring reuses the command palette's ranker (`command_palette::fuzzy`),
//! so `@hero` orders "Hero" > "Hero card" > "Page hero" exactly as the
//! palette would. The canvas selection gets a boost: the layer the user is
//! looking at is the one they most likely mean.

use jian_ops_schema::node::PenNode;
use jian_ops_schema::variable::{VariableKind, VariableScalar, VariableValue};

use super::{ChatMention, MentionCandidate, MentionKind};
use crate::command_palette::fuzzy::fuzzy_score;
use crate::find_layers::document_pages;
use crate::pen_node_ext::PenNodeExt;
use crate::EditorState;

/// Rows the popup shows at most.
pub const MAX_MENTION_ROWS: usize = 8;

/// Layers walked per query. Name search over a huge imported file stops
/// here instead of stalling a keystroke; the selection and pages are
/// always offered regardless.
const MAX_WALKED_NODES: usize = 20_000;

/// Added to a selected layer's score.
const SELECTION_BOOST: u32 = 2_500;

struct Ranked {
    score: u32,
    order: usize,
    candidate: MentionCandidate,
}

fn page_label(index: usize, name: &str) -> String {
    if name.trim().is_empty() {
        format!("Page {}", index + 1)
    } else {
        name.to_string()
    }
}

/// The stable id of page `index` (a mention must survive pages being
/// reordered, added or removed before it is sent). Only multi-page documents
/// offer pages, so the page list exists.
fn page_id(state: &EditorState, index: usize) -> String {
    state
        .doc
        .pages
        .as_ref()
        .and_then(|pages| pages.get(index))
        .map(|page| page.id.clone())
        .unwrap_or_default()
}

fn node_label(node: &PenNode) -> Option<&str> {
    node.base()
        .name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
}

fn node_candidate(node: &PenNode, label: &str, page: Option<&str>) -> MentionCandidate {
    let detail = match page {
        Some(page) => format!("{} · {page}", node.type_tag()),
        None => node.type_tag().to_string(),
    };
    MentionCandidate {
        mention: ChatMention {
            kind: MentionKind::Node,
            key: node.id_str().to_string(),
            label: label.to_string(),
        },
        detail,
    }
}

pub(crate) fn variable_kind(kind: &VariableKind) -> &'static str {
    match kind {
        VariableKind::Color => "color",
        VariableKind::Number => "number",
        VariableKind::String => "string",
        VariableKind::Boolean => "boolean",
    }
}

/// A short preview of a variable's (first) value.
pub(crate) fn scalar_preview(value: &VariableScalar) -> String {
    match value {
        VariableScalar::Bool(b) => b.to_string(),
        VariableScalar::Num(n) => {
            if n.fract() == 0.0 && n.abs() < 1e15 {
                format!("{}", *n as i64)
            } else {
                format!("{n}")
            }
        }
        VariableScalar::Str(s) => s.chars().take(40).collect(),
    }
}

fn variable_detail(kind: &VariableKind, value: &VariableValue) -> String {
    let first = match value {
        VariableValue::Scalar(scalar) => Some(scalar),
        VariableValue::Themed(values) => values.first().map(|v| &v.value),
    };
    match first {
        Some(scalar) => format!("{} · {}", variable_kind(kind), scalar_preview(scalar)),
        None => variable_kind(kind).to_string(),
    }
}

fn file_candidate(path: &str) -> MentionCandidate {
    let folder = path.rsplit_once('/').map(|(dir, _)| dir).unwrap_or("");
    MentionCandidate {
        mention: ChatMention {
            kind: MentionKind::File,
            key: path.to_string(),
            label: path.to_string(),
        },
        detail: folder.to_string(),
    }
}

/// Score a file on its name first (what people type), then its path.
fn file_score(query: &str, path: &str) -> Option<u32> {
    let name = path.rsplit('/').next().unwrap_or(path);
    let by_name = fuzzy_score(query, name);
    let by_path = fuzzy_score(query, path).map(|score| score / 2);
    by_name.max(by_path)
}

/// The popup rows for `raw_query`, best first, at most [`MAX_MENTION_ROWS`].
///
/// An empty query lists what is closest at hand: the selection, the
/// pages, the active page's top-level layers, then components, variables
/// and files. A typed query ranks everything that matches.
pub fn mention_candidates(
    state: &EditorState,
    files: &[String],
    raw_query: &str,
) -> Vec<MentionCandidate> {
    let query = raw_query.trim().to_lowercase();
    let mut ranked: Vec<Ranked> = Vec::new();
    let mut push = |score: u32, candidate: MentionCandidate| {
        let order = ranked.len();
        ranked.push(Ranked {
            score,
            order,
            candidate,
        });
    };
    let pages = document_pages(state);
    let multi_page = pages.len() > 1;
    let active_page = state
        .ui
        .active_page_index
        .min(pages.len().saturating_sub(1));

    // Selection first: boosted, and offered even when unnamed.
    let mut selected_ids: Vec<&str> = Vec::new();
    for id in state.selection.set.iter() {
        let Some(node) = crate::walkers::find_node(state.active_children(), id) else {
            continue;
        };
        let label = node_label(node).unwrap_or_else(|| node.type_tag());
        if let Some(score) = fuzzy_score(&query, label) {
            selected_ids.push(node.id_str());
            push(score + SELECTION_BOOST, node_candidate(node, label, None));
        }
    }

    if multi_page {
        for (index, name, _) in &pages {
            let label = page_label(*index, name);
            if let Some(score) = fuzzy_score(&query, &label) {
                push(
                    score,
                    MentionCandidate {
                        mention: ChatMention {
                            kind: MentionKind::Page,
                            key: page_id(state, *index),
                            label,
                        },
                        detail: String::new(),
                    },
                );
            }
        }
    }

    if query.is_empty() {
        // Top-level layers of the active page only: a whole-tree listing
        // with nothing typed is noise.
        if let Some((_, _, children)) = pages.get(active_page) {
            for node in children.iter() {
                if let Some(label) = node_label(node) {
                    if !selected_ids.contains(&node.id_str()) {
                        push(0, node_candidate(node, label, None));
                    }
                }
            }
        }
    } else {
        let mut walked = 0usize;
        'pages: for (index, name, children) in &pages {
            let page = multi_page.then(|| page_label(*index, name));
            let mut stack: Vec<&PenNode> = children.iter().rev().collect();
            while let Some(node) = stack.pop() {
                walked += 1;
                if walked > MAX_WALKED_NODES {
                    break 'pages;
                }
                if let Some(label) = node_label(node) {
                    if !selected_ids.contains(&node.id_str()) {
                        if let Some(score) = fuzzy_score(&query, label) {
                            push(score, node_candidate(node, label, page.as_deref()));
                        }
                    }
                }
                if let Some(kids) = node.children() {
                    stack.extend(kids.iter().rev());
                }
            }
        }
    }

    for component in state.components.sorted_options().iter() {
        if let Some(score) = fuzzy_score(&query, &component.name) {
            push(
                score,
                MentionCandidate {
                    mention: ChatMention {
                        kind: MentionKind::Component,
                        key: component.id.clone(),
                        label: component.name.clone(),
                    },
                    detail: String::new(),
                },
            );
        }
    }

    if let Some(variables) = state.doc.variables.as_ref() {
        for (name, def) in variables.iter() {
            if let Some(score) = fuzzy_score(&query, name) {
                push(
                    score,
                    MentionCandidate {
                        mention: ChatMention {
                            kind: MentionKind::Variable,
                            key: name.clone(),
                            label: name.clone(),
                        },
                        detail: variable_detail(&def.kind, &def.value),
                    },
                );
            }
        }
    }

    for path in files {
        let score = if query.is_empty() {
            Some(0)
        } else {
            file_score(&query, path)
        };
        if let Some(score) = score {
            push(score, file_candidate(path));
        }
    }

    // Best score first; ties keep source order (selection, pages, layers,
    // components, variables, files).
    ranked.sort_by(|a, b| b.score.cmp(&a.score).then(a.order.cmp(&b.order)));
    ranked
        .into_iter()
        .take(MAX_MENTION_ROWS)
        .map(|r| r.candidate)
        .collect()
}
