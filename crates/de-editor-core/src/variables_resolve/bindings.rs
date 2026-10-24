//! Typed-field variable bindings (`PenNodeBase::variable_bindings`).
//!
//! A typed field such as `fontSize: f64` cannot hold a `$ref`, so a node
//! lists the fields that follow a variable by JSON path, with the value the
//! field held when it was bound. The field keeps that value for every
//! consumer; here the renderer swaps in the variable's value for the
//! node's theme. A field that no longer holds the bound value was edited
//! (in the panel, over MCP, or by an instance override) and keeps its own
//! value.

use serde_json::Value;

use jian_ops_schema::node::PenNode;
use jian_ops_schema::variable::VariableScalar;

use super::{resolve_variable_ref, Theme, Vars};
use crate::pen_node_ext::PenNodeExt;

/// Whether `node` itself carries any binding.
pub(super) fn has_bindings(node: &PenNode) -> bool {
    node.base()
        .variable_bindings
        .as_ref()
        .is_some_and(|bindings| !bindings.is_empty())
}

/// Write each bound field's themed value into `node`. Returns whether a
/// field changed.
pub(super) fn apply(node: &mut PenNode, vars: Option<&Vars>, theme: &Theme) -> bool {
    apply_matching(node, vars, theme, None)
}

/// [`apply`], limited to the bindings of variable `only` when given.
fn apply_matching(
    node: &mut PenNode,
    vars: Option<&Vars>,
    theme: &Theme,
    only: Option<&str>,
) -> bool {
    let Some(bindings) = node
        .base()
        .variable_bindings
        .as_ref()
        .filter(|bindings| !bindings.is_empty())
    else {
        return false;
    };
    let updates: Vec<(String, Value, Value)> = bindings
        .iter()
        .filter(|(_, binding)| only.is_none_or(|name| binding.variable == name))
        .filter_map(|(path, binding)| {
            let scalar = resolve_variable_ref(&format!("${}", binding.variable), vars, theme)?;
            Some((pointer(path), scalar_json(scalar)?, binding.value.clone()))
        })
        .collect();
    if updates.is_empty() {
        return false;
    }
    // Only the node's own fields are bound: leave the subtree out of the
    // round trip so a bound screen root does not serialize the screen.
    let children = node.children_mut().map(std::mem::take);
    let mut changed = false;
    if let Ok(mut json) = serde_json::to_value(&*node) {
        for (path, value, bound) in updates {
            let Some(slot) = json.pointer_mut(&path) else {
                continue;
            };
            if !same(slot, &bound) || same(slot, &value) {
                continue;
            }
            *slot = value;
            changed = true;
        }
        if changed {
            match serde_json::from_value::<PenNode>(json) {
                Ok(resolved) => *node = resolved,
                Err(_) => changed = false,
            }
        }
    }
    if let (Some(children), Some(slot)) = (children, node.children_mut()) {
        *slot = children;
    }
    changed
}

/// Follow a variable rename (`new`) or delete (`None`). A delete writes the
/// variable's last value into each field bound to it, then drops the
/// binding, so the field keeps showing what it showed.
pub(super) fn rename(
    node: &mut PenNode,
    token: &str,
    new: Option<&str>,
    vars: Option<&Vars>,
    theme: &Theme,
) {
    let Some(old) = token.strip_prefix('$') else {
        return;
    };
    let bound = node
        .base()
        .variable_bindings
        .as_ref()
        .is_some_and(|bindings| bindings.values().any(|b| b.variable == old));
    if !bound {
        return;
    }
    if new.is_none() {
        apply_matching(node, vars, theme, Some(old));
    }
    let Some(bindings) = node.base_mut().variable_bindings.as_mut() else {
        return;
    };
    match new {
        Some(new) => {
            for binding in bindings.values_mut().filter(|b| b.variable == old) {
                binding.variable = new.to_owned();
            }
        }
        None => bindings.retain(|_, binding| binding.variable != old),
    }
    if bindings.is_empty() {
        node.base_mut().variable_bindings = None;
    }
}

/// A binding path (`padding/1`) as a JSON pointer (`/padding/1`).
fn pointer(path: &str) -> String {
    if path.starts_with('/') {
        path.to_owned()
    } else {
        format!("/{path}")
    }
}

fn scalar_json(scalar: VariableScalar) -> Option<Value> {
    Some(match scalar {
        VariableScalar::Bool(value) => Value::Bool(value),
        VariableScalar::Num(value) => serde_json::Number::from_f64(value).map(Value::Number)?,
        VariableScalar::Str(value) => Value::String(value),
    })
}

/// Equal values, numbers compared as floats (`16` and `16.0` are the same
/// field value after a round trip through `f32` / `f64` fields).
fn same(a: &Value, b: &Value) -> bool {
    match (a.as_f64(), b.as_f64()) {
        (Some(a), Some(b)) => (a - b).abs() <= 1e-4 * a.abs().max(b.abs()).max(1.0),
        _ => a == b,
    }
}

#[cfg(test)]
#[path = "bindings_tests.rs"]
mod tests;
