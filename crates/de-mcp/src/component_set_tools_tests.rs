//! Component sets over MCP (#368): a three-axis Button set built from
//! existing masters, instances inserted by set id with `variant`, switches
//! keeping props, fallbacks and dropped overrides as `warnings`, the reads,
//! and refusals that name the allowed values.

use std::collections::BTreeMap;

use de_editor_core::ref_resolve::resolve_refs_for_canvas;
use de_editor_core::{EditorCommand, EditorState};
use serde_json::{json, Value};

use crate::batch_program_test_support::{binding_id, call_operations};
use crate::{
    component_set_tool_snapshot, get_component_snapshot, list_components_snapshot,
    update_node_snapshot_for, McpTool, ToolOutcome,
};

/// `Screen` (active) holds `b1` on `btn-primary-md-default`; five masters
/// of a Button (variant × size × state) sit on `Components`.
fn state() -> EditorState {
    let master = |id: &str, name: &str, width: f64| {
        json!({"type":"frame","id":id,"name":name,"reusable":true,"width":width,"height":40,
               "componentProperties":{"label":{"type":"text","default":"Button",
                   "bind":[{"node":format!("{id}-label"),"field":"content"}]}},
               "children":[{"type":"text","id":format!("{id}-label"),"name":"Label","content":"Button"},
                           {"type":"rectangle","id":format!("{id}-spinner"),"name":"Spinner","width":8,"height":8}]})
    };
    let doc = json!({"version":"1.0.0","children":[],"pages":[
        {"id":"p1","name":"Screen","children":[
            {"type":"frame","id":"screen","width":800,"height":600,"children":[
                {"type":"ref","id":"b1","ref":"btn-primary-md-default","props":{"label":"Save"},
                 "descendants":{"btn-primary-md-default-label":{"fontSize":18}}}]}]},
        {"id":"p2","name":"Other","children":[]},
        {"id":"p3","name":"Components","children":[
            master("btn-primary-md-default", "variant=primary, size=md, state=default", 120.0),
            master("btn-primary-sm-default", "variant=primary, size=sm, state=default", 90.0),
            master("btn-ghost-md-default", "variant=ghost, size=md, state=default", 120.0),
            master("btn-primary-md-loading", "variant=primary, size=md, state=loading", 120.0),
            master("btn-primary-lg-default", "variant=primary, size=lg, state=default", 160.0)]}
    ]});
    EditorState::from_document(jian_ops_schema::load_str(&doc.to_string()).unwrap().value)
}

fn args(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

fn call(state: &EditorState, tool: &'static str, pairs: &[(&str, &str)]) -> ToolOutcome {
    component_set_tool_snapshot(state, tool).call(&args(pairs))
}

fn ok(outcome: ToolOutcome) -> (Value, EditorCommand) {
    match outcome {
        ToolOutcome::OkJsonWithCommand(json, command) => {
            (serde_json::from_str(&json).unwrap(), command)
        }
        ToolOutcome::OkWithCommand(map, command) => (json!(map), command),
        other => panic!("expected a command, got {other:?}"),
    }
}

fn error(outcome: ToolOutcome) -> String {
    match outcome {
        ToolOutcome::Err(_, message) => message,
        other => panic!("expected an error, got {other:?}"),
    }
}

fn node(state: &EditorState, id: &str) -> Value {
    serde_json::to_value(de_editor_core::component_set::find_node(&state.doc, id).unwrap())
        .unwrap()
}

/// The fixture with its masters combined into `button`.
fn with_set() -> EditorState {
    let mut state = state();
    let (result, command) = ok(call(
        &state,
        "create_component_set",
        &[
            ("name", "Button"),
            (
                "master_ids",
                r#"["btn-primary-md-default","btn-primary-sm-default","btn-ghost-md-default","btn-primary-md-loading","btn-primary-lg-default"]"#,
            ),
        ],
    ));
    assert_eq!(result["setId"], "button");
    assert!(state.apply(command));
    state
}

#[test]
fn a_three_axis_set_is_created_from_existing_masters() {
    let state = with_set();
    let set = node(&state, "button");
    assert_eq!(
        set["componentSet"]["axes"],
        json!({"variant":["primary","ghost"],"size":["md","sm","lg"],"state":["default","loading"]})
    );
    let (sets, _) = crate::component_set_tools::sets_json(&state.doc);
    let sets: Value = serde_json::from_str(&sets.unwrap()).unwrap();
    assert_eq!(sets[0]["variants"].as_array().unwrap().len(), 5);
}

#[test]
fn explicit_axes_order_the_set_and_unknown_values_are_refused() {
    let state = state();
    let (result, _) = ok(call(
        &state,
        "create_component_set",
        &[
            ("name", "Button"),
            ("master_ids", "btn-primary-md-default, btn-primary-sm-default"),
            ("axes", r#"{"size":["sm","md","lg"],"variant":["primary"],"state":["default"]}"#),
        ],
    ));
    let names: Vec<&str> = result["set"]["axes"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(names, ["size", "variant", "state"]);
    let refused = error(call(
        &state,
        "create_component_set",
        &[
            ("name", "Button"),
            ("master_ids", "btn-primary-md-default, btn-ghost-md-default"),
            ("axes", r#"{"variant":["primary"],"size":["md"],"state":["default"]}"#),
        ],
    ));
    assert_eq!(refused, "unknown value ghost for axis variant; values: primary");
}

#[test]
fn instances_switch_variants_keeping_props_and_matching_overrides() {
    let mut state = with_set();
    let (result, command) = ok(call(
        &state,
        "set_instance_variant",
        &[("nodeId", "b1"), ("variant", r#"{"state":"loading"}"#)],
    ));
    assert_eq!(result["ref"], "btn-primary-md-loading");
    assert!(result.get("warnings").is_none());
    assert!(state.apply(command));
    let b1 = node(&state, "b1");
    assert_eq!(b1["props"], json!({"label":"Save"}));
    assert_eq!(
        b1["descendants"],
        json!({"btn-primary-md-loading-label":{"fontSize":18}})
    );
    let resolved = resolve_refs_for_canvas(&state.doc);
    let shown = serde_json::to_value(&resolved.pages.unwrap()[0].children[0]).unwrap();
    assert_eq!(shown["children"][0]["children"][0]["content"], "Save");

    // ghost/md/loading does not exist: nearest is ghost/md/default.
    let (result, _) = ok(call(
        &state,
        "set_instance_variant",
        &[("nodeId", "b1"), ("variant", r#"{"variant":"ghost"}"#)],
    ));
    assert_eq!(result["ref"], "btn-ghost-md-default");
    assert_eq!(result["variant"]["state"], "loading");
    assert!(result["warnings"][0]
        .as_str()
        .unwrap()
        .contains("no variant; it shows the nearest, btn-ghost-md-default"));

    let refused = error(call(
        &state,
        "set_instance_variant",
        &[("nodeId", "b1"), ("variant", r#"{"size":"xl"}"#)],
    ));
    assert_eq!(refused, "unknown value xl for axis size; values: md, sm, lg");
}

#[test]
fn batch_design_inserts_by_set_id_with_variant_and_switches_with_update() {
    let mut state = with_set();
    let (envelope, command) = call_operations(
        &state,
        r#"save=I("screen",{"type":"ref","ref":"button","variant":{"size":"sm"},"props":{"label":"Go"}})
ghost=I("screen",{"type":"ref","ref":"button","variant":{"variant":"ghost","size":"lg"}})"#,
    );
    assert!(state.apply(command.expect("insert")));
    let save = node(&state, &binding_id(&envelope, "save"));
    assert_eq!(save["ref"], "btn-primary-sm-default");
    assert_eq!(
        save["variant"],
        json!({"size":"sm","state":"default","variant":"primary"})
    );
    let ghost = node(&state, &binding_id(&envelope, "ghost"));
    assert_eq!(ghost["ref"], "btn-ghost-md-default");
    assert_eq!(envelope["warnings"].as_array().map(Vec::len), Some(1));

    let id = binding_id(&envelope, "save");
    let (_, command) = call_operations(
        &state,
        &format!(r#"U("{id}",{{"variant":{{"size":"lg"}},"props":{{"label":"Big"}}}})"#),
    );
    assert!(state.apply(command.expect("update")));
    let save = node(&state, &id);
    assert_eq!(save["ref"], "btn-primary-lg-default");
    assert_eq!(save["props"], json!({"label":"Big"}));

    let (envelope, command) = call_operations(
        &state,
        r#"I("screen",{"type":"ref","ref":"button","variant":{"tone":"x"}})"#,
    );
    assert!(command.is_none());
    assert!(envelope["errors"][0]["error"]
        .as_str()
        .unwrap()
        .contains("unknown variant axis tone; axes: variant, size, state"));
}

#[test]
fn update_node_switches_the_variant_and_patches_the_rest() {
    let mut state = with_set();
    let outcome = update_node_snapshot_for(&state).call(&args(&[
        ("nodeId", "b1"),
        ("data", r#"{"variant":{"size":"sm"},"name":"Small save"}"#),
    ]));
    let (_, command) = ok(outcome);
    assert!(matches!(command, EditorCommand::Batch { .. }));
    assert!(state.apply(command));
    let b1 = node(&state, "b1");
    assert_eq!(b1["ref"], "btn-primary-sm-default");
    assert_eq!(b1["name"], "Small save");
}

#[test]
fn reads_list_sets_with_axes_and_variants() {
    let mut state = with_set();
    let list = list_components_snapshot(&state).call(&BTreeMap::new());
    let ToolOutcome::Ok(list) = list else {
        panic!("list");
    };
    assert_eq!(list["count"], "0", "variant masters are listed under their set");
    let sets: Value = serde_json::from_str(&list["sets"]).unwrap();
    assert_eq!(sets[0]["name"], "Button");
    assert_eq!(sets[0]["default"]["size"], "md");

    let ToolOutcome::Ok(set) =
        get_component_snapshot(&state).call(&args(&[("component_id", "button")]))
    else {
        panic!("set detail");
    };
    assert_eq!(set["kind"], "component_set");
    assert_eq!(
        serde_json::from_str::<Value>(&set["variants"]).unwrap()[1]["values"]["size"],
        "sm"
    );
    let ToolOutcome::Ok(master) = get_component_snapshot(&state)
        .call(&args(&[("component_id", "btn-primary-sm-default")]))
    else {
        panic!("variant detail");
    };
    assert_eq!(master["set"], "button");

    // Renames and new variants answer with the set as it will be.
    let (result, command) = ok(call(
        &state,
        "rename_variant_value",
        &[
            ("set_id", "button"),
            ("axis", "size"),
            ("value", "sm"),
            ("newName", "small"),
        ],
    ));
    assert_eq!(result["set"]["axes"]["size"], json!(["md", "small", "lg"]));
    assert!(state.apply(command));
    let (result, _) = ok(call(
        &state,
        "add_variant",
        &[
            ("set_id", "button"),
            ("values", r#"{"variant":"ghost","state":"loading"}"#),
        ],
    ));
    assert_eq!(result["created"], json!(["button-ghost-md-loading"]));
}
