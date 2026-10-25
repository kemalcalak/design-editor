//! The inputs a script node's program declares (#19).
//!
//! pen.dev's convention: anywhere in the source (usually the header
//! comment) a line declares one input,
//!
//! ```text
//! /** @schema 2.11
//!  * @input bars: number = 5
//!  * @input title: string = "Sales"
//!  * @input accent: color = #3b82f6
//!  * @input mode: enum(bars, lines) = bars
//!  */
//! ```
//!
//! Types are `number`, `string`, `boolean`, `color`, `ref` (a node id) and
//! `enum`, whose choices sit in parentheses or are written as
//! `"a" | "b"`. The schema has no field for the declarations, so they are
//! read from the source every time; the source stays the single source of
//! truth, like shader uniform annotations.

use std::collections::BTreeMap;

use jian_ops_schema::node::ScriptInputValue;
use jian_ops_schema::variable::{VariableDefinition, VariableScalar};
use serde_json::Value;

use crate::variables_resolve::resolve_variable_ref;

/// What control an input gets and how its value is read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScriptInputKind {
    Number,
    String,
    Boolean,
    /// A hex colour string.
    Color,
    /// A node id.
    Ref,
    /// One of these choices.
    Enum(Vec<String>),
}

/// One declared input.
#[derive(Debug, Clone, PartialEq)]
pub struct ScriptInputSpec {
    pub name: String,
    pub kind: ScriptInputKind,
    /// The `= default` value, already in the kind's form.
    pub default: Option<ScriptInputValue>,
}

/// Every `@input` declaration in `source`, in order. Malformed lines and
/// repeated names (after the first) are skipped.
pub fn script_input_specs(source: &str) -> Vec<ScriptInputSpec> {
    let mut specs: Vec<ScriptInputSpec> = Vec::new();
    for line in source.lines() {
        let Some(at) = line.find("@input") else {
            continue;
        };
        let Some(spec) = parse_declaration(&line[at + "@input".len()..]) else {
            continue;
        };
        if specs.iter().all(|known| known.name != spec.name) {
            specs.push(spec);
        }
    }
    specs
}

fn parse_declaration(rest: &str) -> Option<ScriptInputSpec> {
    // Comment closers end the declaration.
    let rest = rest.split("*/").next().unwrap_or(rest).trim();
    let (name, rest) = rest.split_once(':')?;
    let name = name.trim();
    if name.is_empty() || !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return None;
    }
    let (type_text, default_text) = match split_default(rest) {
        Some((ty, default)) => (ty.trim(), Some(default.trim())),
        None => (rest.trim(), None),
    };
    let kind = parse_kind(type_text)?;
    let default = default_text.and_then(|text| parse_value(&kind, text));
    Some(ScriptInputSpec {
        name: name.to_string(),
        kind,
        default,
    })
}

/// Split `type = default` at the first `=` outside quotes and parentheses.
fn split_default(text: &str) -> Option<(&str, &str)> {
    let mut depth = 0usize;
    let mut quote: Option<char> = None;
    for (i, c) in text.char_indices() {
        match (quote, c) {
            (Some(q), _) if c == q => quote = None,
            (Some(_), _) => {}
            (None, '"' | '\'') => quote = Some(c),
            (None, '(') => depth += 1,
            (None, ')') => depth = depth.saturating_sub(1),
            (None, '=') if depth == 0 => return Some((&text[..i], &text[i + 1..])),
            _ => {}
        }
    }
    None
}

fn parse_kind(text: &str) -> Option<ScriptInputKind> {
    let kind = match text {
        "number" => ScriptInputKind::Number,
        "string" => ScriptInputKind::String,
        "boolean" | "bool" => ScriptInputKind::Boolean,
        "color" => ScriptInputKind::Color,
        "ref" => ScriptInputKind::Ref,
        _ => {
            let choices = if let Some(inner) = text
                .strip_prefix("enum")
                .map(str::trim_start)
                .and_then(|t| t.strip_prefix('('))
                .and_then(|t| t.strip_suffix(')'))
            {
                inner.split(',').map(unquote).collect::<Vec<_>>()
            } else if text.contains('|') {
                text.split('|').map(unquote).collect()
            } else {
                return None;
            };
            let choices: Vec<String> = choices.into_iter().filter(|c| !c.is_empty()).collect();
            if choices.is_empty() {
                return None;
            }
            ScriptInputKind::Enum(choices)
        }
    };
    Some(kind)
}

fn unquote(text: &str) -> String {
    let text = text.trim();
    let inner = text
        .strip_prefix('"')
        .and_then(|t| t.strip_suffix('"'))
        .or_else(|| text.strip_prefix('\'').and_then(|t| t.strip_suffix('\'')));
    inner.unwrap_or(text).to_string()
}

fn parse_value(kind: &ScriptInputKind, text: &str) -> Option<ScriptInputValue> {
    match kind {
        ScriptInputKind::Number => text
            .parse::<f64>()
            .ok()
            .filter(|n| n.is_finite())
            .map(ScriptInputValue::Number),
        ScriptInputKind::Boolean => match text {
            "true" => Some(ScriptInputValue::Bool(true)),
            "false" => Some(ScriptInputValue::Bool(false)),
            _ => None,
        },
        ScriptInputKind::Enum(choices) => {
            let choice = unquote(text);
            choices
                .contains(&choice)
                .then_some(ScriptInputValue::Text(choice))
        }
        ScriptInputKind::String | ScriptInputKind::Color | ScriptInputKind::Ref => {
            Some(ScriptInputValue::Text(unquote(text)))
        }
    }
}

impl ScriptInputSpec {
    /// The value an input has when nothing is stored and there is no
    /// default.
    fn zero(&self) -> Value {
        match &self.kind {
            ScriptInputKind::Number => Value::from(0.0),
            ScriptInputKind::Boolean => Value::Bool(false),
            ScriptInputKind::Color => Value::from("#000000"),
            ScriptInputKind::Ref => Value::Null,
            ScriptInputKind::String => Value::from(""),
            ScriptInputKind::Enum(choices) => Value::from(choices[0].as_str()),
        }
    }

    /// `value` in this input's form, `None` when it cannot be one.
    fn coerce(&self, value: Value) -> Option<Value> {
        let fits = match &self.kind {
            ScriptInputKind::Number => value.is_number(),
            ScriptInputKind::Boolean => value.is_boolean(),
            ScriptInputKind::Enum(choices) => value
                .as_str()
                .is_some_and(|text| choices.iter().any(|c| c == text)),
            ScriptInputKind::String | ScriptInputKind::Color | ScriptInputKind::Ref => {
                value.is_string()
            }
        };
        fits.then_some(value)
    }
}

/// The value the program sees for one stored input: a `$variable` binding
/// is resolved under `theme`; an unresolvable binding is `None`.
fn concrete(
    value: &ScriptInputValue,
    vars: Option<&BTreeMap<String, VariableDefinition>>,
    theme: &BTreeMap<String, String>,
) -> Option<Value> {
    if value.variable_ref().is_some() {
        let ScriptInputValue::Text(reference) = value else {
            return None;
        };
        return Some(match resolve_variable_ref(reference, vars, theme)? {
            VariableScalar::Bool(b) => Value::Bool(b),
            VariableScalar::Num(n) => Value::from(n),
            VariableScalar::Str(s) => Value::from(s),
        });
    }
    Some(match value {
        ScriptInputValue::Bool(b) => Value::Bool(*b),
        ScriptInputValue::Number(n) => Value::from(*n),
        ScriptInputValue::Text(s) => Value::from(s.as_str()),
    })
}

/// The `pencil.input` object the program runs with: every declared input
/// (stored value, else default, else the kind's zero) plus any stored
/// value the source does not declare. Bindings are resolved, so a variable
/// change changes this map.
pub fn resolved_script_inputs(
    specs: &[ScriptInputSpec],
    stored: Option<&BTreeMap<String, ScriptInputValue>>,
    vars: Option<&BTreeMap<String, VariableDefinition>>,
    theme: &BTreeMap<String, String>,
) -> BTreeMap<String, Value> {
    let mut out = BTreeMap::new();
    for spec in specs {
        let value = stored
            .and_then(|map| map.get(&spec.name))
            .and_then(|value| concrete(value, vars, theme))
            .and_then(|value| spec.coerce(value))
            .or_else(|| {
                spec.default
                    .as_ref()
                    .and_then(|value| concrete(value, vars, theme))
            })
            .unwrap_or_else(|| spec.zero());
        out.insert(spec.name.clone(), value);
    }
    for (name, value) in stored.into_iter().flatten() {
        if !out.contains_key(name) {
            if let Some(value) = concrete(value, vars, theme) {
                out.insert(name.clone(), value);
            }
        }
    }
    out
}

#[cfg(test)]
#[path = "script_inputs_tests.rs"]
mod tests;
