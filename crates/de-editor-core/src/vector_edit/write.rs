//! Subpaths → document. Runs only after a real edit: the first write
//! replaces an imported `d` with one baked in the parent-local frame.

use super::{source::subpaths_bounds, Subpath};
use crate::path_edit::anchors_to_path_data;
use crate::pen_node_ext::PenNodeExt;
use jian_ops_schema::node::PenNode;

/// Serialize subpaths into one `d` (one `M … [Z]` run per subpath).
pub fn subpaths_to_path_data(subs: &[Subpath]) -> String {
    subs.iter()
        .filter(|s| !s.anchors.is_empty())
        .map(|s| anchors_to_path_data(&s.anchors, s.closed))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Write `subs` into the path node: `d`, and for a single subpath also
/// `anchors` / `closed` (so every anchor-based tool keeps working);
/// a compound path keeps `d` only. The box is re-fitted to the
/// handle-aware bounds at scale 1, which makes the canvas fit an
/// identity. A stale `iconId` is dropped — the shape is no longer the
/// catalogue icon. False for a non-path node.
pub(crate) fn write_subpaths(node: &mut PenNode, subs: &[Subpath]) -> bool {
    let (x, y, w, h) = subpaths_bounds(subs);
    {
        let PenNode::Path(path) = &mut *node else {
            return false;
        };
        path.d = Some(subpaths_to_path_data(subs));
        if let [single] = subs {
            path.anchors = Some(single.anchors.clone());
            path.closed = Some(single.closed);
        } else {
            path.anchors = None;
            path.closed = None;
        }
        path.icon_id = None;
    }
    node.base_mut().x = Some(x);
    node.base_mut().y = Some(y);
    node.set_width_px(w);
    node.set_height_px(h);
    true
}
