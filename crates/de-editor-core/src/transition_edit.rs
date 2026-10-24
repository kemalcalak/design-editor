//! Node-level `transition` edits (#176): the paint-change tween Preview
//! plays when a node's runtime paint (fill, opacity, …) changes.
//!
//! The Motion section authors its duration and easing. Each write clones
//! the node's `transition`, changes one field (the `properties` filter
//! and unknown keys survive) and applies it through
//! `EditorCommand::PatchNodeData`, like `motion_edit`.

use crate::state::EditorState;
use crate::EditorCommand;
use jian_ops_schema::motion::{Easing, Transition};
use serde_json::Value;

/// The easings the panel offers, in list order. A cubic-bezier easing
/// authored elsewhere shows as "Custom" and is kept until one is picked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MotionEasing {
    Standard,
    Emphasized,
    EmphasizedDecelerate,
    EmphasizedAccelerate,
    Linear,
    Ease,
    EaseIn,
    EaseOut,
    EaseInOut,
}

impl MotionEasing {
    pub const ALL: [MotionEasing; 9] = [
        MotionEasing::Standard,
        MotionEasing::Emphasized,
        MotionEasing::EmphasizedDecelerate,
        MotionEasing::EmphasizedAccelerate,
        MotionEasing::Linear,
        MotionEasing::Ease,
        MotionEasing::EaseIn,
        MotionEasing::EaseOut,
        MotionEasing::EaseInOut,
    ];

    pub fn to_schema(self) -> Easing {
        match self {
            MotionEasing::Standard => Easing::Standard,
            MotionEasing::Emphasized => Easing::Emphasized,
            MotionEasing::EmphasizedDecelerate => Easing::EmphasizedDecelerate,
            MotionEasing::EmphasizedAccelerate => Easing::EmphasizedAccelerate,
            MotionEasing::Linear => Easing::Linear,
            MotionEasing::Ease => Easing::Ease,
            MotionEasing::EaseIn => Easing::EaseIn,
            MotionEasing::EaseOut => Easing::EaseOut,
            MotionEasing::EaseInOut => Easing::EaseInOut,
        }
    }

    /// `None` for a cubic-bezier easing (shown as "Custom").
    pub fn from_schema(easing: &Easing) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|candidate| candidate.to_schema() == *easing)
    }
}

/// The node-level transition as the panel shows it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransitionSummary {
    pub duration_ms: u64,
    /// `None` = a cubic-bezier easing the list can't name.
    pub easing: Option<MotionEasing>,
}

impl TransitionSummary {
    pub fn of(transition: &Transition) -> Self {
        Self {
            duration_ms: transition.duration_ms,
            easing: MotionEasing::from_schema(&transition.easing),
        }
    }
}

impl EditorState {
    /// The anchor node's authored `transition`, if any.
    pub fn selected_transition(&self) -> Option<Transition> {
        self.selected_node()
            .and_then(|node| node.motion_declarations().0.cloned())
    }

    /// Give the anchor node the default transition (200 ms, standard).
    pub fn add_selected_transition(&mut self) -> bool {
        if self.selected_transition().is_some() {
            return false;
        }
        self.write_selected_transition(Some(Transition::default()))
    }

    /// Remove the anchor node's transition.
    pub fn remove_selected_transition(&mut self) -> bool {
        if self.selected_transition().is_none() {
            return false;
        }
        self.write_selected_transition(None)
    }

    /// Set the transition's duration, clamped to 0..=60 s.
    pub fn set_selected_transition_duration(&mut self, value: f64) -> bool {
        if !value.is_finite() {
            return false;
        }
        let Some(mut transition) = self.selected_transition() else {
            return false;
        };
        let ms = value
            .round()
            .clamp(0.0, crate::motion_edit::MAX_MOTION_MS as f64) as u64;
        if transition.duration_ms == ms {
            return false;
        }
        transition.duration_ms = ms;
        self.write_selected_transition(Some(transition))
    }

    /// Set the transition's easing.
    pub fn set_selected_transition_easing(&mut self, easing: MotionEasing) -> bool {
        let Some(mut transition) = self.selected_transition() else {
            return false;
        };
        let easing = easing.to_schema();
        if transition.easing == easing {
            return false;
        }
        transition.easing = easing;
        self.write_selected_transition(Some(transition))
    }

    fn write_selected_transition(&mut self, transition: Option<Transition>) -> bool {
        let node_id = self.selection.anchor.clone();
        if !node_id.is_real() || !self.is_editable(&node_id) || self.selected_node().is_none() {
            return false;
        }
        let value = match transition {
            None => Value::Null,
            // Refuse the edit rather than write `null` over the transition.
            Some(transition) => match serde_json::to_value(&transition) {
                Ok(value) => value,
                Err(_) => return false,
            },
        };
        self.apply(EditorCommand::PatchNodeData {
            node_id,
            patch_json: serde_json::json!({ "transition": value }).to_string(),
            page_id: None,
        })
    }
}

#[cfg(test)]
#[path = "transition_edit_tests.rs"]
mod tests;
