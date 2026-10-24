//! Loop-end finalize for one design-agent run, limited to the screens the
//! run created (#116).
//!
//! The run's host captures a [`WriteBaseline`] when the run starts. At the
//! end, only the top-level roots that were not on the page then get the
//! structural finalize, the unfilled-screen marking and the screen roster
//! the loop reports. A board someone made or imported next to them stays as
//! authored, even when the agent wrote into it: per-batch repairs already
//! cover what the agent wrote there.

use std::collections::BTreeSet;

use de_editor_core::write_scope::WriteBaseline;
use de_editor_core::EditorState;

use crate::repair_summary::RepairSummary;
use crate::unfilled_screens::{
    detect_unfilled_screens_for_roots, finalize_and_mark_unfilled_screens_for_roots,
    list_screen_candidates_for_roots,
};

/// What a run's finalize did.
#[derive(Debug, Default)]
pub struct AgentRunFinalize {
    /// What the structural passes checked and repaired.
    pub quality: RepairSummary,
    /// Names of the run's screens still unfilled (now marked on the canvas).
    pub unfilled: Vec<String>,
}

/// The run's screen roster: every screen it created, and the unfilled ones.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct AgentRunScreens {
    pub committed: Vec<String>,
    pub unfilled: Vec<String>,
}

/// Finalize the roots the run created since `run`, then mark the ones still
/// unfilled.
pub fn finalize_agent_run(state: &mut EditorState, run: &WriteBaseline) -> AgentRunFinalize {
    let quality = crate::apply_loop_finalize_for_roots(state, &run.new_roots_after(state));
    // Read the roots again: the finalizer may merge or replace roots.
    let roots = run.new_roots_after(state);
    let unfilled = finalize_and_mark_unfilled_screens_for_roots(state, in_roots(&roots));
    AgentRunFinalize { quality, unfilled }
}

/// The screens the run created since `run`, filled or not. Read-only.
pub fn agent_run_screens(state: &EditorState, run: &WriteBaseline) -> AgentRunScreens {
    let roots = run.new_roots_after(state);
    let names = |hits: Vec<crate::unfilled_screens::UnfilledScreen>| {
        hits.into_iter().map(|hit| hit.name).collect::<Vec<_>>()
    };
    AgentRunScreens {
        committed: names(list_screen_candidates_for_roots(state, in_roots(&roots))),
        unfilled: names(detect_unfilled_screens_for_roots(state, in_roots(&roots))),
    }
}

fn in_roots(roots: &BTreeSet<String>) -> impl Fn(&str) -> bool + '_ {
    move |id| roots.contains(id)
}

#[cfg(test)]
#[path = "agent_run_finalize_tests.rs"]
mod tests;
