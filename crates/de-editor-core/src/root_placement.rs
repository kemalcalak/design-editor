//! Where new page-root content lands when an agent inserts it over MCP.
//!
//! Pure geometry over laid-out bounds: the caller measures the page's
//! existing roots and the freshly inserted ones (the host has the layout
//! engine; this crate does not), and gets back where each new root's
//! bounds should start. New content goes right of everything, aligned to
//! the top, like the orchestrator's `next_root_insert_position`:
//!
//! - A root inserted WITHOUT a position always lands right of the page's
//!   content plus [`ROOT_GAP`], at the top-most root's y (0,0 on an empty
//!   page).
//! - Roots inserted WITH a position keep it unless one of them overlaps an
//!   existing root. Then the whole positioned group moves together, so the
//!   agent's arrangement between them survives, to the same right-of-content
//!   slot. `allow_overlap` keeps them where they were asked to be.

use crate::geometry::{union_of, DocRect};

/// Horizontal gap between existing content and a placed root. Same value
/// as the orchestrator's `FOLLOW_ON_ROOT_GAP` / `SCREEN_ROOT_GAP`.
pub const ROOT_GAP: f64 = 80.0;

/// One freshly inserted root: its bounds as laid out where the insert put
/// it, and whether the agent asked for a position (`x` or `y`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IncomingRoot {
    pub bounds: DocRect,
    pub positioned: bool,
}

/// For each incoming root (same order), the origin its bounds should move
/// to, or `None` when it stays where it is.
pub fn plan_root_placement(
    existing: &[DocRect],
    incoming: &[IncomingRoot],
    allow_overlap: bool,
) -> Vec<Option<(f64, f64)>> {
    let mut plan = vec![None; incoming.len()];
    let mut occupied = existing.to_vec();

    let positioned: Vec<usize> = (0..incoming.len())
        .filter(|&i| incoming[i].positioned)
        .collect();
    let group_overlaps = positioned.iter().any(|&i| {
        existing
            .iter()
            .any(|other| overlaps(&incoming[i].bounds, other))
    });
    let group_shift = if group_overlaps && !allow_overlap {
        let group = union_of(positioned.iter().map(|&i| incoming[i].bounds));
        match (next_slot(existing), (!group.is_empty()).then_some(group)) {
            (Some((x, y)), Some(bbox)) => Some((x - bbox.x, y - bbox.y)),
            _ => None,
        }
    } else {
        None
    };
    for &i in &positioned {
        let mut bounds = incoming[i].bounds;
        if let Some((dx, dy)) = group_shift {
            bounds.x += dx;
            bounds.y += dy;
            plan[i] = Some((bounds.x, bounds.y));
        }
        occupied.push(bounds);
    }

    for (i, root) in incoming.iter().enumerate() {
        if root.positioned {
            continue;
        }
        let (x, y) = next_slot(&occupied).unwrap_or((0.0, 0.0));
        if (x, y) != (root.bounds.x, root.bounds.y) {
            plan[i] = Some((x, y));
        }
        occupied.push(DocRect {
            x,
            y,
            ..root.bounds
        });
    }
    plan
}

/// Right of everything in `occupied` plus [`ROOT_GAP`], at its top edge.
fn next_slot(occupied: &[DocRect]) -> Option<(f64, f64)> {
    let bbox = union_of(occupied.iter().copied());
    (!bbox.is_empty()).then_some((bbox.x + bbox.w + ROOT_GAP, bbox.y))
}

/// Interiors intersect; boxes that only share an edge sit side by side.
fn overlaps(a: &DocRect, b: &DocRect) -> bool {
    a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h
}

#[cfg(test)]
#[path = "root_placement_tests.rs"]
mod root_placement_tests;
