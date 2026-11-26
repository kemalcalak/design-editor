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

use de_editor_core::{EditorState, InstanceDescendant, InstanceEdit, PenNodeExt};
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

/// Instances in a fresh insert may name their master, and the master's
/// children in `descendants`, by a binding (`ref: master` in a script) or by
/// an id authored earlier in this program; both are final ids only after
/// `remap`. Without this the instance keeps a target nothing carries and
/// renders nothing (#366).
pub(crate) fn retarget_program_instances(
    nodes: &mut [PenNode],
    map: &[(String, String)],
    ctx: &ProgramCtx,
) {
    if !nodes.iter().any(subtree_has_instance) {
        return;
    }
    let fresh = |id: &str| {
        ctx.bindings
            .get(id)
            .or_else(|| map.iter().find(|(old, _)| old == id).map(|(_, new)| new))
            .or_else(|| ctx.alias.get(id))
            // A binding made by `R()` / an instance-slot insert is an
            // `instanceId/childId` path, never a master or a master child.
            .filter(|fresh| !fresh.contains('/'))
            .cloned()
    };
    // A target that is a node id in the document always means that node:
    // binding names and authored ids never hijack an existing master.
    let existing = ctx.sim.collect_node_ids();
    let target = |id: &str| {
        if existing.contains(&de_editor_core::NodeId::new(id)) {
            None
        } else {
            fresh(id)
        }
    };
    let ids = super::import_ids::Lookups {
        target: &target,
        path: &fresh,
    };
    super::import_ids::retarget_instances(nodes, &ids);
}

/// The instances an `I()` line inserts must name their masters'
/// properties with values of the right type (#367). Masters inserted
/// earlier in the program count: the sim already holds them.
pub(crate) fn check_program_props(nodes: &[PenNode], ctx: &ProgramCtx) -> Result<()> {
    let context = de_editor_core::PropsContext {
        doc: &ctx.sim.doc,
        components: &ctx.sim.components,
    };
    super::component_property_tools::check_inserted_props(context, nodes)
        .map_err(|error| ProgramError::InvalidValue(error.to_string()))
}

/// Instances an `I()` line inserts that name a set or ask for a variant
/// point at the master they show (#368); unknown axes or values fail the
/// line, a combination without a variant is a warning.
pub(crate) fn normalize_program_variants(nodes: &mut [PenNode], ctx: &mut ProgramCtx) -> Result<()> {
    let warnings =
        super::component_set_tools::normalize_inserted_variants(&ctx.sim.doc, nodes)
            .map_err(|error| ProgramError::InvalidValue(error.to_string()))?;
    ctx.warnings.extend(warnings);
    Ok(())
}

/// `U(instance, {variant:{…}})`: switch the instance's variant as its own
/// command (props and matching overrides stay). True when the patch held
/// nothing else.
pub(crate) fn emit_variant_update(
    node_id: &str,
    value: &mut serde_json::Value,
    ctx: &mut ProgramCtx,
) -> Result<bool> {
    let update = super::component_set_tools::take_variant_update(&ctx.sim.doc, node_id, value)
        .map_err(|error| ProgramError::InvalidValue(error.to_string()))?;
    let Some((command, warnings)) = update else {
        return Ok(false);
    };
    ctx.emit(
        de_editor_core::EditorCommand::ComponentSet(Box::new(command)),
        &format!("Update failed for: {node_id}"),
    )?;
    ctx.warnings.extend(warnings);
    Ok(value.as_object().is_none_or(serde_json::Map::is_empty))
}

/// `U(instance, {props:{…}})`: the values merge into the instance's own by
/// name (`null` clears one) as their own command. True when the patch held
/// nothing else.
pub(crate) fn emit_props_update(
    node_id: &str,
    value: &mut serde_json::Value,
    ctx: &mut ProgramCtx,
) -> Result<bool> {
    let context = de_editor_core::PropsContext {
        doc: &ctx.sim.doc,
        components: &ctx.sim.components,
    };
    let command = super::component_property_tools::take_props_update(Some(context), node_id, value)
        .map_err(|error| ProgramError::InvalidValue(error.to_string()))?;
    let Some(command) = command else {
        return Ok(false);
    };
    ctx.emit(
        de_editor_core::EditorCommand::ComponentProperty(Box::new(command)),
        &format!("Update failed for: {node_id}"),
    )?;
    Ok(value.as_object().is_none_or(serde_json::Map::is_empty))
}

/// Whether `node`'s subtree holds what retargeting rewrites: an instance,
/// or a master whose properties bind its own children (#367).
pub(crate) fn subtree_has_instance(node: &PenNode) -> bool {
    matches!(node, PenNode::Ref(_))
        || matches!(node, PenNode::Frame(frame) if frame.component_properties.is_some() || frame.component_set.is_some())
        || node
            .children()
            .is_some_and(|children| children.iter().any(subtree_has_instance))
}

#[cfg(test)]
#[path = "batch_program_instance_tests.rs"]
mod tests;
