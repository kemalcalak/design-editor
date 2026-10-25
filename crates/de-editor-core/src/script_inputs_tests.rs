use super::*;
use jian_ops_schema::variable::{VariableKind, VariableValue};

const SOURCE: &str = r#"/** @schema 2.11
 * @input bars: number = 5
 * @input title: string = "Sales"
 * @input accent: color = #3b82f6
 * @input mode: enum(bars, "lines") = lines
 * @input shape: "round" | "square"
 * @input stacked: boolean = true
 * @input anchor: ref
 * @input bars: number = 9
 * @input broken number
 */
return [];"#;

#[test]
fn declarations_are_read_in_order_with_their_defaults() {
    let specs = script_input_specs(SOURCE);
    let names: Vec<&str> = specs.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(
        names,
        ["bars", "title", "accent", "mode", "shape", "stacked", "anchor"]
    );
    assert_eq!(specs[0].default, Some(ScriptInputValue::Number(5.0)));
    assert_eq!(
        specs[1].default,
        Some(ScriptInputValue::Text("Sales".into()))
    );
    assert_eq!(specs[2].kind, ScriptInputKind::Color);
    assert_eq!(
        specs[3].kind,
        ScriptInputKind::Enum(vec!["bars".into(), "lines".into()])
    );
    assert_eq!(
        specs[3].default,
        Some(ScriptInputValue::Text("lines".into()))
    );
    assert_eq!(
        specs[4].kind,
        ScriptInputKind::Enum(vec!["round".into(), "square".into()])
    );
    assert_eq!(specs[5].default, Some(ScriptInputValue::Bool(true)));
    assert_eq!(specs[6].kind, ScriptInputKind::Ref);
    assert_eq!(specs[6].default, None);
}

#[test]
fn a_default_outside_the_enum_is_dropped() {
    let specs = script_input_specs("// @input mode: enum(a, b) = c");
    assert_eq!(specs[0].default, None);
}

fn vars() -> BTreeMap<String, VariableDefinition> {
    let mut vars = BTreeMap::new();
    vars.insert(
        "brand".to_string(),
        VariableDefinition {
            kind: VariableKind::Color,
            value: VariableValue::Scalar(VariableScalar::Str("#ff0000".into())),
        },
    );
    vars
}

#[test]
fn resolved_inputs_fill_defaults_zeros_and_bindings() {
    let specs = script_input_specs(SOURCE);
    let mut stored = BTreeMap::new();
    stored.insert("bars".to_string(), ScriptInputValue::Number(3.0));
    stored.insert(
        "accent".to_string(),
        ScriptInputValue::Text("$brand".into()),
    );
    // A value of the wrong kind falls back to the default.
    stored.insert("stacked".to_string(), ScriptInputValue::Text("yes".into()));
    stored.insert("extra".to_string(), ScriptInputValue::Bool(true));
    let vars = vars();
    let out = resolved_script_inputs(&specs, Some(&stored), Some(&vars), &BTreeMap::new());
    assert_eq!(out["bars"], Value::from(3.0));
    assert_eq!(out["title"], Value::from("Sales"));
    assert_eq!(out["accent"], Value::from("#ff0000"));
    assert_eq!(out["mode"], Value::from("lines"));
    assert_eq!(out["shape"], Value::from("round"));
    assert_eq!(out["stacked"], Value::Bool(true));
    assert_eq!(out["anchor"], Value::Null);
    assert_eq!(out["extra"], Value::Bool(true));
}
