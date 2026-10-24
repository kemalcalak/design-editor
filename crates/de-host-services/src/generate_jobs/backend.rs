//! The provider seam behind `generate` jobs.
//!
//! [`GenerateBackend`] is what a job worker calls; tests inject a stub so no
//! test ever reaches the network. [`ProviderBackend`] is production: the same
//! provider dispatch as the browser route (`web_image_generate::run_generate`
//! — OpenAI / Gemini / Replicate / Atlas, result download, endpoint
//! screening), the Openverse → Wikimedia stock ladder, the user's chat model
//! for SVG artwork, and provider image edits for background transforms.
//! Vectorize is local and needs no backend (`vectorize.rs`).

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use de_editor_core::agent_settings::{BuiltinAgentConfig, ImageGenProfile};

use super::image_edit::{edit_image, EditKind};
use super::source::SourceRef;
use super::GenerateJobError;
use crate::web_image_generate::{run_generate, ImageGenerateError, WebImageGenerateRequest};
use crate::web_image_search::{run_first_search_blocking_with_timeout, WebOpenverseCredentials};

/// Wall-clock budget of one stock search.
const STOCK_SEARCH_TIMEOUT: Duration = Duration::from_secs(60);

/// Whether a profile's custom `base_url` must pass the browser route's SSRF
/// screening. Off on the desktop (the user's own saved settings, the
/// popover's trust tier); the `--serve-web` daemon turns it on, because its
/// settings can carry browser-supplied profiles.
static SCREEN_CUSTOM_ENDPOINTS: AtomicBool = AtomicBool::new(false);

/// Screen custom image endpoints in this process from now on.
pub fn screen_custom_endpoints() {
    SCREEN_CUSTOM_ENDPOINTS.store(true, Ordering::Release);
}

fn screening() -> bool {
    SCREEN_CUSTOM_ENDPOINTS.load(Ordering::Acquire)
}

fn has_custom_endpoint(profile: &ImageGenProfile) -> bool {
    profile
        .base_url
        .as_deref()
        .is_some_and(|base| !base.trim().is_empty())
}

/// One AI image request.
#[derive(Clone)]
pub struct ImageJobRequest {
    pub prompt: String,
    /// Target size in document px, used to pick the provider's aspect ratio.
    pub width: Option<f64>,
    pub height: Option<f64>,
    pub profile: ImageGenProfile,
    /// Overall cap on the provider call, for callers with their own deadline
    /// (`enrich_images`); `None` keeps the providers' own timeouts.
    pub timeout: Option<Duration>,
}

/// One stock-photo request.
#[derive(Clone)]
pub struct StockJobRequest {
    pub query: String,
    pub credentials: Option<WebOpenverseCredentials>,
}

/// One SVG drawing request.
#[derive(Clone)]
pub struct SvgJobRequest {
    pub prompt: String,
    /// The target frame's size in document px.
    pub width: f64,
    pub height: f64,
    pub agent: BuiltinAgentConfig,
}

/// One background transform.
#[derive(Clone)]
pub struct EditJobRequest {
    pub source: SourceRef,
    pub kind: EditKind,
    pub profile: ImageGenProfile,
}

/// What a job worker calls. Every method blocks the worker thread. Image
/// methods return a `data:` URL ready to land as an image fill;
/// [`Self::generate_svg`] returns SVG text.
pub trait GenerateBackend: Send + Sync {
    fn generate_image(&self, request: &ImageJobRequest) -> Result<String, GenerateJobError>;
    fn stock_image(&self, request: &StockJobRequest) -> Result<String, GenerateJobError>;
    fn generate_svg(&self, request: &SvgJobRequest) -> Result<String, GenerateJobError>;
    fn edit_image(&self, request: &EditJobRequest) -> Result<String, GenerateJobError>;
}

/// Production backend: real provider calls with the user's own keys.
pub struct ProviderBackend;

impl GenerateBackend for ProviderBackend {
    fn generate_image(&self, request: &ImageJobRequest) -> Result<String, GenerateJobError> {
        let web_request = WebImageGenerateRequest {
            prompt: request.prompt.clone(),
            width: request.width,
            height: request.height,
            profile: request.profile.clone(),
            custom_endpoint: screening() && has_custom_endpoint(&request.profile),
        };
        let url = crate::chat_runtime::block_on_anywhere(async {
            match request.timeout {
                Some(limit) => tokio::time::timeout(limit, run_generate(&web_request))
                    .await
                    .map_err(|_| GenerateJobError::TimedOut)?
                    .map_err(GenerateJobError::from),
                None => run_generate(&web_request)
                    .await
                    .map_err(GenerateJobError::from),
            }
        })?;
        // Inline renders (gpt-image b64, Gemini) can be several MB; shrink an
        // oversized one before it enters the document, as the popover does.
        Ok(de_image_enrich::net::downscale::maybe_downscale_data_url(&url).unwrap_or(url))
    }

    fn stock_image(&self, request: &StockJobRequest) -> Result<String, GenerateJobError> {
        run_first_search_blocking_with_timeout(
            &request.query,
            request.credentials.as_ref(),
            STOCK_SEARCH_TIMEOUT,
        )
        .results
        .into_iter()
        .next()
        .map(|hit| hit.thumb_data_url)
        .ok_or(GenerateJobError::NoStockResult)
    }

    fn generate_svg(&self, request: &SvgJobRequest) -> Result<String, GenerateJobError> {
        super::svg_llm::generate_svg_blocking(
            &request.agent,
            screening(),
            &request.prompt,
            request.width,
            request.height,
        )
    }

    fn edit_image(&self, request: &EditJobRequest) -> Result<String, GenerateJobError> {
        if screening() && has_custom_endpoint(&request.profile) {
            // The edit calls dial their endpoint directly; the web daemon's
            // browser-supplied endpoints only go through the screened
            // generation route.
            return Err(GenerateJobError::Unsupported {
                operation: "a custom image endpoint",
                provider: "web daemon's",
            });
        }
        let source = request.source.load()?;
        let client = provider_client()?;
        crate::chat_runtime::block_on_anywhere(edit_image(
            &client,
            &source,
            &request.kind,
            &request.profile,
        ))
    }
}

/// The plain client for provider hosts named by the user's own settings.
pub(crate) fn provider_client() -> Result<reqwest::Client, GenerateJobError> {
    reqwest::Client::builder()
        .use_rustls_tls()
        .timeout(Duration::from_secs(240))
        .user_agent(concat!("design-editor/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| {
            GenerateJobError::Image(ImageGenerateError::ClientBuild {
                message: e.to_string(),
            })
        })
}

impl From<ImageGenerateError> for GenerateJobError {
    fn from(error: ImageGenerateError) -> Self {
        match error {
            ImageGenerateError::NotConfigured => GenerateJobError::NotConfigured,
            other => GenerateJobError::Image(other),
        }
    }
}
