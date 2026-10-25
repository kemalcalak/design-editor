//! Page assembly: per-page payload build, root-frame clip marking and
//! the taffy layout pass that harvests computed rects.

use super::*;

/// Resolve one page into the paint payload used by the editor scene.
///
/// The full-document adapters above remain available to export/validation
/// callers. Interactive hosts use this focused entry point so switching pages
/// does not lay out every inactive page or retain their duplicate payloads.
pub(crate) fn pen_roots_to_page_payload(
    id: &str,
    name: &str,
    roots: &[PenNode],
    page_idx: usize,
    preserve_authored_geometry: bool,
) -> PagePayload {
    if preserve_authored_geometry {
        build_page_preserving_geometry(id, name, roots)
    } else {
        build_page(id, name, roots, page_idx)
    }
}

pub(super) fn build_page(id: &str, name: &str, roots: &[PenNode], page_idx: usize) -> PagePayload {
    let mut layout_rects: BTreeMap<String, [f32; 4]> = BTreeMap::new();
    for root in roots {
        compute_layout(root, &mut layout_rects);
    }
    let _ = page_idx;
    let mut children: Vec<NodePayload> = roots
        .iter()
        .map(|n| node_to_payload(n, &layout_rects))
        .collect();
    mark_root_frame_clips(roots, &mut children);
    PagePayload {
        id: id.to_string(),
        name: name.to_string(),
        children,
    }
}

pub(super) fn build_page_preserving_geometry(
    id: &str,
    name: &str,
    roots: &[PenNode],
) -> PagePayload {
    let rects = crate::authored_geometry::rects_for_roots(roots);
    let mut children: Vec<NodePayload> = roots.iter().map(|n| node_to_payload(n, &rects)).collect();
    mark_root_frame_clips(roots, &mut children);
    PagePayload {
        id: id.to_string(),
        name: name.to_string(),
        children,
    }
}

/// Legacy TS flattener parity (`document-flattener.ts`): ROOT frames whose
/// canonical `clipContent` field is absent clip like artboards. An explicit
/// `clipContent: false` is authored geometry and must remain open. Pair the
/// source roots with their payloads so the DTO can keep its compact `bool`.
pub(super) fn mark_root_frame_clips(roots: &[PenNode], children: &mut [NodePayload]) {
    for (root, child) in roots.iter().zip(children) {
        if matches!(
            root,
            PenNode::Frame(frame) if frame.container.clip_content.is_none()
        ) && child.kind == "frame"
        {
            child.clip_content = true;
        }
    }
}

/// Run jian-core's `LayoutEngine` on `root` and harvest absolute
/// rects per schema id into `out`. Each page root gets its own
/// `LayoutEngine` instance — Design Editor's canvas is infinite, so
/// roots don't share a coordinate frame.
///
/// Merge note (responsive-m1a into main): `LayoutEngine::node_rect`
/// now bakes a root's own authored `(base.x, base.y)` into its
/// returned absolute rect itself (`root_origins` / `is_origin_normalized`
/// in jian-core's `layout/mod.rs`, added for the responsive runtime's
/// multi-root canvas) — this used to be this function's OWN job (see
/// git history for the prior manual `+ root_ox + root_oy` add here).
/// Doing both doubled every harvested rect's origin (measured: a root
/// authored at doc (400, 60) resolved to scene (800, 120) — root_ox/
/// root_oy added on top of jian-core's own addition). `root_authored_
/// origin` stays exported for `de-host-native`'s Canvas Preview tap
/// translation, which still needs the authored origin as a standalone
/// value (not baked into a rect).
pub(super) fn compute_layout(root: &PenNode, out: &mut BTreeMap<String, [f32; 4]>) {
    let (root_w, root_h) = root_available_size(root);
    let mut tree = NodeTree::new();
    tree.insert_subtree(root.clone(), None);
    // Real-skia text measurement via jian-skia's `SkiaMeasure`
    // (paragraph shaper). The default `EstimateBackend` is a
    // character-count heuristic accurate to ~10% — for any
    // `fit_content` frame whose size depends on text length the
    // 10% error cascades through every flex parent. SkiaMeasure
    // matches what the canvas painter actually draws so the
    // engine + paint agree on widths.
    let mut engine = LayoutEngine::with_backend(layout_measure_backend());
    let Ok(taffy_roots) = engine.build(&tree) else {
        return;
    };
    let Some(root_id) = taffy_roots.first() else {
        return;
    };
    let Ok(text_widths) = engine.compute_with_text_widths(*root_id, (root_w, root_h)) else {
        return;
    };
    let mut wrapping = std::collections::HashSet::new();
    collect_wrapping_text_ids(root, &mut wrapping);
    let mut unrounded = Vec::new();
    // Walk every node by SlotMap iteration, looking up its absolute
    // rect via `node_rect`. Keyed back by the schema id we stashed
    // in `tree.by_id` during insertion. `node_rect` already carries
    // the root's authored canvas offset (see the merge note above),
    // so each design sits where the file placed it without any
    // further adjustment here.
    for (id_str, node_key) in tree.by_id.iter() {
        if let Some(rect) = engine.node_rect(*node_key) {
            out.insert(
                id_str.clone(),
                [
                    rect.origin.x,
                    rect.origin.y,
                    rect.size.width,
                    rect.size.height,
                ],
            );
            if wrapping.contains(id_str.as_str()) {
                if let Some(width) = text_widths.get(node_key) {
                    unrounded.push((id_str.clone(), rect.size.width, *width));
                }
            }
        }
    }
    // The repair walks sibling positions on the whole-pixel grid, so it
    // runs on the rounded boxes.
    crate::layout_repair::repair_fit_content_layout(root, out);
    // Wrapping text then takes taffy's unrounded width: for fixed-width
    // text that is the width the measure wrapped at, and the painter wraps
    // at the box width, so a pixel either way could move a line break the
    // box height was sized for (#202). A box the repair resized keeps the
    // repair's width.
    for (id, rounded, width) in unrounded {
        if let Some(rect) = out.get_mut(&id) {
            if rect[2] == rounded {
                rect[2] = width;
            }
        }
    }
}

/// Ids of the text nodes the painter wraps (`shapes::text_wraps`).
fn collect_wrapping_text_ids<'a>(node: &'a PenNode, out: &mut std::collections::HashSet<&'a str>) {
    use de_editor_core::pen_node_ext::PenNodeExt;
    if let PenNode::Text(text) = node {
        if super::shapes::text_wraps(text) {
            out.insert(text.base.id.as_str());
        }
    }
    for child in node.children().into_iter().flatten() {
        collect_wrapping_text_ids(child, out);
    }
}

/// This thread's layout text measure (shared, memoized). Hosts that run
/// their own layout pass over the document (Preview) take it too, so their
/// text boxes match the canvas.
pub fn layout_measure_backend() -> Rc<dyn MeasureBackend> {
    LAYOUT_MEASURE_BACKEND.with(Rc::clone)
}

#[cfg(test)]
#[path = "pages_wrap_width_tests.rs"]
mod wrap_width_tests;
