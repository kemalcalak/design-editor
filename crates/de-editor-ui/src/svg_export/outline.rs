//! Corner outlines (#389) in the SVG export: a node whose scene carries a
//! smoothed or rounded outline is written as that outline (`<path>`), and
//! clips to it, instead of `<rect rx>` / `<polygon>`. The outline is
//! node-local, so it is moved into the document's coordinates here.

use super::{normalize_rect, xml_escape};
use crate::layout_scene::SceneNode;
use de_editor_core::corner_geometry::translate_path_data;

/// `d` (node-local) moved to `n`'s box origin, XML-escaped for an
/// attribute.
pub(super) fn placed_outline(n: &SceneNode, d: &str) -> String {
    let origin = normalize_rect(n.bounds).origin;
    xml_escape(&translate_path_data(
        d,
        f64::from(origin.x),
        f64::from(origin.y),
    ))
}

/// The node's outline as placed path data, when it has one.
pub(super) fn node_outline(n: &SceneNode) -> Option<String> {
    n.corner_path.as_deref().map(|o| placed_outline(n, &o.d))
}

/// The outline a stroked shape writes: the aligned stroke centre line for
/// an inside / outside stroke, else the outline itself.
pub(super) fn stroked_outline(n: &SceneNode) -> Option<String> {
    let outline = n.corner_path.as_deref()?;
    let stroked = n.stroke.is_some_and(|s| s.width > 0.0);
    let d = if stroked && !outline.stroke_d.is_empty() {
        &outline.stroke_d
    } else {
        &outline.d
    };
    Some(placed_outline(n, d))
}
