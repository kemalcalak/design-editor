//! Pointer-triggered motion (#155): `hover` and `tap` animations.
//!
//! A `hover` animation plays forward while the mouse is over its node (or
//! any of its descendants — hovering a button's label hovers the button)
//! and plays back to the node's own look when the pointer leaves. A `tap`
//! animation does the same for a press: forward on down, back on release.
//!
//! Forward goes from wherever the property is now to the last keyframe —
//! through every authored stop when it starts from rest — so a hover that
//! leaves mid-way reverses smoothly from where it was. Reverse goes back to
//! the authored value with the same duration and easing; the delay only
//! applies going forward.

use crate::animation::{sample_scene_property, PreviewAnimationState};
use crate::motion::{easing_from_schema, PreviewMotionState};
use de_editor_ui::layout_scene::{LayoutScene, SceneNode};
use jian_core::action::animation_registry::animatable_property_registry;
use jian_core::action::services::{
    AnimationDirection, AnimationFillMode, AnimationProperty, AnimationRequest,
};
use jian_ops_schema::motion::{
    MotionPreference, MotionTrigger, NodeAnimation, P1_MOTION_PROPERTIES,
};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// Which way a pointer animation is going.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Direction {
    Forward,
    Back,
}

impl PreviewMotionState {
    /// Whether the mounted document has any `hover` / `tap` animation.
    /// Walked once per screen (the cache resets with the screen), so a
    /// document without pointer motion pays nothing per pointer event.
    pub(crate) fn has_pointer_motion(&self, runtime: &jian_core::Runtime) -> bool {
        if let Some(cached) = self.pointer_motion_cached() {
            return cached;
        }
        let found = crate::motion::node_declarations(runtime, false)
            .iter()
            .any(|(_, _, animations)| animations.iter().any(|a| a.trigger.is_pointer()));
        self.cache_pointer_motion(found);
        found
    }

    /// Start or reverse every `hover` / `tap` animation whose node the
    /// pointer just entered / left or pressed / released. `hovered` and
    /// `pressed` are the nodes under the pointer and under each press.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn observe_pointer(
        &self,
        runtime: &jian_core::Runtime,
        scene: &LayoutScene,
        animation: &PreviewAnimationState,
        effective: MotionPreference,
        hovered: Option<&str>,
        pressed: &[&str],
        now_ms: u64,
    ) {
        let Some(page) = scene.active_page() else {
            return;
        };
        let hovered_chain = hovered
            .map(|id| ancestor_chain(&page.children, id))
            .unwrap_or_default();
        let mut pressed_chain = BTreeSet::new();
        for id in pressed {
            pressed_chain.extend(ancestor_chain(&page.children, id));
        }
        let mut active_now = BTreeSet::new();
        for (target, _, animations) in crate::motion::node_declarations(runtime, false) {
            for (index, declaration) in animations.iter().enumerate() {
                let on = match declaration.trigger {
                    // A press does not end a hover (CSS `:hover` stays on
                    // during `:active`): the interaction state drops the
                    // hover record while the mouse is down.
                    MotionTrigger::Hover => {
                        hovered_chain.contains(&target) || pressed_chain.contains(&target)
                    }
                    MotionTrigger::Tap => pressed_chain.contains(&target),
                    MotionTrigger::Mount | MotionTrigger::InView => continue,
                };
                let key = format!("{target}:{index}");
                let was = self.pointer_was_active(&key);
                if on {
                    active_now.insert(key);
                }
                let direction = match (was, on) {
                    (false, true) => Direction::Forward,
                    (true, false) => Direction::Back,
                    _ => continue,
                };
                self.play_pointer_animation(
                    &target,
                    declaration,
                    scene,
                    animation,
                    effective,
                    direction,
                    now_ms,
                );
            }
        }
        self.set_pointer_active(active_now);
    }

    #[allow(clippy::too_many_arguments)]
    fn play_pointer_animation(
        &self,
        target: &str,
        declaration: &NodeAnimation,
        scene: &LayoutScene,
        animation: &PreviewAnimationState,
        effective: MotionPreference,
        direction: Direction,
        now_ms: u64,
    ) {
        let mut by_property: BTreeMap<String, Vec<(f32, Value)>> = BTreeMap::new();
        for keyframe in &declaration.keyframes {
            for (property, value) in &keyframe.values {
                by_property
                    .entry(property.clone())
                    .or_default()
                    .push((keyframe.offset, value.clone()));
            }
        }
        for (name, stops) in by_property {
            if !P1_MOTION_PROPERTIES.contains(&name.as_str()) {
                tracing::warn!(target = %target, property = %name, "preview motion property rejected");
                continue;
            }
            let Some(descriptor) = animatable_property_registry().get(&name) else {
                continue;
            };
            let property = AnimationProperty::from_registered(&name, descriptor.apply);
            // `scene` carries no animation overrides: this is the node's
            // own (authored + live widget / binding) value.
            let Some(base) = sample_scene_property(scene, target, &property) else {
                continue;
            };
            let Some((_, last)) = stops.last().cloned() else {
                continue;
            };
            let current = animation.current_override(target, &property);
            let destination = match direction {
                Direction::Forward => last,
                Direction::Back => base.clone(),
            };
            if effective == MotionPreference::Reduced {
                animation.set_instant(target, &property, destination, Some(&base));
                continue;
            }
            if !self.track_budget_available(animation, target, &property) {
                tracing::warn!(target = %target, property = %name, "preview motion track budget exceeded");
                animation.set_instant(target, &property, destination, Some(&base));
                continue;
            }
            let from = current.clone().unwrap_or_else(|| base.clone());
            // From rest, play the authored stops; mid-way, head straight
            // for the destination from where the property is.
            let stops = (direction == Direction::Forward && current.is_none() && stops.len() > 2)
                .then_some(stops);
            let delay = match direction {
                Direction::Forward => declaration.delay_ms,
                Direction::Back => 0,
            };
            let request = AnimationRequest {
                target: target.to_owned(),
                property: property.clone(),
                from: Some(from.clone()),
                to: destination,
                stops,
                duration_ms: declaration.duration_ms,
                delay_ms: 0,
                easing: easing_from_schema(&declaration.easing),
                iterations: 1,
                direction: AnimationDirection::Normal,
                fill_mode: AnimationFillMode::Forwards,
                requested_at_ms: now_ms.saturating_add(delay),
            };
            if animation.start_motion(request, from, Some(base), now_ms)
                && animation.has_active(target, &property)
            {
                self.remember_track(target, &property);
            }
        }
    }
}

/// `id` and every ancestor of it, by schema id. Empty when `id` is not in
/// `nodes`.
fn ancestor_chain(nodes: &[SceneNode], id: &str) -> BTreeSet<String> {
    fn walk(nodes: &[SceneNode], id: &str, path: &mut Vec<String>) -> bool {
        for node in nodes {
            path.push(node.id.clone());
            if node.id == id || walk(&node.children, id, path) {
                return true;
            }
            path.pop();
        }
        false
    }
    let mut path = Vec::new();
    if walk(nodes, id, &mut path) {
        path.into_iter().collect()
    } else {
        BTreeSet::new()
    }
}

impl crate::session::PreviewSession {
    /// Re-read the pointer after runtime input so hover / tap motion starts
    /// at the event timestamp, not the next paint.
    pub(crate) fn sync_pointer_motion(&self) {
        if !self.motion.lifecycle_started() || !self.motion.has_pointer_motion(&self.runtime) {
            return;
        }
        let scene = self.overlay_runtime_state_without_animation(&self.scene);
        self.observe_pointer_motion(&scene);
    }

    pub(crate) fn observe_pointer_motion(&self, scene: &LayoutScene) {
        if !self.motion.has_pointer_motion(&self.runtime) {
            return;
        }
        self.motion.observe_pointer(
            &self.runtime,
            scene,
            &self.animation,
            self.effective_motion_preference(),
            self.interaction.hovered_node(),
            &self.interaction.pressed_nodes(),
            self.last_now_ms,
        );
    }
}
