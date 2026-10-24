//! Scene nodes that exist on the editor canvas only (#12).
//!
//! Note / context / prompt annotations reach the scene as
//! `NodeKind::Other(tag)`. Exports (SVG, HTML, PPTX) skip them here; the
//! raster and PDF exports share the canvas node painter, which paints them
//! only inside the editor canvas (`widgets::canvas_annotation`). Code
//! generation skips them on the document side through
//! `PenNode::is_canvas_only`. A later editor-only kind joins both lists.

use crate::layout_scene::{NodeKind, SceneNode};

/// Whether a scene kind tag names a canvas-only node.
pub fn is_canvas_only_tag(tag: &str) -> bool {
    jian_ops_schema::node::AnnotationKind::from_tag(tag).is_some()
}

/// Whether `node` lives on the editor canvas only.
pub fn is_canvas_only(node: &SceneNode) -> bool {
    matches!(&node.kind, NodeKind::Other(tag) if is_canvas_only_tag(tag))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn annotation_tags_are_canvas_only() {
        for tag in ["note", "context", "prompt"] {
            assert!(is_canvas_only(&SceneNode::leaf(
                "n",
                NodeKind::Other(tag.into())
            )));
        }
        assert!(!is_canvas_only(&SceneNode::leaf(
            "i",
            NodeKind::Other("icon_font".into())
        )));
        assert!(!is_canvas_only(&SceneNode::leaf("t", NodeKind::Text)));
    }
}
