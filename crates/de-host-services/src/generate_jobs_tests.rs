//! `generate_jobs` tests — a scripted backend, no network.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use de_editor_core::{EditorState, NodeId};
use jian_ops_schema::node::PenNode;
use jian_ops_schema::style::PenFill;

use super::*;

/// Answers every request from a script; `None` = fail.
pub(crate) struct ScriptedBackend {
    pub(crate) image: Mutex<Option<String>>,
    pub(crate) prompts: Mutex<Vec<String>>,
    pub(crate) delay: Duration,
}

impl ScriptedBackend {
    pub(crate) fn answering(url: Option<&str>) -> Arc<Self> {
        Arc::new(Self {
            image: Mutex::new(url.map(str::to_string)),
            prompts: Mutex::new(Vec::new()),
            delay: Duration::ZERO,
        })
    }
}

impl GenerateBackend for ScriptedBackend {
    fn generate_image(&self, request: &ImageJobRequest) -> Result<String, GenerateJobError> {
        std::thread::sleep(self.delay);
        self.prompts.lock().unwrap().push(request.prompt.clone());
        self.image.lock().unwrap().clone().ok_or_else(|| {
            GenerateJobError::Image(crate::web_image_generate::ImageGenerateError::Provider(
                "provider said no".into(),
            ))
        })
    }

    fn stock_image(&self, request: &StockJobRequest) -> Result<String, GenerateJobError> {
        self.prompts.lock().unwrap().push(request.query.clone());
        self.image
            .lock()
            .unwrap()
            .clone()
            .ok_or(GenerateJobError::NoStockResult)
    }
}

const PNG: &str = "data:image/png;base64,iVBORw0KGgo=";

fn doc_with_pending(pending: &str) -> EditorState {
    let source = format!(
        r##"{{"version":"1.0","pages":[
            {{"id":"p1","name":"One","children":[{{"type":"frame","id":"other","width":10,"height":10}}]}},
            {{"id":"p2","name":"Two","children":[
                {{"type":"frame","id":"hero","width":400,"height":200,
                  "fill":[{{"type":"solid","color":"#ff0000"}},{{"type":"image","url":"{pending}","mode":"fit"}}]}},
                {{"type":"image","id":"pic","src":"{pending}","width":50,"height":50}}
            ]}}]}}"##
    );
    crate::doc_io::load_editor_state_from_source(&source, de_editor_core::Locale::EnUs)
        .expect("fixture")
}

fn image_request() -> JobWork {
    JobWork::Image(ImageJobRequest {
        prompt: "a fern".into(),
        width: Some(400.0),
        height: Some(200.0),
        profile: de_editor_core::agent_settings::ImageGenProfile {
            id: "p".into(),
            name: "p".into(),
            provider: de_editor_core::agent_settings::ImageGenProvider::OpenAi,
            api_key: "sk-test".into(),
            model: "gpt-image-1".into(),
            base_url: None,
            test_status: de_editor_core::agent_settings::ImageTestStatus::Idle,
        },
        timeout: None,
    })
}

fn find<'a>(state: &'a EditorState, page: usize, id: &str) -> &'a PenNode {
    let pages = state.doc.pages.as_ref().unwrap();
    de_editor_core::walkers::find_node(&pages[page].children, &NodeId::new(id)).unwrap()
}

fn fills(node: &PenNode) -> Vec<PenFill> {
    de_editor_core::fills::node_fills(node)
        .cloned()
        .unwrap_or_default()
}

#[test]
fn a_finished_job_rewrites_its_pending_url_on_every_page_and_node() {
    let jobs = GenerateJobs::new(ScriptedBackend::answering(Some(PNG)));
    let ticket = jobs.reserve(GenerateKind::Ai, Some("hero")).unwrap();
    assert!(ticket.pending_url.starts_with(PENDING_IMAGE_PREFIX));
    let mut state = doc_with_pending(&ticket.pending_url);
    jobs.spawn(ticket, image_request());
    assert!(jobs.wait_idle(Duration::from_secs(5)));
    let landed = land_into(&jobs, &mut state, |state, cmd| state.apply(cmd));
    assert!(landed);
    let hero = fills(find(&state, 1, "hero"));
    assert!(matches!(&hero[0], PenFill::Solid(_)), "other fills stay");
    match &hero[1] {
        PenFill::Image(body) => {
            assert_eq!(body.url.as_str(), PNG);
            assert!(body.mode.is_some(), "the authored fit mode survives");
        }
        other => panic!("expected image fill, got {other:?}"),
    }
    match find(&state, 1, "pic") {
        PenNode::Image(image) => assert_eq!(image.src.as_str(), PNG),
        other => panic!("expected image node, got {other:?}"),
    }
    assert_eq!(jobs.records(None)[0].status, JobStatus::Done);
    assert!(!jobs.has_landings(), "drained");
}

#[test]
fn a_failed_job_removes_the_pending_fill_and_reports_why() {
    let jobs = GenerateJobs::new(ScriptedBackend::answering(None));
    let ticket = jobs.reserve(GenerateKind::Ai, Some("hero")).unwrap();
    let mut state = doc_with_pending(&ticket.pending_url);
    let id = ticket.id.clone();
    jobs.spawn(ticket, image_request());
    assert!(jobs.wait_idle(Duration::from_secs(5)));
    assert!(land_into(&jobs, &mut state, |state, cmd| state.apply(cmd)));
    let hero = fills(find(&state, 1, "hero"));
    assert_eq!(hero.len(), 1, "only the solid fill is left: {hero:?}");
    match find(&state, 1, "pic") {
        PenNode::Image(image) => {
            assert_eq!(
                image.src.as_str(),
                de_image_enrich::SEARCH_FAILED_PLACEHOLDER_SRC
            )
        }
        other => panic!("expected image node, got {other:?}"),
    }
    assert_eq!(
        jobs.records(Some(&id))[0].status,
        JobStatus::Failed("provider said no".into())
    );
}

#[test]
fn a_result_whose_url_is_not_placed_yet_waits_for_it() {
    let jobs = GenerateJobs::new(ScriptedBackend::answering(Some(PNG)));
    let ticket = jobs.reserve(GenerateKind::Ai, None).unwrap();
    let pending = ticket.pending_url.clone();
    jobs.spawn(ticket, image_request());
    assert!(jobs.wait_idle(Duration::from_secs(5)));
    let mut empty = doc_with_pending("data:image/png;base64,AA==");
    assert!(!land_into(&jobs, &mut empty, |state, cmd| state.apply(cmd)));
    // The client applies the pending url after the job already finished.
    let mut state = doc_with_pending(&pending);
    std::thread::sleep(UNPLACED_RETRY_GAP + Duration::from_millis(50));
    assert!(land_into(&jobs, &mut state, |state, cmd| state.apply(cmd)));
    match find(&state, 1, "pic") {
        PenNode::Image(image) => assert_eq!(image.src.as_str(), PNG),
        other => panic!("expected image node, got {other:?}"),
    }
}

#[test]
fn reserve_refuses_past_the_running_limit() {
    let backend = Arc::new(ScriptedBackend {
        image: Mutex::new(Some(PNG.into())),
        prompts: Mutex::new(Vec::new()),
        delay: Duration::from_millis(300),
    });
    let jobs = GenerateJobs::new(backend);
    for _ in 0..MAX_RUNNING_JOBS {
        let ticket = jobs.reserve(GenerateKind::Ai, None).unwrap();
        jobs.spawn(ticket, image_request());
    }
    assert_eq!(
        jobs.reserve(GenerateKind::Ai, None),
        Err(GenerateJobError::Busy {
            limit: MAX_RUNNING_JOBS
        })
    );
    assert!(jobs.wait_idle(Duration::from_secs(10)));
    assert!(jobs.reserve(GenerateKind::Ai, None).is_ok());
}

#[test]
fn the_listener_hears_every_finished_job() {
    let jobs = GenerateJobs::new(ScriptedBackend::answering(Some(PNG)));
    let heard = Arc::new(Mutex::new(0usize));
    let counter = Arc::clone(&heard);
    jobs.set_listener(Some(Arc::new(move || *counter.lock().unwrap() += 1)));
    let ticket = jobs.reserve(GenerateKind::Stock, None).unwrap();
    jobs.spawn(
        ticket,
        JobWork::Stock(StockJobRequest {
            query: "fern".into(),
            credentials: None,
        }),
    );
    assert!(jobs.wait_idle(Duration::from_secs(5)));
    // `finish` notifies the waiter before calling the listener.
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(*heard.lock().unwrap(), 1);
}

#[test]
fn gpt_image_models_get_their_own_request_body() {
    let body = crate::web_image_generate::openai_generation_body(
        "gpt-image-1",
        "x",
        Some(800.0),
        Some(400.0),
    );
    assert_eq!(body["size"], "1536x1024");
    assert!(body.get("response_format").is_none());
    let body = crate::web_image_generate::openai_generation_body(
        "dall-e-3",
        "x",
        Some(400.0),
        Some(800.0),
    );
    assert_eq!(body["size"], "1024x1792");
    assert_eq!(body["response_format"], "url");
}

struct PanickingBackend;

impl GenerateBackend for PanickingBackend {
    fn generate_image(&self, _request: &ImageJobRequest) -> Result<String, GenerateJobError> {
        panic!("provider bug");
    }

    fn stock_image(&self, _request: &StockJobRequest) -> Result<String, GenerateJobError> {
        panic!("provider bug");
    }
}

#[test]
fn a_panicking_provider_releases_its_slot_and_fails_the_job() {
    let jobs = GenerateJobs::new(Arc::new(PanickingBackend));
    let ticket = jobs.reserve(GenerateKind::Ai, None).unwrap();
    let id = ticket.id.clone();
    jobs.spawn(ticket, image_request());
    assert!(jobs.wait_idle(Duration::from_secs(5)), "slot released");
    assert_eq!(
        jobs.records(Some(&id))[0].status,
        JobStatus::Failed(GenerateJobError::Panicked.to_string())
    );
}

#[test]
fn generate_kind_parses_its_own_names_only() {
    for kind in GenerateKind::ALL {
        assert_eq!(GenerateKind::parse(kind.as_str()), Some(kind));
    }
    assert_eq!(GenerateKind::parse("video"), None);
}
