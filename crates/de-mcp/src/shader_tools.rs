//! Shader paint MCP write tools (#164): `set_shader_uniform` and
//! `set_shader_source`, the same writes the property panel's uniform rows
//! and source editor make (`EditorCommand::SetShaderUniform` /
//! `SetShaderSource`).
//!
//! Both name the paint with `paint` (`fill` | `effect`, default `fill`) and
//! `index` (default 0) into that list on the node. The applier checks the
//! uniform against what the program declares, so an unknown name or a
//! value of the wrong type is refused there.

use std::collections::BTreeMap;

use de_editor_core::ShaderTarget;
use jian_ops_schema::style::ShaderUniformValue;

use super::write_tools::parse_node_id;
use super::{EditorCommand, McpTool, ToolErrorCode, ToolOutcome};

fn invalid(message: String) -> ToolOutcome {
    ToolOutcome::Err(ToolErrorCode::InvalidArgument, message)
}

// `ToolOutcome` is the shared MCP outcome type, as in `parse_node_id`.
#[allow(clippy::result_large_err)]
fn parse_target(args: &BTreeMap<String, String>) -> Result<ShaderTarget, ToolOutcome> {
    let index = match args.get("index") {
        None => 0,
        Some(raw) => raw
            .trim()
            .parse::<usize>()
            .map_err(|_| invalid(format!("index must be a non-negative integer, got {raw:?}")))?,
    };
    match args.get("paint").map(|p| p.trim().to_ascii_lowercase()) {
        None => Ok(ShaderTarget::Fill(index)),
        Some(p) if p == "fill" => Ok(ShaderTarget::Fill(index)),
        Some(p) if p == "effect" => Ok(ShaderTarget::Effect(index)),
        Some(p) => Err(invalid(format!("paint must be fill or effect, got {p:?}"))),
    }
}

/// `0.5` → float, `1, 2` / `[1, 2]` → vector, `#ff0000` → colour.
#[allow(clippy::result_large_err)]
fn parse_value(raw: &str) -> Result<ShaderUniformValue, ToolOutcome> {
    let text = raw.trim();
    if text.starts_with('#') {
        return Ok(ShaderUniformValue::Color(text.to_owned()));
    }
    let inner = text.trim_start_matches('[').trim_end_matches(']');
    let numbers: Option<Vec<f32>> = inner
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|part| !part.is_empty())
        .map(|part| part.parse::<f32>().ok().filter(|v| v.is_finite()))
        .collect();
    match numbers.as_deref() {
        Some([one]) if !text.starts_with('[') => Ok(ShaderUniformValue::Float(*one)),
        Some(values) if (2..=4).contains(&values.len()) => {
            Ok(ShaderUniformValue::Vec(values.to_vec()))
        }
        _ => Err(invalid(format!(
            "value must be a number, 2-4 numbers or a #hex colour, got {raw:?}"
        ))),
    }
}

/// `set_shader_uniform` — store one user uniform of a shader fill / effect.
pub struct SetShaderUniform;

impl McpTool for SetShaderUniform {
    fn name(&self) -> &str {
        "set_shader_uniform"
    }
    fn call(&self, args: &BTreeMap<String, String>) -> ToolOutcome {
        let node_id = match parse_node_id(args, "node_id") {
            Ok(v) => v,
            Err(e) => return e,
        };
        let target = match parse_target(args) {
            Ok(t) => t,
            Err(e) => return e,
        };
        let Some(name) = args.get("name").map(|n| n.trim()).filter(|n| !n.is_empty()) else {
            return ToolOutcome::Err(ToolErrorCode::MissingArgument, "name is required".into());
        };
        let Some(raw) = args.get("value") else {
            return ToolOutcome::Err(ToolErrorCode::MissingArgument, "value is required".into());
        };
        let value = match parse_value(raw) {
            Ok(v) => v,
            Err(e) => return e,
        };
        let mut out = BTreeMap::new();
        out.insert("wrote".into(), "true".into());
        ToolOutcome::OkWithCommand(
            out,
            EditorCommand::SetShaderUniform {
                node_id,
                target,
                name: name.to_owned(),
                value,
            },
        )
    }
}

pub fn set_shader_uniform_snapshot() -> SetShaderUniform {
    SetShaderUniform
}

/// `set_shader_source` — replace the SkSL of a shader fill / effect.
pub struct SetShaderSource;

impl McpTool for SetShaderSource {
    fn name(&self) -> &str {
        "set_shader_source"
    }
    fn call(&self, args: &BTreeMap<String, String>) -> ToolOutcome {
        let node_id = match parse_node_id(args, "node_id") {
            Ok(v) => v,
            Err(e) => return e,
        };
        let target = match parse_target(args) {
            Ok(t) => t,
            Err(e) => return e,
        };
        let Some(sksl) = args.get("sksl").filter(|s| !s.trim().is_empty()) else {
            return ToolOutcome::Err(ToolErrorCode::MissingArgument, "sksl is required".into());
        };
        let mut out = BTreeMap::new();
        out.insert("wrote".into(), "true".into());
        ToolOutcome::OkWithCommand(
            out,
            EditorCommand::SetShaderSource {
                node_id,
                target,
                sksl: sksl.clone(),
            },
        )
    }
}

pub fn set_shader_source_snapshot() -> SetShaderSource {
    SetShaderSource
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn uniform_values_parse_by_shape() {
        assert_eq!(
            parse_value("0.5").ok(),
            Some(ShaderUniformValue::Float(0.5))
        );
        assert_eq!(
            parse_value("[1, 2]").ok(),
            Some(ShaderUniformValue::Vec(vec![1.0, 2.0]))
        );
        assert_eq!(
            parse_value("1 2 3").ok(),
            Some(ShaderUniformValue::Vec(vec![1.0, 2.0, 3.0]))
        );
        assert_eq!(
            parse_value("#ff0000").ok(),
            Some(ShaderUniformValue::Color("#ff0000".into()))
        );
        assert!(parse_value("nan").is_err());
        assert!(parse_value("1 2 3 4 5").is_err());
    }

    #[test]
    fn tools_emit_the_commands() {
        let out = SetShaderUniform.call(&args(&[
            ("node_id", "a"),
            ("paint", "effect"),
            ("index", "1"),
            ("name", "cell"),
            ("value", "12"),
        ]));
        match out {
            ToolOutcome::OkWithCommand(
                _,
                EditorCommand::SetShaderUniform {
                    target,
                    name,
                    value,
                    ..
                },
            ) => {
                assert_eq!(target, ShaderTarget::Effect(1));
                assert_eq!(name, "cell");
                assert_eq!(value, ShaderUniformValue::Float(12.0));
            }
            other => panic!("{other:?}"),
        }
        match SetShaderSource.call(&args(&[
            ("node_id", "a"),
            ("sksl", "half4 main(float2 p){return half4(1);}"),
        ])) {
            ToolOutcome::OkWithCommand(_, EditorCommand::SetShaderSource { target, .. }) => {
                assert_eq!(target, ShaderTarget::Fill(0));
            }
            other => panic!("{other:?}"),
        }
        assert!(matches!(
            SetShaderUniform.call(&args(&[
                ("node_id", "a"),
                ("paint", "stroke"),
                ("name", "x"),
                ("value", "1")
            ])),
            ToolOutcome::Err(ToolErrorCode::InvalidArgument, _)
        ));
        assert!(matches!(
            SetShaderSource.call(&args(&[("node_id", "a")])),
            ToolOutcome::Err(ToolErrorCode::MissingArgument, _)
        ));
    }
}
