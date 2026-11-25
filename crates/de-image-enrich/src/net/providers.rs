//! Image search: Openverse → two-keyword retry → Wikimedia, thumbnails
//! embedded as `data:` URLs. Used by the desktop image panel's Search
//! popover (`de-host-desktop/src/image_panel_host.rs`) and the MCP
//! `enrich_images` / `generate type:"stock"` paths. Openverse / Wikimedia
//! are product-constant public hosts, so they dial with a plain client.
//!
//! Fetched thumbnails are not re-encoded / down-scaled here (`image_downscale`
//! needs skia, which this crate does not link); the 4 MiB per-image cap still
//! bounds what can be embedded.

mod catalog;
mod download;
pub(crate) mod pacing;
mod relevance;

pub(crate) use catalog::wikimedia_info_is_image;
pub(crate) use catalog::{
    fetch_openverse_json, fetch_openverse_list, fetch_openverse_list_with_aspect,
};
pub use catalog::{
    fetch_openverse_token, parse_openverse_results, parse_wikimedia_results, RawHit,
};
pub use download::{
    fetch_image_and_judge_thumbnail, fetch_image_bytes, fetch_image_data_url,
    fetch_judge_thumbnail, normalize_image_mime_header, read_capped, sniff_image_mime,
};
pub use relevance::simplify_search_query;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use catalog::{fetch_relevant_wikimedia_list, materialize_first_thumb, materialize_thumbs};
use relevance::retain_relevant_hits;
pub(crate) use relevance::{
    retain_relevant_hits as retain_relevant_hits_for_fetch, rewrite_queries, two_keyword_retry,
};

#[cfg(test)]
use catalog::fetch_relevant_wikimedia_list_with;
#[cfg(test)]
use relevance::{
    core_query_words, metadata_is_scene_heavy, query_requests_scene, SCENE_HEAVY_RESULT_WORDS,
};

/// Cap on concurrently running image jobs (search + generate combined).
/// Each job blocks one connection thread for up to minutes of provider
/// network; without a ceiling a page could exhaust the daemon's threads.
const MAX_IN_FLIGHT_IMAGE_JOBS: usize = 4;

static IN_FLIGHT_IMAGE_JOBS: AtomicUsize = AtomicUsize::new(0);

/// RAII slot for one running image job. `acquire` fails once
/// [`MAX_IN_FLIGHT_IMAGE_JOBS`] jobs are running (route answers 429).
pub struct ImageJobSlot(());

impl ImageJobSlot {
    pub fn acquire() -> Option<Self> {
        IN_FLIGHT_IMAGE_JOBS
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
                (n < MAX_IN_FLIGHT_IMAGE_JOBS).then_some(n + 1)
            })
            .ok()
            .map(|_| Self(()))
    }
}

impl Drop for ImageJobSlot {
    fn drop(&mut self) {
        IN_FLIGHT_IMAGE_JOBS.fetch_sub(1, Ordering::AcqRel);
    }
}

/// TS popover requests `count: 5` (desktop parity).
const SEARCH_RESULT_COUNT: usize = 5;
/// Fetch a wider catalogue window before relevance ranking. The public route
/// still materializes at most [`SEARCH_RESULT_COUNT`] thumbnails.
const SEARCH_CANDIDATE_COUNT: usize = 20;
pub const MAX_EMBEDDED_IMAGE_BYTES: usize = 4 * 1024 * 1024;

#[derive(Clone, PartialEq, Eq)]
pub struct WebOpenverseCredentials {
    pub client_id: String,
    pub client_secret: String,
}

impl WebOpenverseCredentials {
    /// `None` unless both parts are non-empty after trimming.
    pub fn from_parts(client_id: &str, client_secret: &str) -> Option<Self> {
        let client_id = client_id.trim();
        let client_secret = client_secret.trim();
        if client_id.is_empty() || client_secret.is_empty() {
            None
        } else {
            Some(Self {
                client_id: client_id.to_string(),
                client_secret: client_secret.to_string(),
            })
        }
    }
}

/// One search hit ready for the desktop popover / MCP.
pub struct WebImageSearchHit {
    pub id: String,
    pub thumb_data_url: String,
    pub attribution: String,
}

pub struct WebImageSearchOutcome {
    pub results: Vec<WebImageSearchHit>,
    /// `"openverse"` / `"wikimedia"`, `None` when nothing landed.
    pub source: Option<&'static str>,
}

/// Run the full search ladder on the calling thread (the caller must NOT
/// hold the state lock).
pub fn run_search_blocking(
    query: &str,
    credentials: Option<&WebOpenverseCredentials>,
) -> WebImageSearchOutcome {
    // A private runtime here would panic the moment this sync helper is
    // reached from a tokio worker; `block_on_anywhere` runs the ladder on the
    // shared (enable_all) runtime instead — same IO/timer drivers, no
    // runtime-in-runtime hazard.
    crate::net::block_on_image_runtime(run_search(query, credentials))
}

/// Run the provider ladder for a single usable thumbnail, bounding the whole
/// async ladder (catalog requests, retries, and thumbnail download together)
/// by `remaining`. This is the MCP enrichment path: unlike the Web UI search,
/// it needs only one image and must return before its outer transport timeout.
pub fn run_first_search_blocking_with_timeout(
    query: &str,
    credentials: Option<&WebOpenverseCredentials>,
    remaining: Duration,
) -> WebImageSearchOutcome {
    run_with_timeout(remaining, run_first_search(query, credentials))
        .unwrap_or_else(empty_search_outcome)
}

/// Judge-aware single-result variant used by headless enrichment callers.
/// The ordinary function above intentionally keeps its existing materializer
/// for the no-judge path and for the image-search popover.
pub fn run_first_search_blocking_with_judge(
    query: &str,
    credentials: Option<&WebOpenverseCredentials>,
    remaining: Duration,
    judge: &dyn crate::net::ImageRelevanceJudge,
    intent: &str,
) -> WebImageSearchOutcome {
    run_with_timeout(
        remaining,
        run_first_search_with_judge(query, credentials, judge, intent),
    )
    .unwrap_or_else(empty_search_outcome)
}

fn run_with_timeout<F>(remaining: Duration, future: F) -> Option<F::Output>
where
    F: std::future::Future,
{
    if remaining.is_zero() {
        return None;
    }
    crate::net::block_on_image_runtime(
        async move { tokio::time::timeout(remaining, future).await.ok() },
    )
}

fn empty_search_outcome() -> WebImageSearchOutcome {
    WebImageSearchOutcome {
        results: Vec::new(),
        source: None,
    }
}

async fn run_search(
    query: &str,
    credentials: Option<&WebOpenverseCredentials>,
) -> WebImageSearchOutcome {
    let Ok(client) = reqwest::Client::builder()
        .use_rustls_tls()
        .timeout(Duration::from_secs(8))
        .user_agent(concat!(
            "design-editor-web-daemon/",
            env!("CARGO_PKG_VERSION")
        ))
        .build()
    else {
        return WebImageSearchOutcome {
            results: Vec::new(),
            source: None,
        };
    };
    run_search_with_fetcher(&client, query, credentials, |url: String| {
        let client = client.clone();
        async move { fetch_image_data_url(&client, &url).await }
    })
    .await
}

/// Single-result variant of [`run_search`]. Provider list lookup keeps the
/// same Openverse → retry → Wikimedia order, but thumbnail materialization
/// stops after the first successful download instead of fetching the whole
/// five-result Web UI page.
async fn run_first_search(
    query: &str,
    credentials: Option<&WebOpenverseCredentials>,
) -> WebImageSearchOutcome {
    let Ok(client) = reqwest::Client::builder()
        .use_rustls_tls()
        .timeout(Duration::from_secs(8))
        .user_agent(concat!(
            "design-editor-web-daemon/",
            env!("CARGO_PKG_VERSION")
        ))
        .build()
    else {
        return empty_search_outcome();
    };
    run_first_search_with_fetcher(&client, query, credentials, |url: String| {
        let client = client.clone();
        async move { fetch_image_data_url(&client, &url).await }
    })
    .await
}

async fn run_first_search_with_judge(
    query: &str,
    credentials: Option<&WebOpenverseCredentials>,
    judge: &dyn crate::net::ImageRelevanceJudge,
    intent: &str,
) -> WebImageSearchOutcome {
    let Ok(client) = reqwest::Client::builder()
        .use_rustls_tls()
        .timeout(Duration::from_secs(8))
        .user_agent(concat!(
            "design-editor-web-daemon/",
            env!("CARGO_PKG_VERSION")
        ))
        .build()
    else {
        return empty_search_outcome();
    };
    let query = simplify_search_query(query);
    let hits = fetch_relevant_openverse_list(&client, &query, credentials)
        .await
        .unwrap_or_default();
    if let Some(result) = materialize_judged_first(&client, hits, &query, intent, judge).await {
        return WebImageSearchOutcome {
            results: vec![result],
            source: Some("openverse"),
        };
    }

    let wiki = fetch_relevant_wikimedia_list(&client, &query).await;
    let Some(result) = materialize_first_thumb(wiki, &|url: String| {
        let client = client.clone();
        async move { fetch_image_data_url(&client, &url).await }
    })
    .await
    else {
        return empty_search_outcome();
    };
    WebImageSearchOutcome {
        results: vec![result],
        source: Some("wikimedia"),
    }
}

/// The full search ladder over a caller-supplied client + thumbnail
/// materializer. Shared by this daemon route (plain embed) and the desktop
/// popover (its own user-agent + skia down-scale pass on each thumbnail).
///
/// `fetch_data_url` downloads one thumbnail URL into a `data:` URL; hits
/// whose thumbnails fail to download are dropped.
pub async fn run_search_with_fetcher<F, Fut>(
    client: &reqwest::Client,
    query: &str,
    credentials: Option<&WebOpenverseCredentials>,
    fetch_data_url: F,
) -> WebImageSearchOutcome
where
    F: Fn(String) -> Fut,
    Fut: std::future::Future<Output = Option<String>>,
{
    // Simplify verbose prompts into keywords (TS simplifySearchQuery).
    let query = simplify_search_query(query);

    // Openverse first; either a zero-result answer or an answer fully removed
    // by the relevance/photo fence retries once with a short concrete subject
    // phrase before falling through to Wikimedia.
    let hits = fetch_relevant_openverse_list(client, &query, credentials).await;
    if let Some(urls) = hits.filter(|h| !h.is_empty()) {
        let results = materialize_thumbs(urls, &fetch_data_url).await;
        if !results.is_empty() {
            return WebImageSearchOutcome {
                results,
                source: Some("openverse"),
            };
        }
    }
    let wiki = fetch_relevant_wikimedia_list(client, &query).await;
    let results = materialize_thumbs(wiki, &fetch_data_url).await;
    let source = (!results.is_empty()).then_some("wikimedia");
    WebImageSearchOutcome { results, source }
}

async fn run_first_search_with_fetcher<F, Fut>(
    client: &reqwest::Client,
    query: &str,
    credentials: Option<&WebOpenverseCredentials>,
    fetch_data_url: F,
) -> WebImageSearchOutcome
where
    F: Fn(String) -> Fut,
    Fut: std::future::Future<Output = Option<String>>,
{
    let query = simplify_search_query(query);

    let hits = fetch_relevant_openverse_list(client, &query, credentials).await;
    if let Some(urls) = hits.filter(|h| !h.is_empty()) {
        if let Some(result) = materialize_first_thumb(urls, &fetch_data_url).await {
            return WebImageSearchOutcome {
                results: vec![result],
                source: Some("openverse"),
            };
        }
    }

    let wiki = fetch_relevant_wikimedia_list(client, &query).await;
    let Some(result) = materialize_first_thumb(wiki, &fetch_data_url).await else {
        return empty_search_outcome();
    };
    WebImageSearchOutcome {
        results: vec![result],
        source: Some("wikimedia"),
    }
}

struct PreparedJudgeHit {
    hit: RawHit,
    thumb_jpeg: Vec<u8>,
    data_url: String,
}

async fn materialize_judged_first(
    client: &reqwest::Client,
    hits: Vec<RawHit>,
    query: &str,
    intent: &str,
    judge: &dyn crate::net::ImageRelevanceJudge,
) -> Option<WebImageSearchHit> {
    const ROUND_SIZE: usize = 5;
    const MAX_JUDGE_CANDIDATES: usize = ROUND_SIZE * 2;
    let mut prepared = Vec::new();
    for hit in hits.into_iter().take(MAX_JUDGE_CANDIDATES) {
        let Some((data_url, thumb_jpeg)) =
            fetch_image_and_judge_thumbnail(client, &hit.thumb_url).await
        else {
            continue;
        };
        prepared.push(PreparedJudgeHit {
            hit,
            thumb_jpeg,
            data_url,
        });
    }
    let batches: Vec<Vec<Vec<u8>>> = prepared
        .chunks(ROUND_SIZE)
        .map(|batch| {
            batch
                .iter()
                .map(|candidate| candidate.thumb_jpeg.clone())
                .collect()
        })
        .collect();
    let selection = crate::net::select_with_judge(judge, query, intent, &batches);
    eprintln!("[ENRICH] {}", crate::net::format_judge_log(&selection));
    let index = selection.ordered_indices().first().copied()?;
    let candidate = prepared.get(index)?;
    Some(WebImageSearchHit {
        id: candidate.hit.id.clone(),
        thumb_data_url: candidate.data_url.clone(),
        attribution: candidate.hit.attribution.clone(),
    })
}

/// Fetch and relevance-filter the Openverse catalogue, retrying at most once
/// with the concrete subject phrase. `None` remains a request-level failure:
/// it falls through to Wikimedia without turning a network error into another
/// Openverse request. `Some([])` and non-empty-but-fully-filtered replies share
/// the same single retry, so the two conditions can never trigger duplicate
/// catalogue requests.
pub(crate) async fn fetch_relevant_openverse_list(
    client: &reqwest::Client,
    query: &str,
    credentials: Option<&WebOpenverseCredentials>,
) -> Option<Vec<RawHit>> {
    fetch_relevant_openverse_list_with(query, |candidate| {
        let client = client.clone();
        let credentials = credentials.cloned();
        async move { fetch_openverse_list(&client, &candidate, credentials.as_ref()).await }
    })
    .await
}

async fn fetch_relevant_openverse_list_with<F, Fut>(
    query: &str,
    mut fetch_list: F,
) -> Option<Vec<RawHit>>
where
    F: FnMut(String) -> Fut,
    Fut: std::future::Future<Output = Option<Vec<RawHit>>>,
{
    let hits = fetch_list(query.to_string()).await?;
    let relevant = retain_relevant_hits(hits, query);
    if !relevant.is_empty() {
        return Some(relevant);
    }

    let Some(retry_query) = two_keyword_retry(query) else {
        return Some(relevant);
    };
    let retry = fetch_list(retry_query).await?;
    // The provider receives a shorter concrete query, but relevance still
    // follows the authored query so photo/studio/isolated intent is not lost.
    Some(retain_relevant_hits(retry, query))
}

#[cfg(test)]
#[path = "web_image_search_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "web_image_search_relevance_tests.rs"]
mod relevance_tests;

#[cfg(test)]
#[path = "web_image_search_retry_tests.rs"]
mod retry_tests;
