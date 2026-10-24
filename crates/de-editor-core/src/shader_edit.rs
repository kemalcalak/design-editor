//! Writing a shader paint's uniforms and source (#164).
//!
//! A shader paint is a shader fill (`PenFill::Shader`) or a shader effect
//! (`PenEffect::Shader`); [`ShaderTarget`] names one on a node. The panel
//! and MCP write through [`EditorCommand::SetShaderUniform`] and
//! [`EditorCommand::SetShaderSource`](crate::EditorCommand), which land here.
//!
//! A uniform write is checked against what the program declares
//! ([`crate::shader_uniform_specs`]): an unknown name or a value that does
//! not fit the declared type is refused, so the panel and MCP can never
//! store a binding the backends would skip. A source write replaces the
//! program (a fill's loader preset is dropped, since a preset wins over the
//! source) and seeds every newly declared uniform with its `@default`; it is
//! not compiled here (core has no SkSL compiler): the hosts compile it and
//! the panel shows the error.

use std::collections::BTreeMap;

use jian_ops_schema::node::PenNode;
use jian_ops_schema::style::{PenEffect, PenFill, ShaderUniformValue};

use crate::fills::{node_effects, node_fills, node_fills_mut};
use crate::node_id::NodeId;
use crate::shader_uniform_specs::{stored_value_specs, uniform_specs, ShaderUniformSpec};
use crate::state::EditorState;
use crate::walkers::find_node_mut;

/// Which shader paint of a node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShaderTarget {
    /// The shader fill at this index of the node's fill list.
    Fill(usize),
    /// The shader effect at this index of the node's effect list.
    Effect(usize),
}

/// A read-only view of one shader paint.
#[derive(Debug, Clone, Copy)]
pub struct ShaderProgram<'a> {
    /// The SkSL source; `None` for a fill expanded from a loader preset.
    pub sksl: Option<&'a str>,
    pub uniforms: Option<&'a BTreeMap<String, ShaderUniformValue>>,
}

impl ShaderProgram<'_> {
    /// The user uniforms the panel shows, in declaration order.
    pub fn specs(&self) -> Vec<ShaderUniformSpec> {
        match self.sksl {
            Some(sksl) => uniform_specs(sksl, self.uniforms),
            None => self.uniforms.map(stored_value_specs).unwrap_or_default(),
        }
    }
}

/// The shader paint `target` names on `node`, if it is one.
pub fn shader_program(node: &PenNode, target: ShaderTarget) -> Option<ShaderProgram<'_>> {
    match target {
        ShaderTarget::Fill(index) => match node_fills(node)?.get(index)? {
            PenFill::Shader(body) => Some(ShaderProgram {
                sksl: body
                    .sksl
                    .as_deref()
                    .filter(|_| body.preset.as_deref() != Some(crate::shader_presets::TURBULENCE)),
                uniforms: body.uniforms.as_ref(),
            }),
            _ => None,
        },
        ShaderTarget::Effect(index) => match node_effects(node).get(index)? {
            PenEffect::Shader(body) => Some(ShaderProgram {
                sksl: Some(body.sksl.as_str()),
                uniforms: body.uniforms.as_ref(),
            }),
            _ => None,
        },
    }
}

/// Mutable parts of one shader paint.
struct ShaderProgramMut<'a> {
    preset: Option<&'a mut Option<String>>,
    sksl: SourceSlot<'a>,
    uniforms: &'a mut Option<BTreeMap<String, ShaderUniformValue>>,
}

enum SourceSlot<'a> {
    Fill(&'a mut Option<String>),
    Effect(&'a mut String),
}

fn shader_program_mut(node: &mut PenNode, target: ShaderTarget) -> Option<ShaderProgramMut<'_>> {
    match target {
        ShaderTarget::Fill(index) => match node_fills_mut(node)?.get_mut(index)? {
            PenFill::Shader(body) => Some(ShaderProgramMut {
                preset: Some(&mut body.preset),
                sksl: SourceSlot::Fill(&mut body.sksl),
                uniforms: &mut body.uniforms,
            }),
            _ => None,
        },
        ShaderTarget::Effect(index) => {
            match crate::command_node_attrs::node_effects_slot(node)?
                .as_mut()?
                .get_mut(index)?
            {
                PenEffect::Shader(body) => Some(ShaderProgramMut {
                    preset: None,
                    sksl: SourceSlot::Effect(&mut body.sksl),
                    uniforms: &mut body.uniforms,
                }),
                _ => None,
            }
        }
    }
}

/// Store `value` for the uniform `name`, coerced to its declared form.
/// `false` when `target` is not a shader paint, the program does not
/// declare `name`, the value does not fit, or it is already stored.
pub fn set_shader_uniform(
    node: &mut PenNode,
    target: ShaderTarget,
    name: &str,
    value: &ShaderUniformValue,
) -> bool {
    let Some(spec) = shader_program(node, target)
        .and_then(|program| program.specs().into_iter().find(|spec| spec.name == name))
    else {
        return false;
    };
    // A `$variable` binding is kept as written; the canvas resolver
    // checks it per theme (#165).
    let reference = matches!(
        value,
        ShaderUniformValue::Color(text) if crate::variables_resolve::is_variable_ref(text)
    ) && !matches!(spec.kind, crate::UniformKind::Vector(_));
    let Some(value) = (if reference {
        Some(value.clone())
    } else {
        spec.coerce(value)
    }) else {
        return false;
    };
    let Some(program) = shader_program_mut(node, target) else {
        return false;
    };
    let map = program.uniforms.get_or_insert_with(BTreeMap::new);
    if map.get(name) == Some(&value) {
        return false;
    }
    map.insert(name.to_owned(), value);
    true
}

/// Replace the program's source with `sksl`. A fill's preset is dropped
/// (a preset wins over the source) and every newly declared uniform with
/// no stored value takes its `@default`. `false` when `target` is not a
/// shader paint or the source is unchanged.
pub fn set_shader_source(node: &mut PenNode, target: ShaderTarget, sksl: &str) -> bool {
    let Some(program) = shader_program_mut(node, target) else {
        return false;
    };
    let has_preset = program
        .preset
        .as_ref()
        .is_some_and(|preset| preset.is_some());
    let unchanged = match &program.sksl {
        SourceSlot::Fill(slot) => slot.as_deref() == Some(sksl) && !has_preset,
        SourceSlot::Effect(slot) => slot.as_str() == sksl,
    };
    if unchanged {
        return false;
    }
    let is_fill = matches!(program.sksl, SourceSlot::Fill(_));
    match program.sksl {
        SourceSlot::Fill(slot) => *slot = Some(sksl.to_owned()),
        SourceSlot::Effect(slot) => *slot = sksl.to_owned(),
    }
    if let Some(preset) = program.preset {
        *preset = None;
    }
    // The layout scene writes the node size only into an existing `size`
    // entry (`de-pen-loader` `payload_shader_to_scene`), so a fill program
    // that reads `size` needs one, as the presets and the `.pen` import
    // store it. Effects always bind it.
    if is_fill && crate::shader_uniform_specs::declares_size(sksl) {
        program
            .uniforms
            .get_or_insert_with(BTreeMap::new)
            .entry("size".to_owned())
            .or_insert_with(|| ShaderUniformValue::Vec(vec![0.0, 0.0]));
    }
    let specs = uniform_specs(sksl, program.uniforms.as_ref());
    for spec in specs {
        let missing = program
            .uniforms
            .as_ref()
            .is_none_or(|map| !map.contains_key(&spec.name));
        if missing && spec.default.is_some() {
            let value = spec.value_in(None);
            program
                .uniforms
                .get_or_insert_with(BTreeMap::new)
                .insert(spec.name, value);
        }
    }
    true
}

impl EditorState {
    /// Apply `SetShaderUniform` / `SetShaderSource` (any other command is
    /// `false`).
    pub(crate) fn cmd_shader(&mut self, cmd: crate::EditorCommand) -> bool {
        match cmd {
            crate::EditorCommand::SetShaderUniform {
                node_id,
                target,
                name,
                value,
            } => self.cmd_set_shader_uniform(&node_id, target, &name, &value),
            crate::EditorCommand::SetShaderSource {
                node_id,
                target,
                sksl,
            } => self.cmd_set_shader_source(&node_id, target, &sksl),
            _ => false,
        }
    }

    /// `SetShaderUniform` — see [`set_shader_uniform`]. Editable-gated.
    fn cmd_set_shader_uniform(
        &mut self,
        node_id: &NodeId,
        target: ShaderTarget,
        name: &str,
        value: &ShaderUniformValue,
    ) -> bool {
        if !node_id.is_real() || !self.is_editable(node_id) {
            return false;
        }
        find_node_mut(self.active_children_mut(), node_id)
            .is_some_and(|node| set_shader_uniform(node, target, name, value))
    }

    /// `SetShaderSource` — see [`set_shader_source`]. Editable-gated.
    fn cmd_set_shader_source(
        &mut self,
        node_id: &NodeId,
        target: ShaderTarget,
        sksl: &str,
    ) -> bool {
        if !node_id.is_real() || !self.is_editable(node_id) {
            return false;
        }
        find_node_mut(self.active_children_mut(), node_id)
            .is_some_and(|node| set_shader_source(node, target, sksl))
    }

    /// The anchor node's shader paint `target`, if it is one.
    pub fn selected_shader_program(&self, target: ShaderTarget) -> Option<ShaderProgram<'_>> {
        self.selected_node()
            .and_then(|node| shader_program(node, target))
    }

    /// The uniform at row `index` of the anchor's shader paint `target`.
    pub fn selected_shader_uniform(
        &self,
        target: ShaderTarget,
        index: usize,
    ) -> Option<ShaderUniformSpec> {
        self.selected_shader_program(target)?
            .specs()
            .into_iter()
            .nth(index)
    }

    /// Write row `index` of the anchor's shader paint through
    /// `SetShaderUniform`. The caller owns history.
    pub fn set_selected_shader_uniform(
        &mut self,
        target: ShaderTarget,
        index: usize,
        value: ShaderUniformValue,
    ) -> bool {
        let node_id = self.selection.anchor.clone();
        let Some(spec) = self.selected_shader_uniform(target, index) else {
            return false;
        };
        self.apply(crate::EditorCommand::SetShaderUniform {
            node_id,
            target,
            name: spec.name,
            value,
        })
    }

    /// Write one number field of row `index`: the float itself, or one
    /// component of a vector. A ranged float is clamped to its range.
    pub fn set_selected_shader_uniform_number(
        &mut self,
        target: ShaderTarget,
        index: usize,
        component: usize,
        value: f32,
    ) -> bool {
        if !value.is_finite() {
            return false;
        }
        let Some(program) = self.selected_shader_program(target) else {
            return false;
        };
        let uniforms = program.uniforms;
        let Some(spec) = program.specs().into_iter().nth(index) else {
            return false;
        };
        let value = match spec.kind {
            crate::UniformKind::Float => {
                let value = spec.range.map_or(value, |(lo, hi)| value.clamp(lo, hi));
                ShaderUniformValue::Float(value)
            }
            crate::UniformKind::Int(1) => {
                let value = spec.range.map_or(value, |(lo, hi)| value.clamp(lo, hi));
                ShaderUniformValue::Float(value.round())
            }
            crate::UniformKind::Bool => {
                ShaderUniformValue::Float(f32::from(u8::from(value != 0.0)))
            }
            crate::UniformKind::Vector(_) | crate::UniformKind::Int(_) => {
                let int = matches!(spec.kind, crate::UniformKind::Int(_));
                let mut numbers = spec.numbers_in(uniforms);
                let Some(slot) = numbers.get_mut(component) else {
                    return false;
                };
                *slot = if int { value.round() } else { value };
                ShaderUniformValue::Vec(numbers)
            }
            crate::UniformKind::Color { .. } => return false,
        };
        self.set_selected_shader_uniform(target, index, value)
    }
}

#[cfg(test)]
#[path = "shader_edit_tests.rs"]
mod tests;
