//! Panel edits of a note / context / prompt annotation (#12): its kind,
//! its content and a prompt's model. Each write is one `PatchNodeData`, so
//! it goes through the normal command path (revision, scene rebuild) and
//! the panel commit records one undo step. Switching the kind rewrites the
//! node's `type` tag; the shared `NoteNode` body carries over unchanged.

use jian_ops_schema::node::{AnnotationKind, TextContent};
use serde_json::{json, Value};

use crate::{EditorCommand, EditorState};

impl EditorState {
    /// The selected node's annotation kind, `None` for any other node.
    pub fn selected_annotation_kind(&self) -> Option<AnnotationKind> {
        self.selected_node()?.annotation().map(|(kind, _)| kind)
    }

    /// Turn the selected annotation into `kind`. False when nothing changes.
    pub fn set_selected_annotation_kind(&mut self, kind: AnnotationKind) -> bool {
        match self.selected_annotation_kind() {
            Some(current) if current != kind => self.patch_selected_annotation(json!({
                "type": kind.tag(),
            })),
            _ => false,
        }
    }

    /// Replace the selected annotation's content with plain `text`.
    pub fn set_selected_annotation_content(&mut self, text: &str) -> bool {
        let Some((_, body)) = self.selected_node().and_then(|node| node.annotation()) else {
            return false;
        };
        if matches!(&body.content, Some(TextContent::Plain(current)) if current == text)
            || (body.content.is_none() && text.is_empty())
        {
            return false;
        }
        self.patch_selected_annotation(json!({ "content": text }))
    }

    /// Set the model a prompt is meant for; a blank `model` clears it.
    pub fn set_selected_annotation_model(&mut self, model: &str) -> bool {
        let Some((_, body)) = self.selected_node().and_then(|node| node.annotation()) else {
            return false;
        };
        let model = model.trim();
        let next = (!model.is_empty()).then(|| model.to_string());
        if body.model == next {
            return false;
        }
        self.patch_selected_annotation(json!({ "model": next }))
    }

    fn patch_selected_annotation(&mut self, patch: Value) -> bool {
        let node_id = self.selection.anchor.clone();
        if !node_id.is_real() || !self.is_editable(&node_id) {
            return false;
        }
        self.apply(EditorCommand::PatchNodeData {
            node_id,
            patch_json: patch.to_string(),
            page_id: None,
        })
    }
}

#[cfg(test)]
#[path = "annotation_edit_tests.rs"]
mod tests;
