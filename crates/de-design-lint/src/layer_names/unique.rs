//! Siblings never get the same proposed name: a repeat is qualified with
//! one of the layer's own headings (`Row (Fast sync)`), and dropped when
//! nothing tells it apart, rather than numbered.

use std::collections::HashSet;

use de_i18n::Locale;
use jian_ops_schema::node::PenNode;

use super::rules::collapse_whitespace;
use super::text_roles::labels;

/// One sibling's naming: its name when it keeps it, else the proposal.
pub(crate) struct SiblingName<'a> {
    pub node: &'a PenNode,
    /// The name the layer keeps (meaningful), if any.
    pub kept: Option<String>,
    /// The rules' proposal (only when the layer has no meaningful name).
    pub proposed: Option<String>,
}

impl SiblingName<'_> {
    /// The name the layer ends up with.
    pub(crate) fn final_name(&self) -> Option<&str> {
        self.kept.as_deref().or(self.proposed.as_deref())
    }
}

fn key(name: &str) -> String {
    collapse_whitespace(name).to_lowercase()
}

/// `name` told apart by one of `node`'s own headings that it does not
/// already say, e.g. `Card (Starter)`.
fn qualified(node: &PenNode, name: &str, locale: Locale) -> Vec<String> {
    let lower = name.to_lowercase();
    labels(node)
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

/// Make the proposals of `siblings` distinct from each other and from the
/// names siblings keep. `order` is the reading order (indexes into
/// `siblings`): an earlier sibling keeps the plain name. `acceptable`
/// vets a qualified alternative for its layer.
pub(crate) fn make_distinct(
    siblings: &mut [SiblingName<'_>],
    order: &[usize],
    locale: Locale,
    acceptable: impl Fn(&PenNode, &str) -> bool,
) {
    let mut taken: HashSet<String> = siblings
        .iter()
        .filter_map(|sibling| sibling.kept.as_deref().map(key))
        .collect();
    for &index in order {
        let sibling = &mut siblings[index];
        let Some(proposed) = sibling.proposed.take() else {
            continue;
        };
        let chosen = if taken.contains(&key(&proposed)) {
            qualified(sibling.node, &proposed, locale)
                .into_iter()
                .find(|name| !taken.contains(&key(name)) && acceptable(sibling.node, name))
        } else {
            Some(proposed)
        };
        if let Some(name) = &chosen {
            taken.insert(key(name));
        }
        sibling.proposed = chosen;
    }
}
