use super::*;
use serde_json::json;

fn state_with_duplicates() -> EditorState {
    let doc = json!({"version":"1.0.0","children":[],"pages":[
        {"id":"p1","name":"Home","children":[
            {"type":"frame","id":"n3","name":"Hero","width":10,"height":10}]},
        {"id":"p2","name":"Shop","children":[
            {"type":"frame","id":"card","reusable":true,"width":50,"height":50,
             "children":[{"type":"frame","id":"slot","width":50,"height":50}]},
            {"type":"ref","id":"inst","ref":"card","descendants":{
                "slot":{"children":[{"type":"text","id":"n3","name":"Price","content":"9"}]}}}]}
    ]});
    EditorState::from_document(
        jian_ops_schema::load_str(&doc.to_string())
            .expect("fixture")
            .value,
    )
}

fn call(state: &EditorState, args: &[(&str, &str)]) -> ToolOutcome {
    let args = args
        .iter()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect();
    repair_duplicate_ids_snapshot(state).call(&args)
}

#[test]
fn dry_run_reports_without_a_command() {
    let state = state_with_duplicates();
    let ToolOutcome::OkJson(result) = call(&state, &[("dryRun", "true")]) else {
        panic!("a dry run returns JSON only");
    };
    let result: Value = serde_json::from_str(&result).expect("json");
    assert_eq!(result["dryRun"], true);
    assert_eq!(result["duplicates"][0]["id"], "n3");
    assert_eq!(result["duplicates"][0]["pages"], json!(["Home", "Shop"]));
    assert_eq!(result["renamed"][0]["id"], "n3");
    assert_eq!(result["renamed"][0]["page"], "Shop");
    assert_eq!(result["renamed"][0]["name"], "Price");
}

#[test]
fn apply_repairs_with_the_predicted_ids() {
    let mut state = state_with_duplicates();
    let ToolOutcome::OkJsonWithCommand(result, command) = call(&state, &[]) else {
        panic!("a repair carries the command");
    };
    let result: Value = serde_json::from_str(&result).expect("json");
    assert!(state.apply(command));
    assert!(state.duplicate_ids().is_empty());
    let new_id = result["renamed"][0]["newId"].as_str().expect("new id");
    assert!(state
        .collect_node_ids()
        .contains(&de_editor_core::NodeId::new(new_id)));
}

#[test]
fn a_clean_document_has_nothing_to_apply() {
    let state = EditorState::new();
    let ToolOutcome::OkJson(result) = call(&state, &[]) else {
        panic!("nothing to apply");
    };
    assert!(result.contains("\"duplicateCount\":0"), "{result}");
    assert!(matches!(
        call(&state, &[("dryRun", "maybe")]),
        ToolOutcome::Err(ToolErrorCode::InvalidArgument, _)
    ));
}
