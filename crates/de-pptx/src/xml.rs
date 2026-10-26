//! `roxmltree` helpers that match elements by local name.
//!
//! PresentationML mixes the `p:`, `a:`, `r:`, `mc:` and vendor extension
//! namespaces, and writers are free to pick other prefixes. Matching the
//! local name keeps the walkers readable; the few places where two
//! namespaces share a local name (`p:txBody` vs `a:txBody` never appear in
//! the same parent) do not collide.

use roxmltree::{Document, Node, ParsingOptions};

use crate::error::PptxError;

/// Relationship-id attribute namespace (`r:embed`, `r:id`, `r:link`).
pub const REL_NS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

pub fn parse<'a>(src: &'a str, part: &str) -> Result<Document<'a>, PptxError> {
    let options = ParsingOptions {
        allow_dtd: false,
        ..ParsingOptions::default()
    };
    Document::parse_with_options(src, options).map_err(|e| PptxError::Xml {
        part: part.to_string(),
        detail: e.to_string(),
    })
}

pub fn child<'a, 'i>(node: Node<'a, 'i>, name: &str) -> Option<Node<'a, 'i>> {
    node.children()
        .find(|c| c.is_element() && c.tag_name().name() == name)
}

pub fn children<'a, 'i: 'a>(
    node: Node<'a, 'i>,
    name: &'a str,
) -> impl Iterator<Item = Node<'a, 'i>> + 'a {
    node.children()
        .filter(move |c| c.is_element() && c.tag_name().name() == name)
}

pub fn elements<'a, 'i: 'a>(node: Node<'a, 'i>) -> impl Iterator<Item = Node<'a, 'i>> + 'a {
    node.children().filter(Node::is_element)
}

/// Follow a path of local names: `path(sp, &["nvSpPr", "nvPr", "ph"])`.
pub fn path<'a, 'i>(node: Node<'a, 'i>, names: &[&str]) -> Option<Node<'a, 'i>> {
    names.iter().try_fold(node, |at, name| child(at, name))
}

/// First descendant (depth-first, self excluded) with this local name.
pub fn descendant<'a, 'i>(node: Node<'a, 'i>, name: &str) -> Option<Node<'a, 'i>> {
    node.descendants()
        .skip(1)
        .find(|c| c.is_element() && c.tag_name().name() == name)
}

pub fn attr_i64(node: Node<'_, '_>, name: &str) -> Option<i64> {
    node.attribute(name)?.trim().parse().ok()
}

pub fn attr_f64(node: Node<'_, '_>, name: &str) -> Option<f64> {
    node.attribute(name)?.trim().parse().ok()
}

/// OOXML booleans: `1` / `true` / `on` and `0` / `false` / `off`.
pub fn attr_bool(node: Node<'_, '_>, name: &str) -> Option<bool> {
    match node.attribute(name)?.trim() {
        "1" | "true" | "on" => Some(true),
        "0" | "false" | "off" => Some(false),
        _ => None,
    }
}

/// A relationship-id attribute (`r:embed="rId2"`), namespace-checked
/// because `id` / `link` are also plain attribute names elsewhere.
pub fn rel_attr<'a>(node: Node<'a, '_>, name: &str) -> Option<&'a str> {
    node.attribute((REL_NS, name))
}

/// Concatenated text of every `a:t` under `node`.
pub fn text_of(node: Node<'_, '_>) -> String {
    node.descendants()
        .filter(|n| n.is_element() && n.tag_name().name() == "t")
        .filter_map(|n| n.text())
        .collect()
}
