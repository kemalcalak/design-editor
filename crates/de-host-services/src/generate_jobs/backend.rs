//! The provider seam behind `generate` jobs.
//!
//! [`GenerateBackend`] is what a job worker calls; tests inject a stub so no
//! test ever reaches the network. [`ProviderBackend`] is production: the
//! shared OpenAI / Gemini / Replicate / Atlas calls of
//! `web_image_generate`, and the Openverse → Wikimedia stock ladder.

use std::time::Duration;

use de_editor_core::agent_settings::{ImageGenProfile, ImageGenProvider};

use super::GenerateJobError;
use crate::web_image_generate::{
    generate_atlas, generate_gemini, generate_openai, generate_replicate, ImageGenerateError,
};
use crate::web_image_search::{
    fetch_image_data_url, run_first_search_blocking_with_timeout, WebOpenverseCredentials,
};

/// Wall-clock budget of one stock search.
const STOCK_SEARCH_TIMEOUT: Duration = Duration::from_secs(60);

/// One AI image request.
#[derive(Clone)]
pub struct ImageJobRequest {
    pub prompt: String,
    /// Target size in document px, used to pick the provider's aspect ratio.
    pub width: Option<f64>,
    pub height: Option<f64>,
    pub profile: ImageGenProfile,
}

/// One stock-photo request.
#[derive(Clone)]
pub struct StockJobRequest {
    pub query: String,
    pub credentials: Option<WebOpenverseCredentials>,
}

/// What a job worker calls. Every method blocks the worker thread and returns
/// a `data:` URL ready to land as an image fill.
pub trait GenerateBackend: Send + Sync {
    fn generate_image(&self, request: &ImageJobRequest) -> Result<String, GenerateJobError>;
    fn stock_image(&self, request: &StockJobRequest) -> Result<String, GenerateJobError>;
}

/// Production backend: real provider calls with the user's own keys.
pub struct ProviderBackend;

impl GenerateBackend for ProviderBackend {
    fn generate_image(&self, request: &ImageJobRequest) -> Result<String, GenerateJobError> {
        let url = crate::chat_runtime::block_on_anywhere(generate(request))?;
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
}

async fn generate(request: &ImageJobRequest) -> Result<String, ImageGenerateError> {
    let client = provider_client()?;
    let profile = &request.profile;
    let (prompt, width, height) = (&request.prompt, request.width, request.height);
    let url = match profile.provider {
        ImageGenProvider::OpenAi | ImageGenProvider::Custom => {
            generate_openai(&client, prompt, profile, width, height).await?
        }
        ImageGenProvider::Gemini => {
            generate_gemini(&client, prompt, profile, width, height).await?
        }
        ImageGenProvider::Replicate => {
            generate_replicate(&client, prompt, profile, width, height).await?
        }
        ImageGenProvider::Atlas => generate_atlas(&client, prompt, profile, width, height).await?,
    };
    if url.starts_with("data:") {
        return Ok(url);
    }
    // Remote result: embed it so the fill renders offline and saves into the
    // document's asset folder like any other image.
    fetch_image_data_url(&client, &url)
        .await
        .ok_or(ImageGenerateError::DownloadFailed)
}

/// The user's saved settings are operator-owned (the desktop's own trust
/// tier), so the provider host dials with a plain client.
pub(crate) fn provider_client() -> Result<reqwest::Client, ImageGenerateError> {
    reqwest::Client::builder()
        .use_rustls_tls()
        .timeout(Duration::from_secs(180))
        .user_agent(concat!("design-editor/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| ImageGenerateError::ClientBuild {
            message: e.to_string(),
        })
}
