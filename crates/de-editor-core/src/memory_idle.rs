//! Trim memory once the editor has been idle (#456).
//!
//! Work that walks large structures to give memory back (the undo history's
//! byte budget, the allocator's free pages) must not run while a person
//! pans, edits or undoes, so hosts run it here: [`IdleTrim`] remembers the
//! last activity, asks for one wake [`IDLE_TRIM_AFTER_MS`] later, and trims
//! once per quiet spell (again only after new activity).

use crate::history_budget::{HISTORY_BYTE_BUDGET, HISTORY_MIN_STEPS};
use crate::state::EditorState;

/// Quiet time before an idle trim.
pub const IDLE_TRIM_AFTER_MS: u64 = 30_000;

/// What one idle trim did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct IdleTrimReport {
    /// Undo steps dropped to fit the history's byte budget.
    pub history_steps_dropped: usize,
}

/// The idle-trim clock a host keeps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct IdleTrim {
    last_activity_ms: u64,
    /// Set once this quiet spell was trimmed; cleared by activity.
    trimmed: bool,
}

impl IdleTrim {
    /// Something happened (input, an edit, a repaint with new content).
    pub fn note_activity(&mut self, now_ms: u64) {
        self.last_activity_ms = now_ms;
        self.trimmed = false;
    }

    /// When the host should wake to trim; `None` once this quiet spell was
    /// trimmed.
    pub fn deadline_ms(&self) -> Option<u64> {
        (!self.trimmed).then(|| self.last_activity_ms.saturating_add(IDLE_TRIM_AFTER_MS))
    }

    /// Trim `state` if the editor has been quiet long enough and this spell
    /// was not trimmed yet.
    pub fn run_if_due(&mut self, state: &mut EditorState, now_ms: u64) -> Option<IdleTrimReport> {
        let due = self.deadline_ms().is_some_and(|at| now_ms >= at);
        if !due {
            return None;
        }
        self.trimmed = true;
        Some(IdleTrimReport {
            history_steps_dropped: state
                .trim_history_to_budget(HISTORY_BYTE_BUDGET, HISTORY_MIN_STEPS),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trims_once_per_quiet_spell() {
        let mut state = EditorState::new();
        let mut idle = IdleTrim::default();
        idle.note_activity(1_000);
        assert_eq!(idle.deadline_ms(), Some(1_000 + IDLE_TRIM_AFTER_MS));
        assert_eq!(
            idle.run_if_due(&mut state, 1_000 + IDLE_TRIM_AFTER_MS - 1),
            None
        );
        assert!(idle
            .run_if_due(&mut state, 1_000 + IDLE_TRIM_AFTER_MS)
            .is_some());
        assert_eq!(idle.deadline_ms(), None, "no second wake in the same spell");
        assert_eq!(idle.run_if_due(&mut state, 10 * IDLE_TRIM_AFTER_MS), None);
        idle.note_activity(50_000);
        assert_eq!(idle.deadline_ms(), Some(50_000 + IDLE_TRIM_AFTER_MS));
    }
}
