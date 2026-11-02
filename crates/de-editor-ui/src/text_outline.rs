//! Which text nodes are outlined (#282). Shared by the canvas painter, the
//! SVG writer and the PPTX writer so they agree on which strokes outline
//! glyphs: widget text (`text_input`, `select`, …) keeps its stroke as the
//! field border, and a non-positive width outlines nothing.

use crate::layout_scene::{SceneNode, SceneStroke};

/// The stroke `node`'s glyphs are outlined with, if any.
pub fn text_outline_stroke(node: &SceneNode) -> Option<SceneStroke> {
    node.stroke
        .filter(|stroke| node.widget.is_none() && stroke.width.is_finite() && stroke.width > 0.0)
}
