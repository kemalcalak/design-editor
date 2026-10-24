//! Motion-section edits (#154): node `animations` with `mount` /
//! `inView` / `hover` / `tap` triggers (#155) over the properties Preview
//! animates.
//!
//! The panel edits the common two-stop shape — one property tweened
//! from a value to a value: a number, or a hex colour for `fill` (#190).
//! Each write clones the node's `animations`, changes one declaration in
//! place (easing, iterations and unknown keys survive) and applies the
//! whole list through
//! `EditorCommand::PatchNodeData`. Undo: the panel arms are listed in
//! `updates_document`, so the host snapshots history around each write.

use crate::state::EditorState;
use crate::EditorCommand;
use jian_ops_schema::events::ExtraJson;
use jian_ops_schema::motion::{Keyframe, MotionTrigger, NodeAnimation, NodeAnimationFillMode};
use serde_json::Value;
use std::collections::BTreeMap;

/// The properties the Motion section offers — the motion set Preview
/// animates. `Fill` tweens a hex colour (#190); `stroke` stays
/// MCP-authored and shows as a custom row.
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
    Fill,
}

impl MotionProperty {
    pub const ALL: [MotionProperty; 10] = [
        MotionProperty::Opacity,
        MotionProperty::TranslateX,
        MotionProperty::TranslateY,
        MotionProperty::ScaleX,
        MotionProperty::ScaleY,
        MotionProperty::Rotation,
        MotionProperty::Width,
        MotionProperty::Height,
        MotionProperty::CornerRadius,
        MotionProperty::Fill,
    ];

    /// Whether the keyframes hold a hex colour rather than a number.
    pub fn is_color(self) -> bool {
        matches!(self, MotionProperty::Fill)
    }

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
            MotionProperty::Fill => "fill",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.key() == key)
    }

    /// The `(from, to)` a freshly picked property starts with: an
    /// entrance that settles on the node's resting value. `resting_fill`
    /// is the node's solid fill: a `Fill` tween starts there and ends 20%
    /// darker (a hover darken), or 20% lighter when it is already near
    /// black, so the two stops always differ; black when the node has
    /// none.
    pub fn default_stops(self, resting_fill: Option<[u8; 4]>) -> (MotionStop, MotionStop) {
        let number = |from: f64, to: f64| (MotionStop::Number(from), MotionStop::Number(to));
        match self {
            MotionProperty::Opacity => number(0.0, 1.0),
            MotionProperty::TranslateX => number(-24.0, 0.0),
            MotionProperty::TranslateY => number(24.0, 0.0),
            MotionProperty::ScaleX | MotionProperty::ScaleY => number(0.8, 1.0),
            MotionProperty::Rotation => number(-15.0, 0.0),
            // Sizes are absolute: a small grow the author then retypes.
            MotionProperty::Width => number(100.0, 120.0),
            MotionProperty::Height => number(40.0, 48.0),
            MotionProperty::CornerRadius => number(0.0, 12.0),
            MotionProperty::Fill => {
                let from = resting_fill.unwrap_or([0, 0, 0, 255]);
                let near_black = from[..3].iter().all(|&c| c < FILL_TWEEN_DARK_LIMIT);
                let shift = |c: u8| {
                    let c = f32::from(c);
                    let shifted = if near_black {
                        c + (255.0 - c) * FILL_TWEEN_SHIFT
                    } else {
                        c * (1.0 - FILL_TWEEN_SHIFT)
                    };
                    shifted.round() as u8
                };
                let to = [shift(from[0]), shift(from[1]), shift(from[2]), from[3]];
                (MotionStop::Color(from), MotionStop::Color(to))
            }
        }
    }
}

/// How far a new Fill tween moves its colour: 20% darker, or lighter for
/// a fill whose channels are all below [`FILL_TWEEN_DARK_LIMIT`].
const FILL_TWEEN_SHIFT: f32 = 0.2;
const FILL_TWEEN_DARK_LIMIT: u8 = 48;

/// One stop value of a simple animation: a number, or an RGBA colour for
/// a colour property.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MotionStop {
    Number(f64),
    Color([u8; 4]),
}

impl MotionStop {
    fn to_json(self) -> Value {
        match self {
            MotionStop::Number(value) => number(value),
            MotionStop::Color(rgba) => Value::String(format_motion_color(rgba)),
        }
    }

    /// The number, for a number stop.
    pub fn as_number(self) -> Option<f64> {
        match self {
            MotionStop::Number(value) => Some(value),
            MotionStop::Color(_) => None,
        }
    }
}

/// `#RRGGBB`, or `#RRGGBBAA` when the colour is not opaque — the form
/// the colour inputs show and the keyframes store.
pub fn format_motion_color(rgba: [u8; 4]) -> String {
    let [r, g, b, a] = rgba;
    if a == 255 {
        format!("#{r:02X}{g:02X}{b:02X}")
    } else {
        format!("#{r:02X}{g:02X}{b:02X}{a:02X}")
    }
}

fn parse_motion_color(text: &str) -> Option<[u8; 4]> {
    de_util::hex_color::parse_hex_rgba8(text, de_util::hex_color::HexOptions::LENIENT)
}

/// One field of a simple Motion row. From / To hold a colour for a
/// colour property (see [`MotionStop`]); Duration / Delay are always ms.
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
    pub from: MotionStop,
    pub to: MotionStop,
    pub duration_ms: u64,
    pub delay_ms: u64,
}

/// Read `animation` as a [`SimpleMotion`], or `None` when it has a shape
/// the panel can't express (more than two stops, several properties,
/// values of the wrong kind for the property, a property outside
/// [`MotionProperty`]).
pub fn simple_motion(animation: &NodeAnimation) -> Option<SimpleMotion> {
    let [first, last] = animation.keyframes.as_slice() else {
        return None;
    };
    if first.offset != 0.0 || last.offset != 1.0 {
        return None;
    }
    let (key, from) = single_value(&first.values)?;
    let (to_key, to) = single_value(&last.values)?;
    if key != to_key {
        return None;
    }
    let property = MotionProperty::from_key(key)?;
    Some(SimpleMotion {
        trigger: animation.trigger,
        property,
        from: stop_of(property, from)?,
        to: stop_of(property, to)?,
        duration_ms: animation.duration_ms,
        delay_ms: animation.delay_ms,
    })
}

fn single_value(values: &BTreeMap<String, Value>) -> Option<(&str, &Value)> {
    let mut iter = values.iter();
    let (key, value) = iter.next()?;
    if iter.next().is_some() {
        return None;
    }
    Some((key.as_str(), value))
}

/// `value` as a stop of `property`: a number for a number property, a hex
/// string for a colour one.
fn stop_of(property: MotionProperty, value: &Value) -> Option<MotionStop> {
    if property.is_color() {
        value
            .as_str()
            .and_then(parse_motion_color)
            .map(MotionStop::Color)
    } else {
        value.as_f64().map(MotionStop::Number)
    }
}

fn keyframe(offset: f32, property: MotionProperty, value: MotionStop) -> Keyframe {
    let mut values = BTreeMap::new();
    values.insert(property.key().to_string(), value.to_json());
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
    let (from, to) = MotionProperty::Opacity.default_stops(None);
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
/// value is not finite, the animation is not simple, or From / To of a
/// colour property (those take [`set_color`]).
fn set_field(animation: &mut NodeAnimation, field: MotionField, value: f64) -> bool {
    if !value.is_finite() {
        return false;
    }
    let Some(simple) = simple_motion(animation) else {
        return false;
    };
    if simple.property.is_color() && matches!(field, MotionField::From | MotionField::To) {
        return false;
    }
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

/// Write a hex colour into From or To (`field`) of a simple colour
/// animation. False when the text is not a hex colour, the field is not
/// From / To, the animation is not a simple colour tween, or the colour is
/// the one already there (a focus-and-blur of `#ff0000` must not rewrite
/// it as `#FF0000`).
fn set_color(animation: &mut NodeAnimation, field: MotionField, text: &str) -> bool {
    let Some(rgba) = parse_motion_color(text) else {
        return false;
    };
    let Some(simple) = simple_motion(animation) else {
        return false;
    };
    let (keyframe, current) = match field {
        MotionField::From => (0, simple.from),
        MotionField::To => (1, simple.to),
        MotionField::DurationMs | MotionField::DelayMs => return false,
    };
    if !simple.property.is_color() || current == MotionStop::Color(rgba) {
        return false;
    }
    animation.keyframes[keyframe].values.insert(
        simple.property.key().to_string(),
        MotionStop::Color(rgba).to_json(),
    );
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
        let resting_fill = self
            .selected_node()
            .and_then(crate::fills::first_solid_fill_hex)
            .and_then(parse_motion_color);
        self.edit_selected_animations(|list| {
            let Some(animation) = list.get_mut(index) else {
                return false;
            };
            match simple_motion(animation) {
                Some(simple) if simple.property == property => return false,
                Some(_) => {}
                None => return false,
            }
            let (from, to) = property.default_stops(resting_fill);
            animation.keyframes = vec![keyframe(0.0, property, from), keyframe(1.0, property, to)];
            true
        })
    }

    /// Write the hex colour `text` into From or To (`field`) of the simple
    /// colour animation at `index` (#190). Duration / Delay are numbers
    /// and go through [`Self::set_selected_animation_value`].
    pub fn set_selected_animation_color(
        &mut self,
        index: usize,
        field: MotionField,
        text: &str,
    ) -> bool {
        self.edit_selected_animations(|list| {
            list.get_mut(index)
                .is_some_and(|animation| set_color(animation, field, text))
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
