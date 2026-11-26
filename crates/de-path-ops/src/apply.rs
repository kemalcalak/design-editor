//! Run a [`PathOperation`] on the editor selection: the one entry point
//! the desktop keyboard, menus, layer menu and property panel share.

use crate::plan::{plan_flatten, plan_offset, plan_outline_stroke};
use de_editor_core::path_ops::{
    PathCommand, PathCommitKind, PathOperation, DEFAULT_SIMPLIFY_TOLERANCE,
};
use de_editor_core::{EditorState, NodeId};
use jian_scene::layout_scene::LayoutScene;

/// Apply `op` to the selection of `state`, reading geometry from `scene`
/// (the layout of the current document). New ids come from `next_id`.
/// True when the document changed, or when the op was refused with a
/// notice (a toast says why): either way the host repaints.
pub fn apply_path_operation(
    state: &mut EditorState,
    scene: &LayoutScene,
    op: PathOperation,
    next_id: &mut u64,
    now_ms: u64,
) -> bool {
    let ids = state.selection_in_paint_order();
    if ids.is_empty() {
        return false;
    }
    let scene_ids = |ids: &[NodeId]| {
        ids.iter()
            .map(|id| id.as_str().to_string())
            .collect::<Vec<_>>()
    };
    match op {
        PathOperation::Boolean(op) => {
            if let [only] = ids.as_slice() {
                return state.boolean_op_of(only).is_some()
                    && state.apply_canvas_path_command(PathCommand::SetBooleanOp {
                        node_id: only.clone(),
                        op,
                    });
            }
            state.create_canvas_boolean_group(ids, op, next_id)
        }
        PathOperation::Flatten => {
            if state.refuse_boolean_on_containers(&ids, now_ms) {
                return true;
            }
            plan_flatten(scene, &scene_ids(&ids)).is_some_and(|plan| {
                state.commit_canvas_path(plan.into_canvas(PathCommitKind::Flatten), next_id)
            })
        }
        PathOperation::OutlineStroke => each(&ids, |id| {
            plan_outline_stroke(scene, id.as_str()).is_some_and(|plan| {
                state.commit_canvas_path(plan.into_canvas(PathCommitKind::OutlineStroke), next_id)
            })
        }),
        PathOperation::Offset { delta, join } => each(&ids, |id| {
            plan_offset(scene, id.as_str(), delta, join).is_some_and(|plan| {
                state.commit_canvas_path(plan.into_canvas(PathCommitKind::Offset), next_id)
            })
        }),
        PathOperation::Simplify => each(&ids, |id| {
            state.apply_canvas_path_command(PathCommand::Simplify {
                node_id: id.clone(),
                tolerance: DEFAULT_SIMPLIFY_TOLERANCE,
            })
        }),
        PathOperation::Reverse => each(&ids, |id| {
            state.apply_canvas_path_command(PathCommand::Reverse {
                node_id: id.clone(),
            })
        }),
    }
}

/// Run `f` on every id; true when any of them applied.
fn each(ids: &[NodeId], mut f: impl FnMut(&NodeId) -> bool) -> bool {
    ids.iter().fold(false, |any, id| f(id) || any)
}

#[cfg(test)]
#[path = "apply_tests.rs"]
mod tests;
