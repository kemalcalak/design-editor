//! Overlapping siblings (#265): two children of one parent whose laid-out
//! boxes cover each other, the case where wrapped text grows down over the
//! line below it. `snapshot_layout` reports them as `overlaps <id>` problems
//! and `batch_design` warns about the pairs a write creates.
//!
//! Only pairs that read as a mistake count, the same rules the
//! orchestrator's text-collision check uses: one side is visible text (a
//! near-transparent watermark numeral does not count), both axes overlap by
//! more than a few px, the overlap covers a real share of the smaller box,
//! and no non-text box holds the other whole, so backgrounds, cards and
//! full-bleed images behind their content stay quiet. Page roots are left to
//! root placement (#37).

use std::collections::BTreeMap;

use jian_scene::layout_scene::{NodeKind, SceneNode};

use super::layout_problems::Area;

/// Both axes must overlap by more than this many document px. This and the
/// two below match `de-orchestrator/src/text_collision.rs`.
const MIN_AXIS_OVERLAP_PX: f32 = 4.0;
/// The overlap must cover more than this share of the smaller box.
const MIN_OVERLAP_AREA_RATIO: f32 = 0.25;
/// Text at or below this opacity is a watermark, not a label.
const WATERMARK_MAX_OPACITY: f32 = 0.15;

fn is_visible_text(node: &SceneNode) -> bool {
    matches!(node.kind, NodeKind::Text)
        && node.opacity > WATERMARK_MAX_OPACITY
        && node.text.as_deref().is_some_and(|t| !t.trim().is_empty())
}

fn overlaps(a: &SceneNode, b: &SceneNode) -> bool {
    if a.hidden || b.hidden || !(is_visible_text(a) || is_visible_text(b)) {
        return false;
    }
    let (pa, pb) = (Area::painted(a), Area::painted(b));
    if pa.width() <= 0.0 || pa.height() <= 0.0 || pb.width() <= 0.0 || pb.height() <= 0.0 {
        return false;
    }
    // A shape holding the other whole is its background, card or image; a
    // text box holding another node is still a collision.
    if (pa.contains(pb) && !is_visible_text(a)) || (pb.contains(pa) && !is_visible_text(b)) {
        return false;
    }
    let shared = pa.intersect(pb);
    if shared.width() <= MIN_AXIS_OVERLAP_PX || shared.height() <= MIN_AXIS_OVERLAP_PX {
        return false;
    }
    let smaller = (pa.width() * pa.height()).min(pb.width() * pb.height());
    shared.width() * shared.height() > smaller * MIN_OVERLAP_AREA_RATIO
}

/// For one sibling list: each node id → the siblings it overlaps.
pub(crate) fn sibling_overlaps(siblings: &[SceneNode]) -> BTreeMap<String, Vec<String>> {
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (i, a) in siblings.iter().enumerate() {
        for b in &siblings[i + 1..] {
            if overlaps(a, b) {
                out.entry(a.id.clone()).or_default().push(b.id.clone());
                out.entry(b.id.clone()).or_default().push(a.id.clone());
            }
        }
    }
    out
}

/// Every overlapping sibling pair under the page roots, as `(a, b)` with
/// `a < b` so two layouts compare.
pub(crate) fn overlapping_pairs(roots: &[SceneNode]) -> Vec<(String, String)> {
    fn walk(siblings: &[SceneNode], out: &mut Vec<(String, String)>) {
        for (i, a) in siblings.iter().enumerate() {
            for b in &siblings[i + 1..] {
                if overlaps(a, b) {
                    let pair = if a.id <= b.id {
                        (a.id.clone(), b.id.clone())
                    } else {
                        (b.id.clone(), a.id.clone())
                    };
                    out.push(pair);
                }
            }
        }
        for node in siblings {
            walk(&node.children, out);
        }
    }
    let mut out = Vec::new();
    for root in roots {
        walk(&root.children, &mut out);
    }
    out
}

fn pairs_in(state: &de_editor_core::EditorState) -> std::collections::HashSet<(String, String)> {
    let scene = de_pen_loader::editor_state_to_layout_scene(state);
    scene
        .pages
        .iter()
        .flat_map(|page| overlapping_pairs(&page.children))
        .collect()
}

/// `"<a> overlaps <b>"` for each overlapping sibling pair that applying
/// `cmd` to `state` creates. Pairs already there are not repeated, so
/// layers someone stacked on purpose stay quiet; `allowOverlap` skips the
/// check.
pub(crate) fn new_overlap_warnings(
    state: &de_editor_core::EditorState,
    cmd: &de_editor_core::EditorCommand,
) -> Vec<String> {
    let mut trial = state.clone();
    if !trial.apply(cmd.clone()) {
        return Vec::new();
    }
    let before = pairs_in(state);
    let mut created: Vec<(String, String)> = pairs_in(&trial)
        .into_iter()
        .filter(|pair| !before.contains(pair))
        .collect();
    created.sort();
    created
        .into_iter()
        .map(|(a, b)| format!("{a} overlaps {b}"))
        .collect()
}
