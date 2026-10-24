//! `batch_design` DSL lines that write inside a component instance (#166):
//! `U` / `R` / `I` on an `instanceId/childId` path (any nesting depth).
//! The node exists only once the instance expands, so each line becomes one
//! `EditorCommand::EditInstanceDescendant` on the authored instance.
//!
//! The pen.dev slot pattern works as written:
//!
//! ```text
//! card=I(page,{type:"ref",ref:"cardMaster"})
//! content=R(card+"/contentSlot",{type:"frame",layout:"vertical"})
//! item=I(content,{type:"text",content:"Item 1"})
//! ```
//!
//! Nodes created inside an instance get fresh ids, and their binding is
//! their path (`card/<id>`), so later lines can update or fill them.

use de_editor_core::{EditorState, InstanceDescendant, InstanceEdit};
use jian_ops_schema::node::PenNode;

use super::batch_design::find_top_level_char;
use super::batch_program::{ProgramCtx, Result};
use super::batch_program_error::ProgramError;
use super::batch_program_resolve::{
    find_node_by_path, instance_child_path, instance_edit_command, strip_outer_quotes,
};

/// `binding=R(instance/child, data)` — swap the descendant for a new node.
pub(crate) fn execute_instance_replace(
    binding: &str,
    node: PenNode,
    hit: &InstanceDescendant,
    path: &str,
    ctx: &mut ProgramCtx,
) -> Result<()> {
    let mut nodes = vec![node];
    let map = ctx.remap(&mut nodes)?;
    let new_id = map
        .first()
        .map(|(_, new)| new.clone())
        .ok_or(ProgramError::ProducedNoNode("Replace"))?;
    let node = nodes.pop().ok_or(ProgramError::ProducedNoNode("Replace"))?;
    let edit = InstanceEdit::Replace {
        node: Box::new(node),
    };
    let cmd = instance_edit_command(hit, edit, ctx.page_id.clone());
    ctx.emit(cmd, &format!("Replace failed for: {path}"))?;
    ctx.bind(binding, &instance_child_path(hit, &new_id));
    Ok(())
}

/// `binding=I(instance/child, data)` — add a child to a container
/// descendant. The first insert into a master node replaces its default
/// children (slot content); later inserts add to the instance's own.
pub(crate) fn execute_instance_insert(
    binding: &str,
    node: PenNode,
    hit: &InstanceDescendant,
    ctx: &mut ProgramCtx,
) -> Result<()> {
    let mut nodes = vec![node];
    let map = ctx.remap(&mut nodes)?;
    let new_id = map
        .first()
        .map(|(_, new)| new.clone())
        .ok_or(ProgramError::ProducedNoNode("Insert"))?;
    let edit = InstanceEdit::InsertChildren { nodes, index: None };
    let cmd = instance_edit_command(hit, edit, ctx.page_id.clone());
    let path = format!("{}/{}", hit.ref_id.as_str(), hit.target.path());
    ctx.emit(
        cmd,
        &format!("Insert parent not found or not a container: {path}"),
    )?;
    ctx.bind(binding, &instance_child_path(hit, &new_id));
    Ok(())
}

/// True when a one-line `operations` payload writes into an instance
/// (`U` / `R` / `I` on an `instance/child` path). The flat single-op
/// parsers hold no document, so such a line runs through the program
/// simulator, which resolves the path against the snapshot.
pub(crate) fn single_line_targets_instance(state: &EditorState, operations: &str) -> bool {
    let line = operations.trim().trim_end_matches([';', ',']).trim();
    let call = find_top_level_char(line, '=').map_or(line, |eq| line[eq + 1..].trim());
    let Some(args) = ["U(", "R(", "I("]
        .iter()
        .find_map(|prefix| call.strip_prefix(prefix))
        .and_then(|rest| rest.strip_suffix(')'))
    else {
        return false;
    };
    let first = find_top_level_char(args, ',').map_or(args, |comma| &args[..comma]);
    let path = strip_outer_quotes(first.trim());
    let no_alias = std::collections::BTreeMap::new();
    if !path.contains('/') || find_node_by_path(state.active_children(), &path, &no_alias).is_some()
    {
        return false;
    }
    // Any instance on the path sends the line to the simulator, so a path
    // that misses inside the instance fails with a named error.
    let segments: Vec<&str> = path.split('/').collect();
    segments[..segments.len() - 1].iter().any(|segment| {
        de_editor_core::NodeId::new_opt(*segment)
            .and_then(|id| de_editor_core::walkers::find_node(state.active_children(), &id))
            .is_some_and(|node| matches!(node, PenNode::Ref(_)))
    })
}

#[cfg(test)]
#[path = "batch_program_instance_tests.rs"]
mod tests;
