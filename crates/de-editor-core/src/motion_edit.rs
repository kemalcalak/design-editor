//! Motion-section edits (#154): node `animations` with `mount` /
//! `inView` / `hover` / `tap` triggers (#155) over the properties Preview
//! animates.
//!
//! The panel edits the common two-stop shape — one property tweened
//! from a value to a value. Each write clones the node's `animations`,
//! changes one declaration in place (easing, iterations and unknown keys
//! survive) and applies the whole list through
//! `EditorCommand::PatchNodeData`. Undo: the panel arms are listed in
//! `updates_document`, so the host snapshots history around each write.

use crate::state::EditorState;
use crate::EditorCommand;
use jian_ops_schema::events::ExtraJson;
use jian_ops_schema::motion::{Keyframe, MotionTrigger, NodeAnimation, NodeAnimationFillMode};
use serde_json::Value;
use std::collections::BTreeMap;

/// The properties the Motion section offers — the motion set Preview
/// animates, minus the colour ones a number field can't express (`fill` /
/// `stroke` stay MCP-authored and show as a custom row).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MotionProperty {
    Opacity,
    TranslateX,
    TranslateY,
    ScaleX,
    ScaleY,
    Rotation,
    Width,
    Height,
    CornerRadius,
}

impl MotionProperty {
    pub const ALL: [MotionProperty; 9] = [
        MotionProperty::Opacity,
        MotionProperty::TranslateX,
        MotionProperty::TranslateY,
        MotionProperty::ScaleX,
        MotionProperty::ScaleY,
        MotionProperty::Rotation,
        MotionProperty::Width,
        MotionProperty::Height,
        MotionProperty::CornerRadius,
    ];

    /// The keyframe key, as `ANIMATION_REGISTRY_PROPERTIES` names it.
    pub fn key(self) -> &'static str {
        match self {
            MotionProperty::Opacity => "opacity",
            MotionProperty::TranslateX => "translateX",
            MotionProperty::TranslateY => "translateY",
            MotionProperty::ScaleX => "scaleX",
            MotionProperty::ScaleY => "scaleY",
            MotionProperty::Rotation => "rotation",
            MotionProperty::Width => "width",
            MotionProperty::Height => "height",
            MotionProperty::CornerRadius => "cornerRadius",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.key() == key)
    }

    /// The `(from, to)` a freshly picked property starts with: an
    /// entrance that settles on the node's resting value.
    pub fn default_range(self) -> (f64, f64) {
        match self {
            MotionProperty::Opacity => (0.0, 1.0),
            MotionProperty::TranslateX => (-24.0, 0.0),
            MotionProperty::TranslateY => (24.0, 0.0),
            MotionProperty::ScaleX | MotionProperty::ScaleY => (0.8, 1.0),
            MotionProperty::Rotation => (-15.0, 0.0),
            // Sizes are absolute: a small grow the author then retypes.
            MotionProperty::Width => (100.0, 120.0),
            MotionProperty::Height => (40.0, 48.0),
            MotionProperty::CornerRadius => (0.0, 12.0),
        }
    }
}

/// One numeric field of a Motion row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MotionField {
    From,
    To,
    DurationMs,
    DelayMs,
}

/// Duration of the animation the Motion "+" adds.
pub const DEFAULT_MOTION_DURATION_MS: u64 = 300;
/// Longest duration / delay the fields accept (one minute).
pub const MAX_MOTION_MS: u64 = 60_000;

/// A two-stop, one-property animation, as the panel edits it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SimpleMotion {
    pub trigger: MotionTrigger,
    pub property: MotionProperty,
    pub from: f64,
    pub to: f64,
    pub duration_ms: u64,
    pub delay_ms: u64,
}

/// Read `animation` as a [`SimpleMotion`], or `None` when it has a shape
/// the panel can't express (more than two stops, several properties,
/// non-number values, a property outside [`MotionProperty`]).
pub fn simple_motion(animation: &NodeAnimation) -> Option<SimpleMotion> {
    let [first, last] = animation.keyframes.as_slice() else {
        return None;
    };
    if first.offset != 0.0 || last.offset != 1.0 {
        return None;
    }
    let (key, from) = single_number(&first.values)?;
    let (to_key, to) = single_number(&last.values)?;
    if key != to_key {
        return None;
    }
    Some(SimpleMotion {
        trigger: animation.trigger,
        property: MotionProperty::from_key(key)?,
        from,
        to,
        duration_ms: animation.duration_ms,
        delay_ms: animation.delay_ms,
    })
}

fn single_number(values: &BTreeMap<String, Value>) -> Option<(&str, f64)> {
    let mut iter = values.iter();
    let (key, value) = iter.next()?;
    if iter.next().is_some() {
        return None;
    }
    Some((key.as_str(), value.as_f64()?))
}

fn keyframe(offset: f32, property: MotionProperty, value: f64) -> Keyframe {
    let mut values = BTreeMap::new();
    values.insert(property.key().to_string(), number(value));
    Keyframe {
        offset,
        values,
        extra: ExtraJson::default(),
    }
}

fn number(value: f64) -> Value {
    serde_json::Number::from_f64(value)
        .map(Value::Number)
        .unwrap_or(Value::Null)
}

/// The fade-in the Motion "+" adds.
pub fn default_animation() -> NodeAnimation {
    let (from, to) = MotionProperty::Opacity.default_range();
    NodeAnimation {
        trigger: MotionTrigger::Mount,
        keyframes: vec![
            keyframe(0.0, MotionProperty::Opacity, from),
            keyframe(1.0, MotionProperty::Opacity, to),
        ],
        duration_ms: DEFAULT_MOTION_DURATION_MS,
        delay_ms: 0,
        easing: Default::default(),
        iterations: 1,
        fill_mode: NodeAnimationFillMode::Forwards,
        once: true,
        extra: ExtraJson::default(),
    }
}

/// How a motion number shows in its input: integral values without a
/// fraction, the rest with full `f64` precision so a focus-and-blur
/// writes back exactly what was there.
pub fn format_motion_number(value: f64) -> String {
    if value.fract() == 0.0 && value.abs() < 1e15 {
        format!("{}", value as i64)
    } else {
        value.to_string()
    }
}

/// Write `value` into the simple animation's `field`. False when the
/// value is not finite or the animation is not simple.
fn set_field(animation: &mut NodeAnimation, field: MotionField, value: f64) -> bool {
    if !value.is_finite() {
        return false;
    }
    let Some(simple) = simple_motion(animation) else {
        return false;
    };
    let ms = || value.round().clamp(0.0, MAX_MOTION_MS as f64) as u64;
    match field {
        // Only the value changes: keyframe-level keys (a per-stop easing,
        // future fields) survive.
        MotionField::From => {
            animation.keyframes[0]
                .values
                .insert(simple.property.key().to_string(), number(value));
        }
        MotionField::To => {
            animation.keyframes[1]
                .values
                .insert(simple.property.key().to_string(), number(value));
        }
        // A zero-length tween never plays; keep at least one frame.
        MotionField::DurationMs => animation.duration_ms = ms().max(1),
        MotionField::DelayMs => animation.delay_ms = ms(),
    }
    true
}

impl EditorState {
    /// The anchor node's authored animations (empty when none).
    pub fn selected_animations(&self) -> Vec<NodeAnimation> {
        self.selected_node()
            .and_then(|node| node.motion_declarations().1.cloned())
            .unwrap_or_default()
    }

    /// Append the default fade-in to the anchor node.
    pub fn add_selected_animation(&mut self) -> bool {
        self.edit_selected_animations(|list| {
            list.push(default_animation());
            true
        })
    }

    /// Remove the animation at `index`.
    pub fn remove_selected_animation(&mut self, index: usize) -> bool {
        self.edit_selected_animations(|list| {
            if index >= list.len() {
                return false;
            }
            list.remove(index);
            true
        })
    }

    /// Set the trigger (`mount` / `inView` / `hover` / `tap`) of the
    /// animation at `index`.
    pub fn set_selected_animation_trigger(&mut self, index: usize, trigger: MotionTrigger) -> bool {
        self.edit_selected_animations(|list| match list.get_mut(index) {
            Some(animation) if animation.trigger != trigger => {
                animation.trigger = trigger;
                true
            }
            _ => false,
        })
    }

    /// Animate `property` in the simple animation at `index`, starting
    /// from that property's default range.
    pub fn set_selected_animation_property(
        &mut self,
        index: usize,
        property: MotionProperty,
    ) -> bool {
        self.edit_selected_animations(|list| {
            let Some(animation) = list.get_mut(index) else {
                return false;
            };
            match simple_motion(animation) {
                Some(simple) if simple.property == property => return false,
                Some(_) => {}
                None => return false,
            }
            let (from, to) = property.default_range();
            animation.keyframes = vec![keyframe(0.0, property, from), keyframe(1.0, property, to)];
            true
        })
    }

    /// Write one numeric field of the simple animation at `index`.
    pub fn set_selected_animation_value(
        &mut self,
        index: usize,
        field: MotionField,
        value: f64,
    ) -> bool {
        self.edit_selected_animations(|list| {
            list.get_mut(index)
                .is_some_and(|animation| set_field(animation, field, value))
        })
    }

    fn edit_selected_animations(
        &mut self,
        edit: impl FnOnce(&mut Vec<NodeAnimation>) -> bool,
    ) -> bool {
        let node_id = self.selection.anchor.clone();
        if !node_id.is_real() || !self.is_editable(&node_id) || self.selected_node().is_none() {
            return false;
        }
        let mut list = self.selected_animations();
        let before = list.clone();
        if !edit(&mut list) || list == before {
            return false;
        }
        let value = if list.is_empty() {
            Value::Null
        } else {
            // Refuse the edit rather than write `null` over every animation.
            let Ok(value) = serde_json::to_value(&list) else {
                return false;
            };
            value
        };
        self.apply(EditorCommand::PatchNodeData {
            node_id,
            patch_json: serde_json::json!({ "animations": value }).to_string(),
            page_id: None,
        })
    }
}

#[cfg(test)]
#[path = "motion_edit_tests.rs"]
mod tests;
