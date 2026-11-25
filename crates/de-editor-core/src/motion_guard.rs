//! Write-time motion guard (#351).
//!
//! Keyframe `values` are a free map, so a write could store a property the
//! loader does not know (`scale`) and the document would only fail on the
//! next open. Every command that brings node JSON into the document passes
//! through [`prepare_command`] before it is applied: the `scale` alias is
//! expanded to `scaleX` + `scaleY`, then each animation is validated with
//! the loader's own rules. MCP tools call [`prepare_motion_json`] /
//! [`prepare_nodes_motion`] earlier to return a readable error.

use crate::instance_edit::InstanceEdit;
use crate::pen_node_ext::PenNodeExt;
use crate::EditorCommand;
use jian_ops_schema::motion::{
    expand_keyframe_aliases, validate_keyframes, validate_nodes, NodeAnimation,
};
use jian_ops_schema::node::PenNode;
use serde::Deserialize;
use serde_json::Value;

/// An animation a write would store that the loader would not accept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MotionWriteError {
    /// JSON path inside the written payload (`$` is its root).
    pub path: String,
    pub reason: String,
}

impl std::fmt::Display for MotionWriteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid animation at {}: {}", self.path, self.reason)
    }
}

impl std::error::Error for MotionWriteError {}

impl From<(String, String)> for MotionWriteError {
    fn from((path, reason): (String, String)) -> Self {
        Self { path, reason }
    }
}

/// Expand the `scale` alias in `nodes` and their descendants, then
/// validate every animation.
pub fn prepare_nodes_motion(nodes: &mut [PenNode]) -> Result<(), MotionWriteError> {
    for node in nodes.iter_mut() {
        expand_node_aliases(node);
    }
    validate_nodes(nodes, "$").map_err(MotionWriteError::from)
}

fn expand_node_aliases(node: &mut PenNode) {
    if let Some(animations) = node.animations_mut() {
        for animation in animations {
            expand_keyframe_aliases(&mut animation.keyframes);
        }
    }
    if let Some(children) = node.children_mut() {
        for child in children {
            expand_node_aliases(child);
        }
    }
}

/// Expand + validate `animations` in a node patch or a node JSON tree
/// (its `children` included). Returns whether the JSON was rewritten.
pub fn prepare_motion_json(value: &mut Value) -> Result<bool, MotionWriteError> {
    prepare_json_at(value, &mut Vec::new())
}

/// `trail` holds the child indices from the root; the error path is
/// built from it only when there is an error.
fn prepare_json_at(value: &mut Value, trail: &mut Vec<usize>) -> Result<bool, MotionWriteError> {
    let Some(object) = value.as_object_mut() else {
        return Ok(false);
    };
    let mut changed = false;
    if let Some(animations) = object.get_mut("animations").filter(|v| !v.is_null()) {
        let animations_path = || format!("{}.animations", json_node_path(trail));
        let mut list =
            Vec::<NodeAnimation>::deserialize(&*animations).map_err(|error| MotionWriteError {
                path: animations_path(),
                reason: error.to_string(),
            })?;
        let mut expanded = false;
        for (index, animation) in list.iter_mut().enumerate() {
            expanded |= expand_keyframe_aliases(&mut animation.keyframes);
            let path = format!("{}[{index}].keyframes", animations_path());
            validate_keyframes(&animation.keyframes, &path)?;
        }
        if expanded {
            *animations = serde_json::to_value(&list).map_err(|error| MotionWriteError {
                path: animations_path(),
                reason: error.to_string(),
            })?;
            changed = true;
        }
    }
    if let Some(Value::Array(children)) = object.get_mut("children") {
        for (index, child) in children.iter_mut().enumerate() {
            trail.push(index);
            changed |= prepare_json_at(child, trail)?;
            trail.pop();
        }
    }
    Ok(changed)
}

fn json_node_path(trail: &[usize]) -> String {
    let mut path = String::from("$");
    for index in trail {
        path.push_str(&format!(".children[{index}]"));
    }
    path
}

/// [`prepare_motion_json`] over a JSON string, rewriting it in place when
/// the alias expanded. Text that is not a JSON object is left for the
/// command's own handler to reject.
fn prepare_json_string(json: &mut String) -> Result<(), MotionWriteError> {
    if !json.contains("animations") {
        return Ok(());
    }
    let Ok(mut value) = serde_json::from_str::<Value>(json) else {
        return Ok(());
    };
    if prepare_motion_json(&mut value)? {
        *json = value.to_string();
    }
    Ok(())
}

/// Expand + validate the node payload of `cmd` in place; an error means
/// the command would store an animation the loader does not accept. A
/// `Batch` is covered because each of its commands is applied on its own.
///
/// Exhaustive on purpose (like `command_batch::batchable`): a new
/// `EditorCommand` variant fails to compile here until someone says
/// whether it carries node JSON.
pub(crate) fn prepare_command(cmd: &mut EditorCommand) -> Result<(), MotionWriteError> {
    use EditorCommand as C;
    match cmd {
        C::PatchNodeData { patch_json, .. } => prepare_json_string(patch_json),
        C::CopyNode { overrides_json, .. } | C::InstantiateKitComponent { overrides_json, .. } => {
            overrides_json.as_mut().map_or(Ok(()), prepare_json_string)
        }
        C::ReplaceSubtree { node, .. } => prepare_nodes_motion(std::slice::from_mut(node)),
        C::UpsertComponent { root, .. } | C::UpsertScreen { root, .. } => {
            prepare_nodes_motion(std::slice::from_mut(root))
        }
        C::InsertSubtree { nodes, .. }
        | C::InsertAuthoredSubtree { nodes, .. }
        | C::InsertAuthoredSubtreePreservingRoots { nodes, .. } => prepare_nodes_motion(nodes),
        C::AddPage { children, .. } => children
            .as_mut()
            .map_or(Ok(()), |n| prepare_nodes_motion(n)),
        C::EditInstanceDescendant { edit, .. } => match edit {
            InstanceEdit::Patch { patch_json } => prepare_json_string(patch_json),
            InstanceEdit::Replace { node } => prepare_nodes_motion(std::slice::from_mut(node)),
            InstanceEdit::InsertChildren { nodes, .. } => prepare_nodes_motion(nodes),
            InstanceEdit::Reset => Ok(()),
        },
        // No node JSON from the caller: these build nodes themselves
        // (shipped templates, SVG import, booleans, kit masters) or only
        // touch fields animations do not live in.
        C::SetVariableColor { .. }
        | C::SetActiveAxisValue { .. }
        | C::CycleActiveAxisValue { .. }
        | C::InsertNode { .. }
        | C::UpdateNode { .. }
        | C::DeleteNode { .. }
        | C::MoveNode { .. }
        | C::ReplaceNode { .. }
        | C::BatchInsert { .. }
        | C::AdoptSceneTemplate { .. }
        | C::RefineDesign { .. }
        | C::SetVariableScalar { .. }
        | C::CreateVariable { .. }
        | C::DeleteVariable { .. }
        | C::RenameVariable { .. }
        | C::SetVariables { .. }
        | C::UpsertVariables { .. }
        | C::SetThemes { .. }
        | C::MergeThemePreset { .. }
        | C::MergeAppState { .. }
        | C::SetDesignMd { .. }
        | C::Library(_)
        | C::InstantiateComponent { .. }
        | C::CreateComponent { .. }
        | C::DeleteComponent { .. }
        | C::RenameComponent { .. }
        | C::SetActivePage { .. }
        | C::RenamePage { .. }
        | C::DeletePage { .. }
        | C::DuplicatePage { .. }
        | C::ReorderPage { .. }
        | C::ClearSelection
        | C::SetSelection { .. }
        | C::SetSelectionSet { .. }
        | C::ToggleNodeSelection { .. }
        | C::SetViewport { .. }
        | C::SetNodeFlag { .. }
        | C::SetNodeFlip { .. }
        | C::SetEllipseArc { .. }
        | C::AddNodeEffect { .. }
        | C::RemoveNodeEffect { .. }
        | C::SetEffectParam { .. }
        | C::SetEffectColor { .. }
        | C::SetShaderUniform { .. }
        | C::SetShaderSource { .. }
        | C::SetActiveTool { .. }
        | C::DuplicateSelected { .. }
        | C::DeleteSelected
        | C::NudgeSelected { .. }
        | C::GroupSelected
        | C::UngroupSelected
        | C::ReorderSelected { .. }
        | C::SetNodeRotation { .. }
        | C::SetNodeText { .. }
        | C::SetNodeCornerRadius { .. }
        | C::SetNodeFontSize { .. }
        | C::SetNodeFontWeight { .. }
        | C::SetNodeStrokeHex { .. }
        | C::SetNodeStrokeWidth { .. }
        | C::SetNodeStrokeSideWidth { .. }
        | C::SetNodeStrokeStyle { .. }
        | C::AlignSelected { .. }
        | C::CommitBooleanResult { .. }
        | C::SetNodeFillHex { .. }
        | C::SetNodeName { .. }
        | C::SetImageVideoSrc { .. }
        | C::SetImageVideoPlayback { .. }
        | C::AddImageVideo { .. }
        | C::RemoveImageVideo { .. }
        | C::ImportSvg { .. }
        | C::SetNodeLayoutProp { .. }
        | C::ReplaceFontFamily { .. }
        | C::ReplaceAllMatchingProperties { .. }
        | C::PromoteLegacyWidgets
        | C::RepairDuplicateIds
        | C::CopySelected
        | C::CutSelected
        | C::PasteClipboard { .. }
        | C::Undo
        | C::Redo
        | C::Batch { .. } => Ok(()),
    }
}

#[cfg(test)]
#[path = "motion_guard_tests.rs"]
mod tests;
