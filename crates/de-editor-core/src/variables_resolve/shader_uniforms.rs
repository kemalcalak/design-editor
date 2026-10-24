//! `$variable` shader uniforms (#165).
//!
//! A shader fill or effect may bind a uniform to a design variable by
//! storing the reference as the uniform's string value (`"u_tint":
//! "$accent"`, `"u_speed": "$motion"`). The canvas resolver turns it into
//! a concrete value for the active theme before the scene is built, so a
//! theme switch or a variable edit repaints the shader with the new value:
//! a number becomes a float, a boolean 0 / 1, a string (a colour) the hex.
//! A reference that does not resolve is dropped, leaving the uniform
//! unbound (zero), the same as a missing value. The reference travels in
//! `ShaderUniformValue::Color`, the schema's only string variant, whatever
//! the variable's kind.

use std::collections::BTreeMap;

use jian_ops_schema::style::ShaderUniformValue;
use jian_ops_schema::variable::VariableScalar;

use super::{is_variable_ref, resolve_variable_ref, Theme, Vars};

type Uniforms = BTreeMap<String, ShaderUniformValue>;

/// Whether any uniform names a variable.
pub(super) fn has_tokens(uniforms: Option<&Uniforms>) -> bool {
    uniforms.is_some_and(|map| map.values().any(is_reference))
}

fn is_reference(value: &ShaderUniformValue) -> bool {
    matches!(value, ShaderUniformValue::Color(text) if is_variable_ref(text))
}

/// Resolve every variable-bound uniform in place. Returns whether anything
/// changed.
pub(super) fn resolve(uniforms: Option<&mut Uniforms>, vars: Option<&Vars>, theme: &Theme) -> bool {
    let Some(uniforms) = uniforms else {
        return false;
    };
    let names: Vec<String> = uniforms
        .iter()
        .filter(|(_, value)| is_reference(value))
        .map(|(name, _)| name.clone())
        .collect();
    for name in &names {
        let Some(ShaderUniformValue::Color(reference)) = uniforms.get(name) else {
            continue;
        };
        match resolve_variable_ref(reference, vars, theme).and_then(scalar_value) {
            Some(value) => {
                uniforms.insert(name.clone(), value);
            }
            None => {
                uniforms.remove(name);
            }
        }
    }
    !names.is_empty()
}

/// The uniform value a resolved variable binds as.
fn scalar_value(scalar: VariableScalar) -> Option<ShaderUniformValue> {
    match scalar {
        VariableScalar::Num(n) if n.is_finite() => Some(ShaderUniformValue::Float(n as f32)),
        VariableScalar::Num(_) => None,
        VariableScalar::Bool(b) => Some(ShaderUniformValue::Float(f32::from(u8::from(b)))),
        VariableScalar::Str(text) if !is_variable_ref(&text) => {
            Some(ShaderUniformValue::Color(text))
        }
        VariableScalar::Str(_) => None,
    }
}

/// Rename (`new`) or inline (`new == None`, the variable is deleted) the
/// references to `token` (`$name`).
pub(super) fn replace(
    uniforms: Option<&mut Uniforms>,
    token: &str,
    new: Option<&str>,
    vars: Option<&Vars>,
    theme: &Theme,
) {
    let Some(uniforms) = uniforms else {
        return;
    };
    for value in uniforms.values_mut() {
        if !matches!(value, ShaderUniformValue::Color(text) if text == token) {
            continue;
        }
        match new {
            Some(new) => *value = ShaderUniformValue::Color(format!("${new}")),
            None => {
                if let Some(resolved) =
                    resolve_variable_ref(token, vars, theme).and_then(scalar_value)
                {
                    *value = resolved;
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "shader_uniforms_tests.rs"]
mod tests;
