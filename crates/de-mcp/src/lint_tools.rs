use std::collections::{BTreeMap, BTreeSet};

use de_design_lint::{
    detect_all, detect_default_layer_names, document_locale, FixProperty, Issue, IssueCategory,
    IssueSeverity,
};
use de_editor_core::rename_layers::DocumentInstances;
use de_editor_core::repeated_structures::{find_repeated_structures, DEFAULT_MIN_NODES};
use de_editor_core::{EditorState, PenNodeExt};
use jian_ops_schema::node::PenNode;

use super::{McpTool, ToolErrorCode, ToolOutcome};

pub struct LintDocument {
    issues: Vec<Issue>,
    subtree_ids: BTreeMap<String, BTreeSet<String>>,
}

impl McpTool for LintDocument {
    fn name(&self) -> &str {
        "lint_document"
    }

    fn call(&self, args: &BTreeMap<String, String>) -> ToolOutcome {
        let allowed = match args.get("nodeId").map(|value| value.trim()) {
            None | Some("") => None,
            Some(node_id) => match self.subtree_ids.get(node_id) {
                Some(ids) => Some(ids),
                None => {
                    return ToolOutcome::Err(
                        ToolErrorCode::InvalidArgument,
                        format!("nodeId not found: {node_id}"),
                    )
                }
            },
        };
        let issues: Vec<&Issue> = self
            .issues
            .iter()
            .filter(|issue| allowed.is_none_or(|ids| ids.contains(&issue.node_id)))
            .collect();
        ToolOutcome::OkJson(
            serde_json::json!({
                "count": issues.len(),
                "issues": issues,
            })
            .to_string(),
        )
    }
}

pub fn lint_document_snapshot(state: &EditorState) -> LintDocument {
    let mut issues = Vec::new();
    let mut subtree_ids = BTreeMap::new();
    let mut lint_page = |roots: &[PenNode]| {
        for root in roots {
            collect_subtree_ids(root, &mut subtree_ids);
            issues.extend(detect_all(root, &state.doc));
        }
        issues.extend(default_layer_name_issues(roots, state));
    };
    if let Some(pages) = state.doc.pages.as_ref() {
        for page in pages {
            lint_page(&page.children);
        }
    }
    lint_page(&state.doc.children);
    issues.extend(repeated_structure_issues(state));
    LintDocument {
        issues,
        subtree_ids,
    }
}

/// The default-layer-name Info findings of one page (#383). Naming is
/// editorial hygiene rather than a generation quality gate, so it is
/// reported here and not folded into `detect_all`, which the generation
/// pipelines' pre-validation and repair loops run. The page is named as a
/// whole with its instances expanded, exactly as `rename_layers` names it,
/// so each suggestion is the name `rename_layers` applies.
fn default_layer_name_issues(roots: &[PenNode], state: &EditorState) -> Vec<Issue> {
    if roots.is_empty() {
        return Vec::new();
    }
    let page = DocumentInstances::new(&state.doc).naming_page(roots);
    detect_default_layer_names(&page, document_locale(roots))
}

fn collect_subtree_ids(
    node: &PenNode,
    subtree_ids: &mut BTreeMap<String, BTreeSet<String>>,
) -> BTreeSet<String> {
    let mut ids = BTreeSet::new();
    ids.insert(node.id_str().to_string());
    if let Some(children) = node.children() {
        for child in children {
            ids.extend(collect_subtree_ids(child, subtree_ids));
        }
    }
    subtree_ids.insert(node.id_str().to_string(), ids.clone());
    ids
}

/// One `Info` finding per group of hand-built copies that could share a
/// component (#369), on the group's first member; the fix is the
/// `convert_to_instances` call. Never auto-applied (`Info`).
fn repeated_structure_issues(state: &EditorState) -> Vec<Issue> {
    find_repeated_structures(&state.doc, &state.components, None, DEFAULT_MIN_NODES)
        .into_iter()
        .map(|group| {
            let ids: Vec<&str> = group.node_ids.iter().map(|id| id.as_str()).collect();
            Issue {
                node_id: ids[0].to_string(),
                category: IssueCategory::RepeatedStructure,
                severity: IssueSeverity::Info,
                property: FixProperty::None,
                current_value: serde_json::json!({
                    "groupId": group.group_id,
                    "nodeIds": ids,
                    "nodesPerMember": group.nodes_per_member,
                }),
                suggested_value: serde_json::json!({
                    "tool": "convert_to_instances",
                    "arguments": {"groupId": group.group_id},
                }),
                reason: format!(
                    "{} similar layers could share a component",
                    group.node_ids.len()
                ),
            }
        })
        .collect()
}
