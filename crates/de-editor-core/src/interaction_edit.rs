//! Interactions-section edits (#154): the trigger / action / transition
//! vocabulary the right panel authors, and the `EditorState` writers
//! behind it.
//!
//! Every write rebuilds the node's whole `events` block and applies it
//! through `EditorCommand::PatchNodeData`. `PatchNodeData` is a shallow
//! merge, so a patch of `{"events":{"onTap":…}}` would replace the block
//! and silently drop sibling handlers (`onChange`, `onSwipe`, …); writing
//! the full block keeps them.
//!
//! Undo: the panel arms are listed in `updates_document`, so the host
//! snapshots history around each write and drops a no-op.
//!
//! [`InteractionMenuTarget`] is popover state, not an edit; it lives here
//! so `EditorUiState` (core) can hold it next to the trigger vocabulary.

use crate::pen_node_ext::PenNodeExt;
use crate::state::EditorState;
use crate::EditorCommand;
use jian_ops_schema::events::{Action, ActionList, EventHandlers};

/// The gestures the panel authors, in the order the section lists them.
/// Each maps onto the handler key the preview runtime fires for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InteractionTrigger {
    /// `onTap`.
    Tap,
    /// `onHoverEnter` (pointer devices only in Preview).
    Hover,
    /// `onLongPress`.
    LongPress,
}

impl InteractionTrigger {
    pub const ALL: [InteractionTrigger; 3] = [
        InteractionTrigger::Tap,
        InteractionTrigger::Hover,
        InteractionTrigger::LongPress,
    ];

    /// The canonical `events` key, as `jian-core`'s `handler_key` names it.
    pub fn handler_key(self) -> &'static str {
        match self {
            InteractionTrigger::Tap => "onTap",
            InteractionTrigger::Hover => "onHoverEnter",
            InteractionTrigger::LongPress => "onLongPress",
        }
    }

    /// The authored actions for this trigger, if any.
    pub fn actions(self, handlers: &EventHandlers) -> Option<&ActionList> {
        match self {
            InteractionTrigger::Tap => handlers.on_tap.as_ref(),
            InteractionTrigger::Hover => handlers.on_hover_enter.as_ref(),
            InteractionTrigger::LongPress => handlers.on_long_press.as_ref(),
        }
    }

    fn slot(self, handlers: &mut EventHandlers) -> &mut Option<ActionList> {
        match self {
            InteractionTrigger::Tap => &mut handlers.on_tap,
            InteractionTrigger::Hover => &mut handlers.on_hover_enter,
            InteractionTrigger::LongPress => &mut handlers.on_long_press,
        }
    }
}

/// Which navigate verb an action uses. The verb is also the screen
/// transition Preview plays: `push` slides the next screen in (and Back
/// returns), `replace` cross-fades without growing the route stack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NavigateVerb {
    Push,
    Replace,
}

impl NavigateVerb {
    pub fn as_str(self) -> &'static str {
        match self {
            NavigateVerb::Push => "push",
            NavigateVerb::Replace => "replace",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "push" => Some(NavigateVerb::Push),
            "replace" => Some(NavigateVerb::Replace),
            _ => None,
        }
    }
}

/// What the shared Interactions / Motion popover is showing. One
/// popover serves every row so the hosts' existing hit, hover and
/// dismiss routing covers all of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InteractionMenuTarget {
    /// The header "+": the triggers not authored yet, then "Animation".
    #[default]
    AddTrigger,
    /// One trigger's actions (Navigate to … / Back / Remove). Also used
    /// for a trigger just picked from `AddTrigger` that has no row yet.
    Trigger(InteractionTrigger),
    /// The animated-property list of the Motion row at this index.
    MotionProperty(usize),
}

/// The action an Interactions edit writes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InteractionActionEdit {
    /// Navigate to a `/`-rooted screen path.
    Navigate { path: String },
    /// `pop`: back to the previous screen.
    Back,
}

/// A navigate action body: the JSON string VALUE `"\"/path\""`, the
/// Tier-1 expression source of a string literal — the same encoding
/// `de-orchestrator::wire_screen_navigation` writes.
pub fn navigate_action(verb: NavigateVerb, path: &str) -> Action {
    let body = serde_json::to_string(path).unwrap_or_default();
    let mut map = std::collections::BTreeMap::new();
    map.insert(verb.as_str().to_string(), serde_json::Value::String(body));
    Action(map)
}

/// The back (`pop`) action.
pub fn pop_action() -> Action {
    let mut map = std::collections::BTreeMap::new();
    map.insert("pop".to_string(), serde_json::Value::Null);
    Action(map)
}

/// The verb of a trigger's single navigate action, if that is what it holds.
pub fn single_navigate_verb(actions: Option<&ActionList>) -> Option<NavigateVerb> {
    match actions.map(Vec::as_slice) {
        Some([only]) => only
            .single()
            .and_then(|(key, _)| NavigateVerb::from_key(key)),
        _ => None,
    }
}

/// The patch JSON writing `handlers` as the node's whole `events` block,
/// or removing the block when nothing is left in it. `None` when the
/// block does not serialize: the edit is refused rather than writing
/// `null` over every handler.
pub fn events_patch_json(handlers: &EventHandlers) -> Option<String> {
    if *handlers == EventHandlers::default() {
        return Some(r#"{"events":null}"#.to_string());
    }
    let value = serde_json::to_value(handlers).ok()?;
    Some(serde_json::json!({ "events": value }).to_string())
}

impl EditorState {
    /// Set `trigger`'s actions on the anchor node to the single `edit`.
    /// A navigate keeps the verb (transition) the trigger already had
    /// and defaults to `replace`, the verb the generation pipeline wires.
    pub fn set_selected_interaction_action(
        &mut self,
        trigger: InteractionTrigger,
        edit: &InteractionActionEdit,
    ) -> bool {
        self.edit_selected_events(|handlers| {
            let action = match edit {
                InteractionActionEdit::Navigate { path } => {
                    let verb = single_navigate_verb(trigger.actions(handlers))
                        .unwrap_or(NavigateVerb::Replace);
                    navigate_action(verb, path)
                }
                InteractionActionEdit::Back => pop_action(),
            };
            *trigger.slot(handlers) = Some(vec![action]);
            true
        })
    }

    /// Switch `trigger`'s single navigate action to `verb`, keeping its
    /// target. No-op (false) when the trigger holds anything else.
    pub fn set_selected_interaction_verb(
        &mut self,
        trigger: InteractionTrigger,
        verb: NavigateVerb,
    ) -> bool {
        self.edit_selected_events(|handlers| {
            let slot = trigger.slot(handlers);
            let Some([only]) = slot.as_deref() else {
                return false;
            };
            let Some((key, body)) = only.single() else {
                return false;
            };
            if NavigateVerb::from_key(key).is_none() || key == verb.as_str() {
                return false;
            }
            let mut map = std::collections::BTreeMap::new();
            map.insert(verb.as_str().to_string(), body.clone());
            *slot = Some(vec![Action(map)]);
            true
        })
    }

    /// Remove every action of `trigger`; the `events` block goes away
    /// when nothing else is left in it.
    pub fn remove_selected_interaction(&mut self, trigger: InteractionTrigger) -> bool {
        self.edit_selected_events(|handlers| trigger.slot(handlers).take().is_some())
    }

    /// Run `edit` on a copy of the anchor's `events` block and write the
    /// result when `edit` reports a change.
    fn edit_selected_events(&mut self, edit: impl FnOnce(&mut EventHandlers) -> bool) -> bool {
        let node_id = self.selection.anchor.clone();
        if !node_id.is_real() || !self.is_editable(&node_id) {
            return false;
        }
        let Some(node) = self.selected_node() else {
            return false;
        };
        let mut handlers = node.events().cloned().unwrap_or_default();
        if !edit(&mut handlers) {
            return false;
        }
        let Some(patch_json) = events_patch_json(&handlers) else {
            return false;
        };
        self.apply(EditorCommand::PatchNodeData {
            node_id,
            patch_json,
            page_id: None,
        })
    }
}

#[cfg(test)]
#[path = "interaction_edit_tests.rs"]
mod tests;
