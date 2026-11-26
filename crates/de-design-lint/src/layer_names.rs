//! Layer names (#383): which names are editor defaults (`Frame 1`,
//! `Rectangle 3`), the offline rule-based namer that proposes semantic
//! names (`Primary button`, `Pricing card`, `Hero / Title`) in the
//! document's language, and the lint finding for default-named layers.
//!
//! Only names are ever proposed — ids are never touched — and a layer whose
//! name already says something is left alone. A text layer named after its
//! own content counts as named.

mod default_names;
mod lang;
mod rules;
mod text_roles;
mod unique;
mod walk;

use std::collections::HashSet;

use de_i18n::Locale;
use jian_ops_schema::node::PenNode;
use serde::Serialize;
use serde_json::Value;

use crate::issue::{FixProperty, Issue, IssueCategory, IssueSeverity};
use crate::node_util::{base, children};

pub use default_names::is_default_layer_name;
pub use lang::detect_document_locale;

pub use rules::{collapse_whitespace, text_content};

use walk::candidates;

/// One proposed rename: the layer, its name today (`None` when it has
/// none) and the name the rules propose.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LayerNameProposal {
    pub node_id: String,
    pub current: Option<String>,
    pub proposed: String,
}

/// True when the layer's name already says what it is: set, not an editor
/// default, or (for a text layer) its own content.
pub fn has_meaningful_name(node: &PenNode) -> bool {
    let Some(name) = base(node)
        .name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
    else {
        return false;
    };
    if text_content(node).is_some_and(|content| collapse_whitespace(name) == content) {
        return true;
    }
    !is_default_layer_name(name)
}

/// Every text layer's content under `roots`, in tree order.
pub fn texts_under(roots: &[PenNode]) -> Vec<String> {
    fn walk(node: &PenNode, out: &mut Vec<String>) {
        if let Some(text) = text_content(node) {
            if !text.is_empty() {
                out.push(text);
            }
        }
        for child in children(node) {
            walk(child, out);
        }
    }
    let mut out = Vec::new();
    for root in roots {
        walk(root, &mut out);
    }
    out
}

/// The language `roots` are written in (see [`detect_document_locale`]).
pub fn document_locale(roots: &[PenNode]) -> Locale {
    let texts = texts_under(roots);
    detect_document_locale(texts.iter().map(String::as_str))
}

/// Rule-based names for the layers under `roots` (a page's top-level
/// layers) that have no meaningful name, in tree order. `targets` limits
/// the proposals to those layers and their descendants; `None` covers the
/// whole page. Role words come from `locale` — pass the document's
/// language ([`document_locale`]).
pub fn propose_layer_names(
    roots: &[PenNode],
    targets: Option<&HashSet<String>>,
    locale: Locale,
) -> Vec<LayerNameProposal> {
    candidates(roots, targets, locale)
        .into_iter()
        .filter_map(|candidate| {
            Some(LayerNameProposal {
                proposed: candidate.proposed?,
                node_id: candidate.node_id,
                current: candidate.current,
            })
        })
        .collect()
}

/// Info finding for every layer under `root` (a top-level layer of a
/// page) whose name is an editor default. The suggested value is the
/// rules' proposal, or `null` when they have none; the fix is
/// `rename_layers`.
pub fn detect_default_layer_names(root: &PenNode) -> Vec<Issue> {
    let roots = std::slice::from_ref(root);
    candidates(roots, None, document_locale(roots))
        .into_iter()
        .filter_map(|candidate| {
            let current = candidate.current?;
            is_default_layer_name(&current).then(|| Issue {
                reason: format!(
                    "layer name \"{}\" is an editor default; rename it after what it is \
                     (rename_layers)",
                    current.trim()
                ),
                node_id: candidate.node_id,
                category: IssueCategory::DefaultLayerName,
                severity: IssueSeverity::Info,
                property: FixProperty::Name,
                current_value: Value::String(current),
                suggested_value: candidate.proposed.map(Value::String).unwrap_or(Value::Null),
            })
        })
        .collect()
}

#[cfg(test)]
#[path = "layer_names_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "layer_names_roles_tests.rs"]
mod roles_tests;
