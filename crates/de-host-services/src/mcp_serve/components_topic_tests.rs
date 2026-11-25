//! The `components` guideline topic's worked examples (#366), run as written
//! against the MCP host: each must leave linked `ref` instances only (no
//! detached copies, no dangling targets), `list_components` must report its
//! masters, and one master edit must change every instance.

use std::collections::BTreeMap;

use serde_json::{json, Value};

use super::*;
use de_editor_core::ref_resolve::resolve_refs_for_canvas;
use de_editor_core::variables_resolve::resolve_document_for_canvas;

fn call(state: &mut EditorState, tool: &str, args: Value) -> Value {
    let line = json!({
        "jsonrpc": "2.0", "id": 1, "method": "tools/call",
        "params": {"name": tool, "arguments": args}
    })
    .to_string();
    let response =
        process_message_with_applier(state, &line, |_, state, cmd| state.apply(cmd.clone()))
            .expect("dispatch")
            .expect("response");
    assert!(
        !response.contains(r#""isError":true"#),
        "{tool}: {response}"
    );
    serde_json::from_str(&tool_text(&response)).expect("tool result JSON")
}

fn guide() -> String {
    de_ai_skills::guideline_for("components").expect("components topic")
}

/// The fenced `lang` blocks of `text`, in order.
fn fenced(text: &str, lang: &str) -> Vec<String> {
    text.split(&format!("```{lang}\n"))
        .skip(1)
        .filter_map(|block| block.split("```").next())
        .map(str::to_string)
        .collect()
}

/// The script of worked example `n`, taken from the topic text itself so
/// the guide and this test cannot drift.
fn example_script(n: usize) -> String {
    let marker = format!("// Example {n}:");
    fenced(&guide(), "js")
        .into_iter()
        .find(|block| block.starts_with(&marker))
        .unwrap_or_else(|| panic!("components topic must carry example {n}"))
}

fn run_example(state: &mut EditorState, n: usize) {
    let out = call(state, "batch_design", json!({"script": example_script(n)}));
    assert!(out["results"].is_array(), "example {n}: {out}");
}

/// Every node below `value` (a document or a node), depth-first.
fn nodes(value: &Value) -> Vec<&Value> {
    let mut out = Vec::new();
    fn walk<'a>(value: &'a Value, out: &mut Vec<&'a Value>) {
        if let Some(children) = value["children"].as_array() {
            for child in children {
                out.push(child);
                walk(child, out);
            }
        }
    }
    walk(value, &mut out);
    out
}

fn doc_json(state: &EditorState) -> Value {
    serde_json::to_value(&state.doc).expect("document JSON")
}

fn named<'a>(value: &'a Value, name: &str) -> &'a Value {
    nodes(value)
        .into_iter()
        .find(|node| node["name"] == name)
        .unwrap_or_else(|| panic!("no node named {name}"))
}

fn id_of(value: &Value, name: &str) -> String {
    named(value, name)["id"]
        .as_str()
        .expect("node id")
        .to_string()
}

/// The example left linked instances only: exactly `masters` are reusable
/// and registered, every `ref` names a registered master, `count` refs point
/// at the master named `repeated`, and no plain frame repeats a master's
/// name (a hand-built or detached copy).
fn assert_linked_only(state: &mut EditorState, masters: &[&str], repeated: &str, count: usize) {
    let listed = call(state, "list_components", json!({}));
    assert_eq!(listed["count"], masters.len().to_string(), "{listed}");
    let doc = doc_json(state);
    let mut registered = BTreeMap::new();
    for master in masters {
        let id = id_of(&doc, master);
        assert!(
            listed["components"]
                .as_str()
                .is_some_and(|all| all.contains(&format!("{master}|{id}"))),
            "list_components must report {master}|{id}: {listed}"
        );
        registered.insert(id, *master);
    }
    let all = nodes(&doc);
    for node in &all {
        if node["type"] == "ref" {
            let target = node["ref"].as_str().unwrap_or_default();
            assert!(
                registered.contains_key(target),
                "ref {} points at {target}, not a registered master",
                node["id"]
            );
            assert!(node.get("fill").is_none(), "no visual props on a ref");
        }
        if let Some(name) = node["name"].as_str() {
            if masters.contains(&name) {
                assert_eq!(
                    node["reusable"], true,
                    "{name} appears as a plain frame: a copy, not an instance"
                );
            }
        }
    }
    let repeated_id = id_of(&doc, repeated);
    let linked = all
        .iter()
        .filter(|node| node["type"] == "ref" && node["ref"] == repeated_id.as_str())
        .count();
    assert_eq!(linked, count, "linked instances of {repeated}");
}

/// The instances of `master` as the canvas expands and resolves them.
fn rendered_instances(state: &EditorState, master: &str) -> Vec<Value> {
    let doc = doc_json(state);
    let master_id = id_of(&doc, master);
    let ids: Vec<String> = nodes(&doc)
        .into_iter()
        .filter(|node| node["type"] == "ref" && node["ref"] == master_id.as_str())
        .map(|node| node["id"].as_str().expect("ref id").to_string())
        .collect();
    let expanded = resolve_refs_for_canvas(&state.doc);
    let resolved = resolve_document_for_canvas(&expanded, &BTreeMap::new());
    let resolved = serde_json::to_value(&resolved).expect("resolved JSON");
    ids.iter()
        .map(|id| {
            nodes(&resolved)
                .into_iter()
                .find(|node| node["id"] == id.as_str())
                .unwrap_or_else(|| panic!("instance {id} did not render"))
                .clone()
        })
        .collect()
}

/// `field` of the node named `name` inside each instance.
fn child_field(instances: &[Value], name: &str, field: &str) -> Vec<Value> {
    instances
        .iter()
        .map(|instance| named(instance, name)[field].clone())
        .collect()
}

fn edit(state: &mut EditorState, operations: String) {
    call(state, "batch_design", json!({"operations": operations}));
}

#[test]
fn example_1_cards_are_one_master_and_linked_instances() {
    let mut state = EditorState::new();
    run_example(&mut state, 1);
    assert_linked_only(&mut state, &["Card/Product"], "Card/Product", 4);

    let cards = rendered_instances(&state, "Card/Product");
    assert_eq!(
        child_field(&cards, "Title", "content"),
        ["Desk lamp", "Chair", "Shelf", "Rug"].map(Value::from),
        "each instance shows its own content"
    );

    let title = id_of(&doc_json(&state), "Title");
    edit(&mut state, format!(r#"U("{title}",{{"fontSize":20}})"#));
    let cards = rendered_instances(&state, "Card/Product");
    assert_eq!(
        child_field(&cards, "Title", "fontSize"),
        vec![json!(20.0); 4]
    );
    assert_eq!(
        child_field(&cards, "Title", "content"),
        ["Desk lamp", "Chair", "Shelf", "Rug"].map(Value::from),
        "overrides survive the master edit"
    );
}

#[test]
fn example_2_themed_instances_share_one_master() {
    let mut state = EditorState::new();
    // The setup calls are the topic's own JSON blocks too.
    let setup = fenced(&guide(), "json");
    let [themes, variables] =
        [&setup[0], &setup[1]].map(|args| serde_json::from_str::<Value>(args).expect("setup JSON"));
    call(&mut state, "set_themes", themes);
    call(&mut state, "set_variables", variables);
    run_example(&mut state, 2);
    assert_linked_only(&mut state, &["Card/Stat"], "Card/Stat", 2);

    let stats = rendered_instances(&state, "Card/Stat");
    let surfaces: Vec<Value> = stats
        .iter()
        .map(|stat| stat["fill"][0]["color"].clone())
        .collect();
    assert_eq!(surfaces, [json!("#FFFFFF"), json!("#111827")]);
    assert_eq!(
        child_field(&stats, "Value", "fill")
            .iter()
            .map(|fill| fill[0]["color"].clone())
            .collect::<Vec<_>>(),
        [json!("#111827"), json!("#F9FAFB")],
        "each instance resolves the variables in its own theme"
    );

    let value = id_of(&doc_json(&state), "Value");
    edit(&mut state, format!(r#"U("{value}",{{"fontSize":32}})"#));
    let stats = rendered_instances(&state, "Card/Stat");
    assert_eq!(
        child_field(&stats, "Value", "fontSize"),
        vec![json!(32.0); 2]
    );
}

#[test]
fn example_3_tiers_nest_the_button_instance() {
    let mut state = EditorState::new();
    run_example(&mut state, 3);
    assert_linked_only(
        &mut state,
        &["Button/Primary", "Card/PricingTier"],
        "Card/PricingTier",
        3,
    );

    let tiers = rendered_instances(&state, "Card/PricingTier");
    let labels: Vec<Value> = tiers
        .iter()
        .map(|tier| named(named(tier, "CTA"), "Label")["content"].clone())
        .collect();
    assert_eq!(
        labels,
        ["Start free", "Upgrade", "Contact sales"].map(Value::from),
        "each tier sets its nested button label"
    );

    let button = id_of(&doc_json(&state), "Button/Primary");
    edit(
        &mut state,
        format!(r#"U("{button}",{{"cornerRadius":999}})"#),
    );
    let tiers = rendered_instances(&state, "Card/PricingTier");
    for tier in &tiers {
        assert_eq!(named(tier, "CTA")["cornerRadius"], json!(999.0), "{tier}");
    }
}

#[test]
fn get_editor_state_points_at_the_components_topic() {
    let mut state = EditorState::new();
    let empty = call(&mut state, "get_editor_state", json!({}));
    assert_eq!(empty["component_count"], "0");
    assert!(empty.get("components_hint").is_none());

    run_example(&mut state, 1);
    let with_master = call(&mut state, "get_editor_state", json!({}));
    assert_eq!(with_master["component_count"], "1");
    assert!(with_master["components_hint"]
        .as_str()
        .is_some_and(|hint| hint.contains("topic=components")));

    let card = id_of(&doc_json(&state), "Card/Product");
    let detail = call(&mut state, "get_component", json!({"component_id": card}));
    let keys = detail["descendants"].as_str().expect("descendants record");
    assert!(
        keys.contains("|Title|text") && keys.contains("|Price|text"),
        "get_component lists the descendants keys: {keys}"
    );
}
