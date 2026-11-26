//! Replace text in find matches (#395): every matching text layer on every
//! page, as one undo step. Styled text keeps its runs (each run is
//! rewritten on its own); locked layers are left alone.

use jian_ops_schema::node::{PenNode, TextContent};

use super::FindQuery;
use crate::pen_node_ext::PenNodeExt;
use crate::{EditorState, NodeId};

/// Replace every case-insensitive occurrence of `needle` (already
/// lowercased) in `haystack`. `None` when there is none.
pub fn replace_case_insensitive(haystack: &str, needle: &str, replacement: &str) -> Option<String> {
    if needle.is_empty() {
        return None;
    }
    let needle: Vec<char> = needle.chars().collect();
    let chars: Vec<(usize, char)> = haystack.char_indices().collect();
    let mut out = String::with_capacity(haystack.len());
    let mut replaced = false;
    let mut i = 0;
    while i < chars.len() {
        if let Some(consumed) = match_at(&chars[i..], &needle) {
            out.push_str(replacement);
            replaced = true;
            i += consumed;
        } else {
            out.push(chars[i].1);
            i += 1;
        }
    }
    replaced.then_some(out)
}

/// How many haystack chars `needle` covers starting at `chars[0]`.
fn match_at(chars: &[(usize, char)], needle: &[char]) -> Option<usize> {
    let mut want = needle.iter();
    let mut pending = want.next()?;
    for (consumed, (_, c)) in chars.iter().enumerate() {
        for lower in c.to_lowercase() {
            if lower != *pending {
                return None;
            }
            match want.next() {
                Some(next) => pending = next,
                None => return Some(consumed + 1),
            }
        }
    }
    None
}

fn replace_content(content: &mut TextContent, needle: &str, replacement: &str) -> bool {
    match content {
        TextContent::Plain(text) => match replace_case_insensitive(text, needle, replacement) {
            Some(next) => {
                *text = next;
                true
            }
            None => false,
        },
        TextContent::Styled(segments) => {
            let mut changed = false;
            for segment in segments.iter_mut() {
                if let Some(next) = replace_case_insensitive(&segment.text, needle, replacement) {
                    segment.text = next;
                    changed = true;
                }
            }
            changed
        }
    }
}

fn replace_in_node(node: &mut PenNode, needle: &str, replacement: &str) -> bool {
    if node.base().locked == Some(true) {
        return false;
    }
    match node {
        PenNode::Text(text) => replace_content(&mut text.content, needle, replacement),
        PenNode::Note(note) | PenNode::Context(note) | PenNode::Prompt(note) => note
            .content
            .as_mut()
            .is_some_and(|content| replace_content(content, needle, replacement)),
        _ => false,
    }
}

fn walk(
    nodes: &mut [PenNode],
    targets: &std::collections::HashSet<&str>,
    needle: &str,
    replacement: &str,
) -> usize {
    let mut count = 0;
    for node in nodes.iter_mut() {
        if targets.contains(node.id_str()) && replace_in_node(node, needle, replacement) {
            count += 1;
        }
        if let Some(children) = node.children_mut() {
            count += walk(children, targets, needle, replacement);
        }
    }
    count
}

impl EditorState {
    /// Replace the query's text (its `text:` filter, else its free text) in
    /// the text content of `matches` with `replacement`, on every page, as
    /// one undo step. Returns how many layers changed.
    pub fn replace_text_in_layer_matches(
        &mut self,
        query: &FindQuery,
        matches: &[(usize, NodeId)],
        replacement: &str,
    ) -> usize {
        let Some(needle) = query.replace_needle().map(str::to_string) else {
            return 0;
        };
        let targets: std::collections::HashSet<&str> =
            matches.iter().map(|(_, id)| id.as_str()).collect();
        if targets.is_empty() {
            return 0;
        }
        let snapshot = self.snapshot_for_history();
        let count = match self.doc.pages.as_mut() {
            Some(pages) if !pages.is_empty() => pages
                .iter_mut()
                .map(|page| walk(&mut page.children, &targets, &needle, replacement))
                .sum(),
            _ => walk(&mut self.doc.children, &targets, &needle, replacement),
        };
        if count > 0 {
            self.history_push_past(snapshot);
        }
        count
    }
}
