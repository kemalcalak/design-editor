//! Marking a top-level frame as a Preview screen, or as the start screen
//! (#223). The property panel's Interactions rows and MCP `set_screen`
//! share these writes.
//!
//! A marker is the frame's `screen` route path. Marking takes a path made
//! from the frame's name (any script, unique on the page); the start
//! screen is `/`, and the frame that held `/` before moves to a path of
//! its own name. All writes are `PatchNodeData` on the active page,
//! applied as one `Batch` (one undo step).

use std::collections::BTreeSet;

use jian_ops_schema::node::PenNode;

use crate::{EditorCommand, EditorState, NodeId, PenNodeExt};

/// What to do with a frame's `screen` marker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenMarkerEdit {
    /// Route it at a path of its own (kept when it already has one).
    Mark,
    /// Make it the start screen (`/`).
    Start,
    /// Remove its marker.
    Remove,
}

/// Why a marker edit was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScreenMarkerError {
    /// Only a top-level frame of the page can be a screen.
    NotTopLevelFrame { node_id: String },
}

impl std::fmt::Display for ScreenMarkerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotTopLevelFrame { node_id } => {
                write!(f, "{node_id} is not a top-level frame of the page")
            }
        }
    }
}

impl std::error::Error for ScreenMarkerError {}

/// The writes `edit` makes to frame `node_id` among the page's `roots`;
/// empty when the marker already is what was asked.
pub fn screen_marker_commands(
    roots: &[PenNode],
    node_id: &str,
    edit: ScreenMarkerEdit,
) -> Result<Vec<EditorCommand>, ScreenMarkerError> {
    let refused = || ScreenMarkerError::NotTopLevelFrame {
        node_id: node_id.to_owned(),
    };
    let current = roots
        .iter()
        .find_map(|node| match node {
            PenNode::Frame(frame) if frame.base.id == node_id => Some(frame.screen.clone()),
            _ => None,
        })
        .ok_or_else(refused)?;
    let mut used: BTreeSet<String> = roots
        .iter()
        .filter_map(|node| match node {
            PenNode::Frame(frame) => frame.screen.clone(),
            _ => None,
        })
        .collect();
    let mut out = Vec::new();
    match edit {
        ScreenMarkerEdit::Mark if current.is_none() => {
            let path = own_path(roots, node_id, &mut used);
            out.push(patch(node_id, Some(&path)));
        }
        ScreenMarkerEdit::Start if current.as_deref() != Some("/") => {
            let holder = roots.iter().find_map(|node| match node {
                PenNode::Frame(frame) if frame.screen.as_deref() == Some("/") => {
                    Some(frame.base.id.clone())
                }
                _ => None,
            });
            if let Some(holder) = holder {
                let path = own_path(roots, &holder, &mut used);
                out.push(patch(&holder, Some(&path)));
            }
            out.push(patch(node_id, Some("/")));
        }
        ScreenMarkerEdit::Remove if current.is_some() => out.push(patch(node_id, None)),
        _ => {}
    }
    Ok(out)
}

impl EditorState {
    /// Apply `edit` to the selected top-level frame; `true` when the
    /// document changed.
    pub fn edit_selected_screen_marker(&mut self, edit: ScreenMarkerEdit) -> bool {
        let id = self.selection.anchor.as_str().to_owned();
        match screen_marker_commands(self.active_children(), &id, edit) {
            Ok(commands) if !commands.is_empty() => self.apply(EditorCommand::Batch { commands }),
            _ => false,
        }
    }
}

fn patch(node_id: &str, path: Option<&str>) -> EditorCommand {
    EditorCommand::PatchNodeData {
        node_id: NodeId::new(node_id.to_owned()),
        patch_json: serde_json::json!({ "screen": path }).to_string(),
        page_id: None,
    }
}

/// A free `/slug` from the frame's name, recorded in `used`.
fn own_path(roots: &[PenNode], node_id: &str, used: &mut BTreeSet<String>) -> String {
    let name = roots
        .iter()
        .find(|node| node.id_str() == node_id)
        .and_then(|node| node.base().name.clone())
        .unwrap_or_default();
    unique_route_path(&name, used)
}

/// A route path made from `name` that `used` doesn't hold yet (`/slug`,
/// then `/slug-2`, …; `/screen` for a name without words), recorded in
/// `used`. Preview's auto-wire names routes the same way.
pub fn unique_route_path(name: &str, used: &mut BTreeSet<String>) -> String {
    let slug = route_slug(name);
    let base = if slug.is_empty() {
        "screen".to_owned()
    } else {
        slug
    };
    let mut path = format!("/{base}");
    let mut suffix = 2;
    while used.contains(&path) {
        path = format!("/{base}-{suffix}");
        suffix += 1;
    }
    used.insert(path.clone());
    path
}

/// Lower-cased words of `name` in any script, joined by hyphens.
pub fn route_slug(name: &str) -> String {
    name.split(is_slug_separator)
        .filter(|word| !word.is_empty())
        .map(str::to_lowercase)
        .collect::<Vec<_>>()
        .join("-")
}

fn is_slug_separator(c: char) -> bool {
    c.is_whitespace()
        || c.is_ascii_punctuation()
        || matches!(
            c,
            '·' | '—'
                | '–'
                | '…'
                | '«'
                | '»'
                | '“'
                | '”'
                | '‘'
                | '’'
                | '、'
                | '。'
                | '，'
                | '：'
        )
}

#[cfg(test)]
#[path = "screen_marker_edit_tests.rs"]
mod tests;
