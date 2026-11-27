//! Siblings never get the same proposed name. Siblings that would share
//! one are each told apart by one of their own texts (`Row (HD export)`,
//! the first of them too, so the set reads evenly). Those without a text
//! to tell them apart are told apart together, by their fill colours when
//! every one differs (`Shape (Blue)`), else by where they sit among each
//! other (`Shape (Left)`, see `tell_apart`). A lone one keeps the plain
//! name when it is still free; past that, one keeps the plain name and the
//! rest get no proposal. Nothing is ever numbered.

use std::collections::HashSet;

use de_i18n::Locale;
use jian_ops_schema::node::PenNode;

use super::facts::Facts;
use super::placeholder::slot_thing;
use super::rules::collapse_whitespace;
use super::tell_apart::{fill_word, position_words};
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

/// `name` told apart by `detail`: `Card (Starter)`.
fn qualify(name: &str, detail: &str, locale: Locale) -> String {
    de_i18n::translate_with(
        locale,
        "renameLayers.role.qualified",
        &[("name", name), ("detail", detail)],
    )
}

/// `name` told apart by `node`'s own texts that it does not already say,
/// in reading order, e.g. `Card (Starter)`. A placeholder label (`Drop
/// logo`) is what a slot's name came from, so it says nothing new.
fn qualified(node: &PenNode, name: &str, locale: Locale, facts: &Facts) -> Vec<String> {
    let lower = name.to_lowercase();
    labels(node, facts)
        .into_iter()
        .filter(|label| !lower.contains(&label.to_lowercase()) && slot_thing(label).is_none())
        .map(|label| qualify(name, &label, locale))
        .collect()
}

/// Siblings that share one proposal and hold no text that tells them
/// apart, and what they are named in.
struct Untold<'s, 'n> {
    nodes: Vec<&'n PenNode>,
    /// Every sibling, in tree order: the grid a place is read in.
    siblings: Vec<&'n PenNode>,
    plain: &'s str,
    parent: Option<&'n PenNode>,
    locale: Locale,
}

impl Untold<'_, '_> {
    /// One qualified name per member: by fill when every fill has a word
    /// and no two match, else by position. `None` when neither tells every
    /// one apart with names still free.
    fn told_together(
        &self,
        taken: &HashSet<String>,
        acceptable: &impl Fn(&PenNode, &str) -> bool,
    ) -> Option<Vec<String>> {
        let usable = |details: Vec<String>| -> Option<Vec<String>> {
            let names: Vec<String> = details
                .iter()
                .map(|detail| qualify(self.plain, detail, self.locale))
                .collect();
            let mut seen = HashSet::new();
            let fine = names.iter().zip(&self.nodes).all(|(name, node)| {
                let name_key = key(name);
                !taken.contains(&name_key) && seen.insert(name_key) && acceptable(node, name)
            });
            fine.then_some(names)
        };
        let fills: Option<Vec<String>> = self
            .nodes
            .iter()
            .map(|node| fill_word(node, self.locale))
            .collect();
        fills.and_then(usable).or_else(|| {
            position_words(&self.nodes, &self.siblings, self.parent, self.locale).and_then(usable)
        })
    }
}

/// Make the applied proposals of `siblings` distinct from each other and
/// from the names siblings hold. `order` is the reading order (indexes
/// into `siblings`), `parent` the layer holding them (`None` for a page's
/// top-level layers); `acceptable` vets a qualified name for its layer.
pub(crate) fn make_distinct(
    siblings: &mut [SiblingName<'_>],
    order: &[usize],
    parent: Option<&PenNode>,
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
        if untold.len() > 1 || (!untold.is_empty() && !plain_free) {
            let plain = siblings[untold[0]]
                .proposed
                .clone()
                .expect("grouped proposal");
            let group = Untold {
                nodes: untold.iter().map(|&index| siblings[index].node).collect(),
                siblings: siblings.iter().map(|sibling| sibling.node).collect(),
                plain: &plain,
                parent,
                locale,
            };
            if let Some(names) = group.told_together(&taken, &acceptable) {
                for (index, name) in untold.into_iter().zip(names) {
                    taken.insert(key(&name));
                    siblings[index].proposed = Some(name);
                }
                continue;
            }
        }
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
