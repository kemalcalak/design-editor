//! The page walk behind the rules' proposals: each run of siblings is
//! named together (texts by the roles of their unit, no two siblings
//! alike), then walked into with the section name its descendants read.

use std::collections::HashSet;

use de_i18n::Locale;
use jian_ops_schema::node::PenNode;

use crate::node_util::{base, children, node_id};

use super::rules::{collapse_whitespace, infer_role, text_content, Place, Role};
use super::text_roles::{is_unit, ordered_children, UnitRoles};
use super::unique::{make_distinct, SiblingName};
use super::{has_meaningful_name, is_default_layer_name};

/// A layer that has no meaningful name, with the rules' proposal for it
/// (`None` when the rules have nothing better to offer).
pub(super) struct Candidate {
    pub node_id: String,
    pub current: Option<String>,
    pub proposed: Option<String>,
}

struct Walk<'a> {
    targets: Option<&'a HashSet<String>>,
    locale: Locale,
    out: Vec<Candidate>,
}

/// A run of siblings: their parent (`None` for a page's top-level layers),
/// their depth below the page, the section they are named in, the roles
/// of their unit's texts and whether they sit inside a target.
struct Siblings<'s, 'n> {
    parent: Option<&'n PenNode>,
    nodes: &'n [PenNode],
    depth: usize,
    section: Option<&'s str>,
    unit: &'s UnitRoles,
    in_target: bool,
}

impl Siblings<'_, '_> {
    fn place(&self, index: usize) -> Place<'_> {
        Place {
            parent: self.parent,
            index,
            depth: self.depth,
        }
    }

    /// Indexes of the siblings in reading order.
    fn reading_order(&self) -> Vec<usize> {
        let Some(parent) = self.parent else {
            return (0..self.nodes.len()).collect();
        };
        ordered_children(parent)
            .into_iter()
            .filter_map(|kid| self.nodes.iter().position(|node| std::ptr::eq(node, kid)))
            .collect()
    }
}

impl Walk<'_> {
    /// The role and the proposal (before siblings are compared) of each
    /// sibling.
    fn name_run<'n>(&self, run: &Siblings<'_, 'n>) -> (Vec<Option<Role>>, Vec<SiblingName<'n>>) {
        let roles: Vec<Option<Role>> = run
            .nodes
            .iter()
            .enumerate()
            .map(|(index, node)| {
                run.unit
                    .role_of(node)
                    .or_else(|| infer_role(node, run.place(index)))
            })
            .collect();
        let mut names: Vec<SiblingName<'n>> = run
            .nodes
            .iter()
            .zip(&roles)
            .map(|(node, role)| {
                let current = base(node).name.clone();
                if has_meaningful_name(node) {
                    return SiblingName {
                        node,
                        kept: current,
                        proposed: None,
                    };
                }
                let proposed = role
                    .as_ref()
                    .map(|role| role.render(self.locale, run.section))
                    .filter(|proposed| acceptable(node, proposed, current.as_deref()));
                SiblingName {
                    node,
                    kept: None,
                    proposed,
                }
            })
            .collect();
        make_distinct(
            &mut names,
            &run.reading_order(),
            self.locale,
            |node, name| acceptable(node, name, base(node).name.as_deref()),
        );
        (roles, names)
    }

    /// Name a run of siblings together, record the proposals inside the
    /// targets and walk into each sibling.
    fn siblings(&mut self, run: Siblings<'_, '_>) {
        let (roles, names) = self.name_run(&run);
        for ((node, name), role) in run.nodes.iter().zip(&names).zip(&roles) {
            let id = node_id(node);
            let in_target = run.in_target || self.targets.is_none_or(|t| t.contains(id));
            if in_target && name.kept.is_none() {
                self.out.push(Candidate {
                    node_id: id.to_string(),
                    current: base(node).name.clone(),
                    proposed: name.proposed.clone(),
                });
            }
            let kids = children(node);
            if kids.is_empty() {
                continue;
            }
            // A section-like layer (or any wide child of a screen) names the
            // section its descendants sit in: `Hero / Title`.
            let final_name = name.final_name();
            let names_section = role.as_ref().is_some_and(Role::is_section_like)
                || (run.depth == 1 && final_name.is_some());
            let own_unit = is_unit(node, run.depth).then(|| UnitRoles::of(node, run.depth));
            self.siblings(Siblings {
                parent: Some(node),
                nodes: kids,
                depth: run.depth + 1,
                section: if names_section {
                    final_name
                } else {
                    run.section
                },
                unit: own_unit.as_ref().unwrap_or(run.unit),
                in_target,
            });
        }
    }
}

/// A proposal is worth making when it is not itself a default name (a
/// text layer named after its content is fine) and it changes something.
fn acceptable(node: &PenNode, proposed: &str, current: Option<&str>) -> bool {
    if proposed.trim().is_empty() || current.map(str::trim) == Some(proposed.trim()) {
        return false;
    }
    !is_default_layer_name(proposed)
        || text_content(node).is_some_and(|content| content == collapse_whitespace(proposed))
}

/// Every layer under `roots` without a meaningful name (inside `targets`
/// when given), in tree order, with the rules' proposal.
pub(super) fn candidates(
    roots: &[PenNode],
    targets: Option<&HashSet<String>>,
    locale: Locale,
) -> Vec<Candidate> {
    let mut walk = Walk {
        targets,
        locale,
        out: Vec::new(),
    };
    walk.siblings(Siblings {
        parent: None,
        nodes: roots,
        depth: 0,
        section: None,
        unit: &UnitRoles::default(),
        in_target: false,
    });
    walk.out
}
