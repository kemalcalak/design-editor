//! Siblings never get the same proposed name. Siblings that would share
//! one are each told apart by one of their own texts (`Row (HD export)`,
//! the first of them too, so the set reads evenly); one that nothing tells
//! apart keeps the plain name when it is still free and gets no proposal
//! otherwise. Nothing is ever numbered.

use std::collections::HashSet;

use de_i18n::Locale;
use jian_ops_schema::node::PenNode;

use super::facts::Facts;
use super::rules::collapse_whitespace;
use super::text_roles::labels;

/// One sibling's naming.
pub(crate) struct SiblingName<'a> {
    pub node: &'a PenNode,
    /// The meaningful name the layer keeps, if any.
    pub kept: Option<String>,
    /// The rules' proposal (only when the layer has no meaningful name).
    pub proposed: Option<String>,
    /// The proposal is applied (the layer is inside the targets). A
    /// sibling outside them keeps its current name, which is what its
    /// siblings must not repeat; its proposal only names its section.
    pub renamed: bool,
    /// The layer's name today.
    pub current: Option<String>,
}

impl SiblingName<'_> {
    /// The name the layer's descendants read as their section.
    pub(crate) fn final_name(&self) -> Option<&str> {
        self.kept.as_deref().or(self.proposed.as_deref())
    }

    /// The name this sibling holds that others must not repeat, before
    /// proposals are compared.
    fn holds(&self) -> Option<&str> {
        match (&self.kept, self.renamed) {
            (Some(kept), _) => Some(kept),
            (None, false) => self.current.as_deref(),
            (None, true) => None,
        }
    }
}

fn key(name: &str) -> String {
    collapse_whitespace(name).to_lowercase()
}

/// `name` told apart by `node`'s own texts that it does not already say,
/// in reading order, e.g. `Card (Starter)`.
fn qualified(node: &PenNode, name: &str, locale: Locale, facts: &Facts) -> Vec<String> {
    let lower = name.to_lowercase();
    labels(node, facts)
        .into_iter()
        .filter(|label| !lower.contains(&label.to_lowercase()))
        .map(|label| {
            de_i18n::translate_with(
                locale,
                "renameLayers.role.qualified",
                &[("name", name), ("detail", label.as_str())],
            )
        })
        .collect()
}

/// Make the applied proposals of `siblings` distinct from each other and
/// from the names siblings hold. `order` is the reading order (indexes
/// into `siblings`); `acceptable` vets a qualified name for its layer.
pub(crate) fn make_distinct(
    siblings: &mut [SiblingName<'_>],
    order: &[usize],
    locale: Locale,
    facts: &Facts,
    acceptable: impl Fn(&PenNode, &str) -> bool,
) {
    let mut taken: HashSet<String> = siblings
        .iter()
        .filter_map(|sibling| sibling.holds().map(key))
        .collect();
    // Applied proposals grouped by name, in reading order.
    let mut groups: Vec<(String, Vec<usize>)> = Vec::new();
    for &index in order {
        let sibling = &siblings[index];
        let Some(proposed) = sibling.proposed.as_deref().filter(|_| sibling.renamed) else {
            continue;
        };
        let name_key = key(proposed);
        match groups.iter_mut().find(|(k, _)| *k == name_key) {
            Some((_, members)) => members.push(index),
            None => groups.push((name_key, vec![index])),
        }
    }
    // A name only one sibling proposes, and nobody holds, stays as it is.
    let (clashing, alone): (Vec<_>, Vec<_>) = groups
        .into_iter()
        .partition(|(name_key, members)| members.len() > 1 || taken.contains(name_key));
    for (name_key, _) in &alone {
        taken.insert(name_key.clone());
    }
    for (name_key, members) in clashing {
        let mut untold = Vec::new();
        for index in members {
            let sibling = &siblings[index];
            let plain = sibling.proposed.clone().expect("grouped proposal");
            let told = qualified(sibling.node, &plain, locale, facts)
                .into_iter()
                .find(|name| !taken.contains(&key(name)) && acceptable(sibling.node, name));
            match told {
                Some(name) => {
                    taken.insert(key(&name));
                    siblings[index].proposed = Some(name);
                }
                None => untold.push(index),
            }
        }
        let mut plain_free = !taken.contains(&name_key);
        for index in untold {
            if plain_free {
                taken.insert(name_key.clone());
                plain_free = false;
            } else {
                siblings[index].proposed = None;
            }
        }
    }
}
