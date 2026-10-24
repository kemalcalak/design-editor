//! `generate` / `get_generation_status` tool tests — scripted backend, no
//! network, own job queue per test.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use de_editor_core::agent_settings::{ImageGenProfile, ImageGenProvider, ImageTestStatus};
use de_editor_core::{EditorCommand, EditorState, NodeId};
use de_mcp::{McpTool, ToolErrorCode, ToolOutcome};
use jian_ops_schema::node::PenNode;
use jian_ops_schema::style::PenFill;

use super::generate_tool::{GenerateTool, GenerationStatusTool};
use crate::generate_jobs::{
    land_into, EditJobRequest, GenerateBackend, GenerateJobError, GenerateJobs, ImageJobRequest,
    StockJobRequest, SvgJobRequest, PENDING_IMAGE_PREFIX,
};

const PNG: &str = "data:image/png;base64,iVBORw0KGgo=";

/// One provider call as the backend saw it: prompt, width, height.
type SeenCall = (String, Option<f64>, Option<f64>);

#[derive(Default)]
struct Scripted {
    answer: Option<String>,
    seen: Mutex<Vec<SeenCall>>,
}

impl GenerateBackend for Scripted {
    fn generate_image(&self, request: &ImageJobRequest) -> Result<String, GenerateJobError> {
        self.seen
            .lock()
            .unwrap()
            .push((request.prompt.clone(), request.width, request.height));
        self.answer.clone().ok_or_else(|| {
            GenerateJobError::Image(crate::web_image_generate::ImageGenerateError::Provider(
                "quota exceeded".into(),
            ))
        })
    }

    fn stock_image(&self, request: &StockJobRequest) -> Result<String, GenerateJobError> {
        self.seen
            .lock()
            .unwrap()
            .push((request.query.clone(), None, None));
        self.answer.clone().ok_or(GenerateJobError::NoStockResult)
    }

    fn generate_svg(&self, _request: &SvgJobRequest) -> Result<String, GenerateJobError> {
        Err(GenerateJobError::NoSvg)
    }

    fn edit_image(&self, _request: &EditJobRequest) -> Result<String, GenerateJobError> {
        Err(GenerateJobError::NotConfigured)
    }
}

fn state_with_frame(configured: bool) -> EditorState {
    let source = r##"{"version":"1.0","children":[
        {"type":"frame","id":"hero","width":1440,"height":800,"fill":[{"type":"solid","color":"#112233"}]},
        {"type":"text","id":"label","content":"Hi"}
    ]}"##;
    let mut state =
        crate::doc_io::load_editor_state_from_source(source, de_editor_core::Locale::EnUs)
            .expect("fixture");
    if configured {
        state
            .editor_ui
            .agent_settings
            .image_gen_profiles
            .push(ImageGenProfile {
                id: "img".into(),
                name: "Images".into(),
                provider: ImageGenProvider::OpenAi,
                api_key: "sk-test".into(),
                model: "gpt-image-1".into(),
                base_url: None,
                test_status: ImageTestStatus::Idle,
            });
    }
    state
}

fn args(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

fn hero_fills(state: &EditorState) -> Vec<PenFill> {
    match de_editor_core::walkers::find_node(state.active_children(), &NodeId::new("hero")) {
        Some(PenNode::Frame(frame)) => frame.container.fill.clone().unwrap_or_default(),
        other => panic!("hero frame missing: {other:?}"),
    }
}

/// Call `generate`, apply its pending write, wait for the job, land it.
fn run(
    state: &mut EditorState,
    jobs: &Arc<GenerateJobs>,
    call: &[(&str, &str)],
) -> serde_json::Value {
    let tool = GenerateTool::for_test(state, Arc::clone(jobs));
    let ToolOutcome::OkJsonWithCommand(json, command) = tool.call(&args(call)) else {
        panic!("generate did not return a write");
    };
    assert!(matches!(command, EditorCommand::PatchNodeData { .. }));
    assert!(state.apply(command), "pending write applies");
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    let pending = value["url"].as_str().unwrap();
    assert!(pending.starts_with(PENDING_IMAGE_PREFIX));
    // Bookkeeping only: the fill holds the pending url right after the call.
    assert!(hero_fills(state)
        .iter()
        .any(|fill| matches!(fill, PenFill::Image(body) if body.url.as_str() == pending)));
    assert!(jobs.wait_idle(Duration::from_secs(5)));
    land_into(jobs, state, |state, cmd| state.apply(cmd));
    value
}

#[test]
fn ai_generation_lands_as_the_node_image_fill_after_the_call() {
    let backend = Arc::new(Scripted {
        answer: Some(PNG.into()),
        ..Default::default()
    });
    let jobs = GenerateJobs::new(backend.clone());
    let mut state = state_with_frame(true);
    let value = run(
        &mut state,
        &jobs,
        &[("type", "ai"), ("nodeId", "hero"), ("prompt", "fern macro")],
    );
    assert_eq!(value["status"], "pending");
    let fills = hero_fills(&state);
    assert_eq!(
        fills.len(),
        2,
        "the image stacks on the solid fill: {fills:?}"
    );
    assert!(matches!(&fills[0], PenFill::Solid(_)));
    assert!(matches!(&fills[1], PenFill::Image(body) if body.url.as_str() == PNG));
    let seen = backend.seen.lock().unwrap().clone();
    assert_eq!(seen, vec![("fern macro".into(), Some(1440.0), Some(800.0))]);
}

#[test]
fn stock_generation_uses_the_query() {
    let backend = Arc::new(Scripted {
        answer: Some(PNG.into()),
        ..Default::default()
    });
    let jobs = GenerateJobs::new(backend.clone());
    // Stock needs no image provider.
    let mut state = state_with_frame(false);
    run(
        &mut state,
        &jobs,
        &[
            ("type", "stock"),
            ("nodeId", "hero"),
            ("query", "fern frond"),
        ],
    );
    assert!(matches!(&hero_fills(&state)[1], PenFill::Image(body) if body.url.as_str() == PNG));
    assert_eq!(backend.seen.lock().unwrap()[0].0, "fern frond");
}

#[test]
fn a_failed_generation_drops_the_pending_fill_and_status_explains() {
    let jobs = GenerateJobs::new(Arc::new(Scripted::default()));
    let mut state = state_with_frame(true);
    let value = run(
        &mut state,
        &jobs,
        &[("type", "ai"), ("nodeId", "hero"), ("prompt", "x")],
    );
    let fills = hero_fills(&state);
    assert_eq!(
        fills.len(),
        1,
        "pending fill removed, solid kept: {fills:?}"
    );
    assert!(matches!(&fills[0], PenFill::Solid(_)));
    let status = GenerationStatusTool::for_test(Arc::clone(&jobs));
    let ToolOutcome::OkJson(json) =
        status.call(&args(&[("jobId", value["jobId"].as_str().unwrap())]))
    else {
        panic!("status json");
    };
    let report: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(report["jobs"][0]["status"], "failed");
    assert_eq!(report["jobs"][0]["error"], "quota exceeded");
    assert_eq!(report["running"], false);
}

#[test]
fn without_a_key_ai_generation_writes_nothing_and_says_how_to_configure() {
    let backend = Arc::new(Scripted::default());
    let jobs = GenerateJobs::new(backend.clone());
    let state = state_with_frame(false);
    let tool = GenerateTool::for_test(&state, Arc::clone(&jobs));
    match tool.call(&args(&[
        ("type", "ai"),
        ("nodeId", "hero"),
        ("prompt", "x"),
    ])) {
        ToolOutcome::Err(ToolErrorCode::ToolFailed, message) => {
            assert!(message.contains("Settings > Images"), "{message}")
        }
        other => panic!("expected a configure error, got {other:?}"),
    }
    assert!(jobs.records(None).is_empty(), "no job started");
    assert!(backend.seen.lock().unwrap().is_empty(), "no provider call");
}

#[test]
fn bad_arguments_are_rejected_before_any_job() {
    let jobs = GenerateJobs::new(Arc::new(Scripted::default()));
    let state = state_with_frame(true);
    let tool = GenerateTool::for_test(&state, Arc::clone(&jobs));
    for (call, code) in [
        (
            vec![("nodeId", "hero"), ("prompt", "x")],
            ToolErrorCode::MissingArgument,
        ),
        (
            vec![("type", "video"), ("nodeId", "hero"), ("prompt", "x")],
            ToolErrorCode::InvalidArgument,
        ),
        (
            vec![("type", "ai"), ("prompt", "x")],
            ToolErrorCode::MissingArgument,
        ),
        (
            vec![("type", "ai"), ("nodeId", "hero")],
            ToolErrorCode::MissingArgument,
        ),
        (
            vec![("type", "ai"), ("nodeId", "nope"), ("prompt", "x")],
            ToolErrorCode::InvalidArgument,
        ),
        (
            vec![("type", "ai"), ("nodeId", "label"), ("prompt", "x")],
            ToolErrorCode::InvalidArgument,
        ),
    ] {
        match tool.call(&args(&call)) {
            ToolOutcome::Err(got, _) => assert_eq!(got, code, "{call:?}"),
            other => panic!("{call:?} should fail, got {other:?}"),
        }
    }
    assert!(jobs.records(None).is_empty());
}

#[test]
fn the_wire_path_reports_a_missing_provider_as_a_tool_error() {
    // Through the real registry + global queue: with no key the call fails
    // before any job or provider call, so this never touches the network.
    let mut state = state_with_frame(false);
    let line = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/call",
        "params": {"name": "generate", "arguments": {"type": "ai", "nodeId": "hero", "prompt": "fern"}}
    })
    .to_string();
    let response = super::process_message_with_applier(&mut state, &line, |_, _, _| {
        panic!("nothing may be written without a provider")
    })
    .expect("dispatch")
    .expect("response");
    assert!(response.contains(r#""isError":true"#), "{response}");
    assert!(response.contains("Settings > Agents"), "{response}");
}
