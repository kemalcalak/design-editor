//! `generate` svg / vectorize-image / remove-background / replace-background
//! tests (#11) — scripted provider backend, real local tracer, no network.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::Engine as _;
use de_editor_core::agent_settings::{
    BuiltinAgentConfig, BuiltinAgentKind, ImageGenProfile, ImageGenProvider, ImageTestStatus,
};
use de_editor_core::agent_settings_builtin_presets::BuiltinAgentPresetKey;
use de_editor_core::{EditorCommand, EditorState, NodeId, PenNodeExt as _};
use de_mcp::{McpTool, ToolErrorCode, ToolOutcome};
use jian_ops_schema::node::PenNode;
use jian_ops_schema::style::PenFill;

use super::generate_tool::GenerateTool;
use crate::generate_jobs::{
    land_into, EditJobRequest, EditKind, GenerateBackend, GenerateJobError, GenerateJobs,
    ImageJobRequest, SourceRef, StockJobRequest, SvgJobRequest,
};

const SVG: &str =
    r##"<svg viewBox="0 0 100 100"><rect width="100" height="100" fill="#ff0000"/></svg>"##;
/// One tool call as argument pairs.
type Call = Vec<(&'static str, &'static str)>;

const EDITED: &str = "data:image/png;base64,RURJVEVE";

#[derive(Default)]
struct Scripted {
    svg: Option<String>,
    edits: Mutex<Vec<(SourceRef, EditKind)>>,
    prompts: Mutex<Vec<String>>,
}

impl GenerateBackend for Scripted {
    fn generate_image(&self, _: &ImageJobRequest) -> Result<String, GenerateJobError> {
        Err(GenerateJobError::NotConfigured)
    }

    fn stock_image(&self, _: &StockJobRequest) -> Result<String, GenerateJobError> {
        Err(GenerateJobError::NoStockResult)
    }

    fn generate_svg(&self, request: &SvgJobRequest) -> Result<String, GenerateJobError> {
        self.prompts.lock().unwrap().push(request.prompt.clone());
        self.svg.clone().ok_or(GenerateJobError::NoSvg)
    }

    fn edit_image(&self, request: &EditJobRequest) -> Result<String, GenerateJobError> {
        self.edits
            .lock()
            .unwrap()
            .push((request.source.clone(), request.kind.clone()));
        Ok(EDITED.to_string())
    }
}

/// A `width` x `height` PNG, left half red, right half blue.
fn two_colour_png(width: u32, height: u32) -> String {
    let image = image::RgbaImage::from_fn(width, height, |x, _| {
        if x < width / 2 {
            image::Rgba([220, 30, 30, 255])
        } else {
            image::Rgba([30, 30, 220, 255])
        }
    });
    let mut bytes = Vec::new();
    image
        .write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
        .unwrap();
    format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    )
}

fn state(photo_url: &str, configured: bool) -> EditorState {
    let source = format!(
        r##"{{"version":"1.0","children":[
            {{"type":"frame","id":"logo","width":400,"height":200}},
            {{"type":"rectangle","id":"photo","width":300,"height":300,
              "fill":[{{"type":"solid","color":"#ffffff"}},{{"type":"image","url":"{photo_url}"}}]}},
            {{"type":"rectangle","id":"card","width":120,"height":120}},
            {{"type":"text","id":"label","content":"Hi"}}
        ]}}"##
    );
    let mut state =
        crate::doc_io::load_editor_state_from_source(&source, de_editor_core::Locale::EnUs)
            .expect("fixture");
    if configured {
        let settings = &mut state.editor_ui.agent_settings;
        settings.builtin_agents.push(BuiltinAgentConfig {
            id: "chat".into(),
            preset: BuiltinAgentPresetKey::Anthropic,
            display_name: "Chat".into(),
            kind: BuiltinAgentKind::Anthropic,
            api_key: "sk-ant-test".into(),
            models: vec!["claude-test".into()],
            base_url: String::new(),
            enabled: true,
        });
        settings.image_gen_profiles.push(ImageGenProfile {
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

fn node<'a>(state: &'a EditorState, id: &str) -> &'a PenNode {
    de_editor_core::walkers::find_node(state.active_children(), &NodeId::new(id)).unwrap()
}

/// Call, apply the call's own write (if any), wait, land.
fn run(
    state: &mut EditorState,
    jobs: &Arc<GenerateJobs>,
    call: &[(&str, &str)],
) -> serde_json::Value {
    let tool = GenerateTool::for_test(state, Arc::clone(jobs));
    let json = match tool.call(&args(call)) {
        ToolOutcome::OkJsonWithCommand(json, command) => {
            assert!(state.apply(command), "the call's write applies");
            json
        }
        ToolOutcome::OkJson(json) => json,
        other => panic!("{call:?} failed: {other:?}"),
    };
    assert!(jobs.wait_idle(Duration::from_secs(30)));
    land_into(jobs, state, |state, cmd| state.apply(cmd));
    serde_json::from_str(&json).unwrap()
}

fn frame_children(state: &EditorState, id: &str) -> usize {
    node(state, id).children().map_or(0, Vec::len)
}

fn frame_placeholder(state: &EditorState, id: &str) -> Option<bool> {
    match node(state, id) {
        PenNode::Frame(frame) => frame.placeholder,
        other => panic!("not a frame: {other:?}"),
    }
}

#[test]
fn svg_artwork_lands_as_frame_children_scaled_to_the_frame() {
    let backend = Arc::new(Scripted {
        svg: Some(SVG.into()),
        ..Default::default()
    });
    let jobs = GenerateJobs::new(backend.clone());
    let mut state = state("data:image/png;base64,AA==", true);
    let tool = GenerateTool::for_test(&state, Arc::clone(&jobs));
    let ToolOutcome::OkJsonWithCommand(_, command) = tool.call(&args(&[
        ("type", "svg"),
        ("nodeId", "logo"),
        ("prompt", "a fox mark"),
    ])) else {
        panic!("svg returns the placeholder write");
    };
    assert!(state.apply(command));
    assert_eq!(
        frame_placeholder(&state, "logo"),
        Some(true),
        "pending frame"
    );
    assert!(jobs.wait_idle(Duration::from_secs(5)));
    assert!(land_into(&jobs, &mut state, |state, cmd| state.apply(cmd)));
    assert_eq!(frame_placeholder(&state, "logo"), None, "flag cleared");
    let kids = node(&state, "logo").children().unwrap();
    assert_eq!(kids.len(), 1);
    // A 100-unit square into 400x200: 200 px, centred.
    assert_eq!(kids[0].width_px(), Some(200.0));
    assert_eq!(kids[0].base().x, Some(100.0));
    assert_eq!(backend.prompts.lock().unwrap().as_slice(), ["a fox mark"]);
}

#[test]
fn a_failed_svg_only_clears_the_placeholder() {
    let jobs = GenerateJobs::new(Arc::new(Scripted::default()));
    let mut state = state("data:image/png;base64,AA==", true);
    run(
        &mut state,
        &jobs,
        &[("type", "svg"), ("nodeId", "logo"), ("prompt", "x")],
    );
    assert_eq!(frame_placeholder(&state, "logo"), None);
    assert_eq!(frame_children(&state, "logo"), 0);
    let record = &jobs.records(None)[0];
    assert_eq!(
        record.status,
        crate::generate_jobs::JobStatus::Failed(GenerateJobError::NoSvg.to_string())
    );
}

#[test]
fn vectorize_traces_the_source_locally_into_the_frame() {
    let photo = two_colour_png(300, 300);
    let jobs = GenerateJobs::new(Arc::new(Scripted::default()));
    // No provider needed: tracing is local.
    let mut state = state(&photo, false);
    run(
        &mut state,
        &jobs,
        &[
            ("type", "vectorize-image"),
            ("nodeId", "logo"),
            ("sourceNodeId", "photo"),
        ],
    );
    assert_eq!(frame_placeholder(&state, "logo"), None);
    assert!(
        frame_children(&state, "logo") >= 2,
        "both colour regions traced: {:?}",
        node(&state, "logo").children()
    );
}

#[test]
fn sources_are_checked_at_call_time() {
    let jobs = GenerateJobs::new(Arc::new(Scripted::default()));
    let small = two_colour_png(64, 64);
    let state = state(&small, true);
    let tool = GenerateTool::for_test(&state, Arc::clone(&jobs));
    let cases: Vec<(Call, ToolErrorCode, &str)> = vec![
        (
            vec![
                ("type", "vectorize-image"),
                ("nodeId", "logo"),
                ("sourceNodeId", "photo"),
            ],
            ToolErrorCode::ToolFailed,
            "outside 256 to 4096 px",
        ),
        (
            vec![
                ("type", "remove-background"),
                ("imageUrl", "data:image/png;base64,Zm9v"),
            ],
            ToolErrorCode::InvalidArgument,
            "not in the document",
        ),
        (
            vec![("type", "remove-background"), ("sourceNodeId", "card")],
            ToolErrorCode::InvalidArgument,
            "shows no image",
        ),
        (
            vec![("type", "remove-background")],
            ToolErrorCode::MissingArgument,
            "imageUrl or sourceNodeId",
        ),
        (
            vec![("type", "svg"), ("nodeId", "photo"), ("prompt", "x")],
            ToolErrorCode::InvalidArgument,
            "not a frame",
        ),
    ];
    for (call, code, needle) in cases {
        match tool.call(&args(&call)) {
            ToolOutcome::Err(got, message) => {
                assert_eq!(got, code, "{call:?}: {message}");
                assert!(message.contains(needle), "{call:?}: {message}");
            }
            other => panic!("{call:?} should fail, got {other:?}"),
        }
    }
    assert!(jobs.records(None).is_empty(), "no job started");
}

#[test]
fn svg_needs_a_chat_provider_and_transforms_an_image_provider() {
    let photo = two_colour_png(300, 300);
    let jobs = GenerateJobs::new(Arc::new(Scripted::default()));
    let state = state(&photo, false);
    let tool = GenerateTool::for_test(&state, Arc::clone(&jobs));
    for (call, needle) in [
        (
            vec![("type", "svg"), ("nodeId", "logo"), ("prompt", "x")],
            "no chat provider",
        ),
        (
            vec![("type", "remove-background"), ("sourceNodeId", "photo")],
            "no image provider",
        ),
    ] {
        match tool.call(&args(&call)) {
            ToolOutcome::Err(ToolErrorCode::ToolFailed, message) => {
                assert!(message.contains(needle), "{message}")
            }
            other => panic!("{call:?}: {other:?}"),
        }
    }
    assert!(jobs.records(None).is_empty());
}

#[test]
fn remove_background_on_a_node_replaces_its_source_fill_in_place() {
    let photo = two_colour_png(300, 300);
    let backend = Arc::new(Scripted::default());
    let jobs = GenerateJobs::new(backend.clone());
    let mut state = state(&photo, true);
    run(
        &mut state,
        &jobs,
        &[
            ("type", "remove-background"),
            ("sourceNodeId", "photo"),
            ("nodeId", "photo"),
        ],
    );
    let PenNode::Rectangle(rect) = node(&state, "photo") else {
        panic!("photo rectangle");
    };
    let fills = rect.container.fill.clone().unwrap();
    assert!(matches!(&fills[0], PenFill::Solid(_)), "other fills stay");
    assert!(matches!(&fills[1], PenFill::Image(body) if body.url.as_str() == EDITED));
    let edits = backend.edits.lock().unwrap();
    assert_eq!(edits[0].0, SourceRef::Data(photo));
    assert_eq!(edits[0].1, EditKind::RemoveBackground);
}

#[test]
fn replace_background_returns_a_url_that_lands_where_the_client_puts_it() {
    let photo = two_colour_png(300, 300);
    let backend = Arc::new(Scripted::default());
    let jobs = GenerateJobs::new(backend.clone());
    let mut state = state(&photo, true);
    let tool = GenerateTool::for_test(&state, Arc::clone(&jobs));
    // pen.dev's shape: imageUrl in, url out, no node touched by the call.
    let ToolOutcome::OkJson(json) = tool.call(&args(&[
        ("type", "replace-background"),
        ("imageUrl", &photo),
        ("prompt", "wet slate slab"),
    ])) else {
        panic!("replace-background returns only a url");
    };
    let url = serde_json::from_str::<serde_json::Value>(&json).unwrap()["url"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(jobs.wait_idle(Duration::from_secs(5)));
    // The client applies the url after the job finished.
    let patch = serde_json::json!({ "fill": [{ "type": "image", "url": url }] });
    assert!(state.apply(EditorCommand::PatchNodeData {
        node_id: NodeId::new("card"),
        patch_json: patch.to_string(),
        page_id: None,
    }));
    std::thread::sleep(Duration::from_millis(600));
    assert!(land_into(&jobs, &mut state, |state, cmd| state.apply(cmd)));
    let PenNode::Rectangle(card) = node(&state, "card") else {
        panic!("card");
    };
    assert!(matches!(
        &card.container.fill.as_ref().unwrap()[0],
        PenFill::Image(body) if body.url.as_str() == EDITED
    ));
    assert_eq!(
        backend.edits.lock().unwrap()[0].1,
        EditKind::ReplaceBackground("wet slate slab".into())
    );
}
