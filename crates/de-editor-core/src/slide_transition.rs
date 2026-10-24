//! Slide transitions in Present (#158).
//!
//! A slide (a top-level frame) may carry `slideTransition` — none, fade,
//! slide or push, with a duration and easing. Present plays the arriving
//! slide's transition on next, and the left slide's transition reversed on
//! previous, so a deck reads the same in both directions. A "deck"
//! transition is the same setting on every slide: the Slides panel and the
//! MCP tool write it to the selected slides, or to all of them.
//!
//! This module holds the pure parts: which transition a step plays, the
//! running transition's clock, where each of the two slides is at a given
//! progress, and the document writes. Hosts paint through
//! `de_preview_core`'s slide painter, which also applies the easing. PDF and
//! deck exports read boards, not this, so they are unaffected.

use crate::preview_slideshow::{active_page_boards, selected_page_boards};
use crate::transition_edit::MotionEasing;
use crate::{EditorCommand, EditorState, NodeId, PenNodeExt};
use jian_ops_schema::motion::{SlideTransition, SlideTransitionKind};

/// Duration a newly authored slide transition gets.
pub const DEFAULT_SLIDE_TRANSITION_MS: u64 = 400;

/// A transition in flight between two slides.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlideTransitionRun {
    /// The slide being left.
    pub from_board: String,
    pub kind: SlideTransitionKind,
    pub duration_ms: u64,
    /// `None` = a cubic-bezier easing the panel list can't name; hosts
    /// play it with the standard curve.
    pub easing: Option<MotionEasing>,
    pub started_ms: u64,
    /// Next (`true`) or previous: previous plays the motion mirrored.
    pub forward: bool,
}

impl SlideTransitionRun {
    /// Linear progress `0..1` at `now_ms`, or `None` once it has finished.
    pub fn progress(&self, now_ms: u64) -> Option<f32> {
        let elapsed = now_ms.saturating_sub(self.started_ms);
        (elapsed < self.duration_ms).then(|| elapsed as f32 / self.duration_ms as f32)
    }
}

/// Which slide a painted layer shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlideLayerBoard {
    /// The slide being left.
    From,
    /// The slide arriving (the one the slideshow now points at).
    To,
}

/// One slide's place in a transition frame: horizontal offset as a
/// fraction of the slide's on-screen width (`-1` = one width to the
/// left) and opacity. Painted in the order returned.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SlideLayer {
    pub board: SlideLayerBoard,
    pub offset_x: f32,
    pub opacity: f32,
}

/// The two layers of a transition at eased progress `t` (`0..1`).
/// `forward = false` mirrors the horizontal motion.
pub fn slide_layers(kind: SlideTransitionKind, t: f32, forward: bool) -> Vec<SlideLayer> {
    let t = t.clamp(0.0, 1.0);
    let side = if forward { 1.0 } else { -1.0 };
    let layer = |board, offset_x, opacity| SlideLayer {
        board,
        offset_x,
        opacity,
    };
    match kind {
        SlideTransitionKind::None => vec![layer(SlideLayerBoard::To, 0.0, 1.0)],
        SlideTransitionKind::Fade => vec![
            layer(SlideLayerBoard::From, 0.0, 1.0),
            layer(SlideLayerBoard::To, 0.0, t),
        ],
        // The arriving slide comes in over the one it covers.
        SlideTransitionKind::Slide => vec![
            layer(SlideLayerBoard::From, 0.0, 1.0),
            layer(SlideLayerBoard::To, side * (1.0 - t), 1.0),
        ],
        // The arriving slide pushes the old one out.
        SlideTransitionKind::Push => vec![
            layer(SlideLayerBoard::From, -side * t, 1.0),
            layer(SlideLayerBoard::To, side * (1.0 - t), 1.0),
        ],
    }
}

/// The authored `slideTransition` of top-level frame `board`.
pub fn board_slide_transition(state: &EditorState, board: &str) -> Option<SlideTransition> {
    state
        .active_children()
        .iter()
        .find(|node| node.id_str() == board)
        .and_then(|node| node.base().slide_transition.clone())
}

impl EditorState {
    /// Start the transition for a step that just moved the slideshow off
    /// `from_board`: the arriving slide's on next, the left slide's on
    /// previous. Returns whether a transition is now playing (`false` for
    /// none / zero length, and outside a slideshow).
    pub fn begin_slide_transition(&mut self, from_board: &str, forward: bool, now_ms: u64) -> bool {
        let Some(current) = self
            .preview_slideshow()
            .and_then(|slideshow| slideshow.current_board())
            .map(str::to_string)
        else {
            return false;
        };
        let owner = if forward {
            current.as_str()
        } else {
            from_board
        };
        let run = board_slide_transition(self, owner)
            .filter(|t| t.kind != SlideTransitionKind::None && t.duration_ms > 0)
            .map(|t| SlideTransitionRun {
                from_board: from_board.to_string(),
                kind: t.kind,
                duration_ms: t.duration_ms,
                easing: MotionEasing::from_schema(&t.easing),
                started_ms: now_ms,
                forward,
            });
        let started = run.is_some();
        self.editor_ui.preview.slide_transition = run;
        started
    }

    /// The transition playing at `now_ms`; clears a finished one.
    pub fn slide_transition_at(&mut self, now_ms: u64) -> Option<(SlideTransitionRun, f32)> {
        let run = self.editor_ui.preview.slide_transition.clone()?;
        match run.progress(now_ms) {
            Some(progress) => Some((run, progress)),
            None => {
                self.editor_ui.preview.slide_transition = None;
                None
            }
        }
    }

    /// The slides a transition edit targets: the selected slides, or all
    /// of the page's slides when none is selected (the deck).
    pub fn slide_transition_targets(&self) -> Vec<String> {
        let selected = selected_page_boards(self);
        if selected.is_empty() {
            active_page_boards(self)
        } else {
            selected
        }
    }

    /// The transition kind every target slide shares; `None` when they
    /// differ or there is no slide. A slide without the field is `none`.
    pub fn slides_transition_kind(&self) -> Option<SlideTransitionKind> {
        let mut kinds = self.slide_transition_targets().into_iter().map(|id| {
            board_slide_transition(self, &id)
                .map(|t| t.kind)
                .unwrap_or_default()
        });
        let first = kinds.next()?;
        kinds.all(|kind| kind == first).then_some(first)
    }

    /// Set `kind` (and `duration_ms` when given; otherwise each slide
    /// keeps its own, defaulting to [`DEFAULT_SLIDE_TRANSITION_MS`]) on
    /// the target slides — `ids`, or [`Self::slide_transition_targets`].
    /// `none` removes the field. One command, so one undo step. Returns
    /// whether anything changed.
    pub fn set_slides_transition(
        &mut self,
        ids: Option<&[String]>,
        kind: SlideTransitionKind,
        duration_ms: Option<u64>,
    ) -> bool {
        let targets = match ids {
            Some(ids) => ids.to_vec(),
            None => self.slide_transition_targets(),
        };
        let boards: Vec<_> = targets
            .into_iter()
            .map(|id| {
                let current = board_slide_transition(self, &id);
                (id, current)
            })
            .collect();
        let commands = slide_transition_commands(&boards, kind, duration_ms);
        if commands.is_empty() {
            return false;
        }
        self.apply(EditorCommand::Batch { commands })
    }
}

/// The page's top-level frames with their current transition — the input
/// [`slide_transition_commands`] takes.
pub fn page_slide_transitions(state: &EditorState) -> Vec<(String, Option<SlideTransition>)> {
    active_page_boards(state)
        .into_iter()
        .map(|id| {
            let current = board_slide_transition(state, &id);
            (id, current)
        })
        .collect()
}

/// The `slideTransition` patches that set `kind` on `boards` (each given
/// with its current transition): `duration_ms` when given, else the
/// slide's own duration, else [`DEFAULT_SLIDE_TRANSITION_MS`]; `none`
/// removes the field. Only slides that change get a patch. Shared by the
/// Slides panel and the MCP `set_slide_transition` tool.
pub fn slide_transition_commands(
    boards: &[(String, Option<SlideTransition>)],
    kind: SlideTransitionKind,
    duration_ms: Option<u64>,
) -> Vec<EditorCommand> {
    boards
        .iter()
        .filter_map(|(id, current)| {
            let next = (kind != SlideTransitionKind::None).then(|| {
                let mut next = current.clone().unwrap_or_default();
                next.kind = kind;
                if let Some(ms) = duration_ms {
                    next.duration_ms = ms.min(crate::motion_edit::MAX_MOTION_MS);
                } else if current.is_none() {
                    next.duration_ms = DEFAULT_SLIDE_TRANSITION_MS;
                }
                next
            });
            if next == *current {
                return None;
            }
            let value = match &next {
                Some(next) => serde_json::to_value(next).ok()?,
                None => serde_json::Value::Null,
            };
            Some(EditorCommand::PatchNodeData {
                node_id: NodeId::new(id.clone()),
                patch_json: serde_json::json!({ "slideTransition": value }).to_string(),
                page_id: None,
            })
        })
        .collect()
}

#[cfg(test)]
#[path = "slide_transition_tests.rs"]
mod tests;
