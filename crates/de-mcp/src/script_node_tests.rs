use super::*;
use de_editor_core::{NodeId, PenNodeExt};
use std::collections::BTreeMap;

fn job(source: &str) -> ScriptJob {
    ScriptJob {
        node_id: NodeId::new("s"),
        source: source.to_string(),
        width: 200.0,
        height: 100.0,
        inputs: BTreeMap::from([("bars".to_string(), Value::from(3.0))]),
        hash: "h".into(),
    }
}

#[test]
fn the_program_sees_size_and_inputs_and_returns_nodes() {
    let nodes = run_script_node(&job(r##"/** @schema 2.11
 * @input bars: number = 5 */
const w = pencil.width / pencil.input.bars;
const out = [];
for (let i = 0; i < pencil.input.bars; i++) {
  out.push({ type: "rectangle", x: i * w, y: 0, width: w - 2, height: pencil.height,
             fill: "#3b82f6" });
}
return out;"##))
    .unwrap();
    assert_eq!(nodes.len(), 3);
    assert_eq!(nodes[1].base().x, Some(200.0 / 3.0));
    assert_eq!(nodes[2].height_px(), Some(100.0));
}

#[test]
fn math_random_is_the_same_on_every_run() {
    let source = r#"return [{ type: "text", content: String(Math.random()) }];"#;
    let a = serde_json::to_string(&run_script_node(&job(source)).unwrap()).unwrap();
    let b = serde_json::to_string(&run_script_node(&job(source)).unwrap()).unwrap();
    assert_eq!(a, b);
}

#[test]
fn nested_children_come_back_as_a_tree() {
    let nodes = run_script_node(&job(r#"return [{ type: "frame", width: 50, height: 50,
            children: [{ type: "ellipse", width: 10, height: 10 }] }];"#))
    .unwrap();
    assert_eq!(nodes[0].children().map(Vec::len), Some(1));
}

#[test]
fn a_runaway_program_is_stopped_by_the_time_limit() {
    let started = Instant::now();
    let error = run_script_node(&job("while (true) {}")).unwrap_err();
    assert_eq!(
        error,
        ScriptNodeError::TimedOut {
            limit_ms: TIME_LIMIT.as_millis() as u64
        }
    );
    assert!(started.elapsed() < TIME_LIMIT * 3);
}

#[test]
fn bad_programs_report_why() {
    let threw = run_script_node(&job("throw new Error('no data')")).unwrap_err();
    assert_eq!(threw.to_string(), "script error: no data");
    assert_eq!(
        run_script_node(&job("return 4;")).unwrap_err(),
        ScriptNodeError::NotAnArray
    );
    assert_eq!(
        run_script_node(&job("")).unwrap_err(),
        ScriptNodeError::NotAnArray
    );
    assert!(matches!(
        run_script_node(&job("return [{ type: 'rectangle', x: 'left' }];")).unwrap_err(),
        ScriptNodeError::InvalidNode { index: 0, .. }
    ));
    assert_eq!(
        run_script_node(&job("return [{ type: 'script', script: 'return []' }];")).unwrap_err(),
        ScriptNodeError::NestedScript { index: 0 }
    );
}

#[test]
fn output_is_capped() {
    let error = run_script_node(&job(
        "return Array.from({ length: 2001 }, () => ({ type: 'rectangle', width: 1, height: 1 }));",
    ))
    .unwrap_err();
    assert_eq!(
        error,
        ScriptNodeError::TooManyNodes {
            count: MAX_NODES + 1,
            max: MAX_NODES
        }
    );
}

#[test]
fn stale_scripts_regenerate_in_place() {
    let doc = r##"{"version":"1.0.0","children":[
      {"type":"script","id":"bars","width":90,"height":30,
       "script":"// @input n: number = 2\nreturn Array.from({length: pencil.input.n}, (_, i) => ({type: 'rectangle', x: i * 30, width: 20, height: pencil.height}));",
       "inputs":{"n":3}}]}"##;
    let mut state = EditorState::from_document(jian_ops_schema::load_str(doc).unwrap().value);
    assert!(regenerate_stale_scripts(&mut state));
    let script = &state.doc.children[0];
    let ids: Vec<&str> = script
        .children()
        .unwrap()
        .iter()
        .map(|c| c.id_str())
        .collect();
    assert_eq!(ids, ["bars-0", "bars-1", "bars-2"]);
    // Already current: nothing to do.
    assert!(!regenerate_stale_scripts(&mut state));
}

#[test]
fn the_insert_dsl_refuses_children_under_a_script() {
    let program = r#"s=I(null,{"type":"script","width":50,"height":50,"script":"return [];"})
I(s,{"type":"text","content":"hand-made"})"#;
    let error = crate::batch_design_dsl::parse_insert_operations(program).unwrap_err();
    assert!(error.to_string().contains("is a script node"), "{error}");
}
