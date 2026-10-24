//! The loop finalizer limited to a set of top-level roots.
//!
//! [`apply_loop_finalize_counted`](crate::apply_loop_finalize_counted) rewrites
//! every root on the active page. A design-agent run or a file-backed MCP
//! session should only finalize the roots it created, so a design a person
//! made or imported next to them stays as authored (#107, #116).
//!
//! The finalizer runs on a copy whose active page holds only those roots;
//! its recorded commands then replay onto the real document as one batch, so
//! undo history sees one ordinary step. A replay that is rejected, touches
//! another root, or diverges from the finalizer's own result is refused.

use std::collections::BTreeSet;

use de_editor_core::{EditorCommand, EditorState, PenNodeExt};

use crate::record_loop_finalize_counted;
use crate::repair_summary::RepairSummary;

#[path = "loop_finalize_scoped_error.rs"]
mod loop_finalize_scoped_error;
pub use loop_finalize_scoped_error::ScopedFinalizeError;

/// The result of finalizing some roots.
#[derive(Debug)]
pub struct ScopedFinalize {
    /// The finalized document, or `None` when the finalizer changed nothing.
    pub state: Option<EditorState>,
    pub summary: RepairSummary,
}

/// Finalize only the active-page roots whose id is in `roots`, leaving
/// every other root exactly as it was. `Ok(None)` when none of `roots` is on
/// the active page.
pub fn record_loop_finalize_for_roots(
    state: &EditorState,
    roots: &BTreeSet<String>,
) -> Result<Option<ScopedFinalize>, ScopedFinalizeError> {
    let Some(scoped) = scoped_state(state, roots) else {
        return Ok(None);
    };
    let recorded = record_loop_finalize_counted(&scoped).map_err(ScopedFinalizeError::Record)?;
    let finalized = if recorded.commands.is_empty() {
        None
    } else {
        Some(replay_on_page(state, &recorded, roots).ok_or(ScopedFinalizeError::Replay)?)
    };
    Ok(Some(ScopedFinalize {
        state: finalized,
        summary: recorded.summary,
    }))
}

/// [`apply_loop_finalize_counted`](crate::apply_loop_finalize_counted) over
/// the active-page roots whose id is in `roots`. When they are the whole page
/// the plain finalizer runs; otherwise [`record_loop_finalize_for_roots`]
/// does, and a finalize it refuses leaves the document unchanged and reports
/// nothing repaired.
pub fn apply_loop_finalize_for_roots(
    state: &mut EditorState,
    roots: &BTreeSet<String>,
) -> RepairSummary {
    let children = state.active_children();
    if !children.is_empty() && children.iter().all(|root| roots.contains(root.id_str())) {
        return crate::apply_loop_finalize_counted(state);
    }
    match record_loop_finalize_for_roots(state, roots) {
        Ok(Some(finalized)) => {
            if let Some(next) = finalized.state {
                *state = next;
            }
            finalized.summary
        }
        Ok(None) => RepairSummary::default(),
        Err(error) => {
            eprintln!("design-editor: loop finalize of the generated roots skipped: {error}");
            RepairSummary::default()
        }
    }
}

/// A clone of `state` whose active page holds only `roots`, or `None` when
/// none of them is on the page.
fn scoped_state(state: &EditorState, roots: &BTreeSet<String>) -> Option<EditorState> {
    if !state
        .active_children()
        .iter()
        .any(|root| roots.contains(root.id_str()))
    {
        return None;
    }
    let mut scoped = state.clone();
    scoped
        .active_children_mut()
        .retain(|root| roots.contains(root.id_str()));
    Some(scoped)
}

/// Apply the finalizer's recorded commands to a copy of the whole document.
/// `None` unless the batch applies, every root outside `roots` is left
/// exactly as it was, and the finalized roots come out as the finalizer
/// produced them.
fn replay_on_page(
    state: &EditorState,
    recorded: &crate::RecordedLoopFinalize,
    roots: &BTreeSet<String>,
) -> Option<EditorState> {
    let outside_roots = |state: &EditorState| -> Vec<serde_json::Value> {
        state
            .active_children()
            .iter()
            .filter(|root| !roots.contains(root.id_str()))
            .filter_map(|root| serde_json::to_value(root).ok())
            .collect()
    };
    let mut next = state.clone();
    if !next.apply(EditorCommand::Batch {
        commands: recorded.commands.clone(),
    }) {
        return None;
    }
    if outside_roots(&next) != outside_roots(state) {
        return None;
    }
    let finalized_roots_match = recorded.state.active_children().iter().all(|expected| {
        next.active_children()
            .iter()
            .find(|root| root.id_str() == expected.id_str())
            .is_some_and(|root| {
                serde_json::to_value(root).ok() == serde_json::to_value(expected).ok()
            })
    });
    finalized_roots_match.then_some(next)
}

#[cfg(test)]
#[path = "loop_finalize_scoped_tests.rs"]
mod tests;
