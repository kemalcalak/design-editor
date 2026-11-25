//! Image generation against the user's provider (OpenAI /
//! OpenAI-compatible, Gemini inline-image, Replicate / Atlas prediction
//! polling), used by the MCP `generate` jobs (`generate_jobs`) and the
//! desktop Generate popover (`de-host-desktop/src/image_generate_host.rs`).
//!
//! The profile is the user's own saved setting, so its endpoint (default or
//! custom `base_url`) dials with a plain client, the desktop popover's
//! trust tier.

use std::time::Duration;

use de_editor_core::agent_settings::{ImageGenProfile, ImageGenProvider};

use crate::web_image_search::fetch_image_data_url;

#[path = "web_image_generate_error.rs"]
mod error;
pub use error::ImageGenerateError;

#[path = "web_image_generate_atlas.rs"]
mod atlas;
pub use atlas::generate_atlas;

/// Truncation cap for surfaced provider errors (TS parity).
const ERROR_MESSAGE_CAP: usize = 200;

/// Cap for a provider RESPONSE body. Gemini legitimately inlines the image
/// as base64 (a 4 MiB image is ~5.5 MiB of JSON), so this sits well above
/// that — but it still bounds what a (browser-configured) custom endpoint
/// can stream into daemon memory.
const MAX_PROVIDER_BODY_BYTES: usize = 16 * 1024 * 1024;

/// Read a provider response's status + body under
/// [`MAX_PROVIDER_BODY_BYTES`] (streaming abort, not read-then-check).
pub(crate) async fn read_provider_body(
    provider: &'static str,
    resp: reqwest::Response,
) -> Result<(reqwest::StatusCode, String), ImageGenerateError> {
    let status = resp.status();
    let bytes = crate::web_image_search::read_capped(resp, MAX_PROVIDER_BODY_BYTES)
        .await
        .ok_or(ImageGenerateError::BodyTooLarge { provider })?;
    Ok((status, String::from_utf8_lossy(&bytes).into_owned()))
}

pub(crate) struct WebImageGenerateRequest {
    pub(crate) prompt: String,
    pub(crate) width: Option<f64>,
    pub(crate) height: Option<f64>,
    pub(crate) profile: ImageGenProfile,
}

pub(crate) async fn run_generate(
    request: &WebImageGenerateRequest,
) -> Result<String, ImageGenerateError> {
    let profile = &request.profile;
    let client = generate_client()?;
    let url = match profile.provider {
        ImageGenProvider::OpenAi | ImageGenProvider::Custom => {
            generate_openai(
                &client,
                &request.prompt,
                profile,
                request.width,
                request.height,
            )
            .await?
        }
        ImageGenProvider::Gemini => {
            generate_gemini(
                &client,
                &request.prompt,
                profile,
                request.width,
                request.height,
            )
            .await?
        }
        ImageGenProvider::Replicate => {
            generate_replicate(
                &client,
                &request.prompt,
                profile,
                request.width,
                request.height,
            )
            .await?
        }
        ImageGenProvider::Atlas => {
            generate_atlas(
                &client,
                &request.prompt,
                profile,
                request.width,
                request.height,
            )
            .await?
        }
    };
    if url.starts_with("data:") {
        // Inline base64 (Gemini / OpenAI b64_json). Unlike the desktop, no
        // down-scale pass here — image_downscale needs skia and this crate
        // stays GL-free.
        return Ok(url);
    }
    // Remote URL → embed as a data URL so the preview paints and the applied
    // src stays renderable offline.
    fetch_image_data_url(&client, &url)
        .await
        .ok_or(ImageGenerateError::DownloadFailed)
}

fn generate_client() -> Result<reqwest::Client, ImageGenerateError> {
    reqwest::Client::builder()
        .use_rustls_tls()
        .timeout(Duration::from_secs(120))
        .user_agent(concat!(
            "design-editor-web-daemon/",
            env!("CARGO_PKG_VERSION")
        ))
        .build()
        .map_err(|e| ImageGenerateError::ClientBuild {
            message: e.to_string(),
        })
}

/// TS `mapToOpenAISize`.
fn openai_size(width: Option<f64>, height: Option<f64>) -> &'static str {
    let (Some(w), Some(h)) = (width, height) else {
        return "1024x1024";
    };
    let ratio = w / h;
    if ratio > 1.3 {
        "1792x1024"
    } else if ratio < 0.77 {
        "1024x1792"
    } else {
        "1024x1024"
    }
}

/// Whether `model` is one of OpenAI's GPT image models, which always answer
/// with base64, reject `response_format`, and take their own size set.
pub(crate) fn is_gpt_image_model(model: &str) -> bool {
    model.trim().to_ascii_lowercase().starts_with("gpt-image")
}

/// `images/generations` body. DALL-E keeps the TS shape (`url` responses,
/// 1792 px sizes); GPT image models get their own sizes and no
/// `response_format`.
pub(crate) fn openai_generation_body(
    model: &str,
    prompt: &str,
    width: Option<f64>,
    height: Option<f64>,
) -> serde_json::Value {
    if is_gpt_image_model(model) {
        return serde_json::json!({
            "model": model,
            "prompt": prompt,
            "n": 1,
            "size": gpt_image_size(width, height),
        });
    }
    serde_json::json!({
        "model": model,
        "prompt": prompt,
        "n": 1,
        "size": openai_size(width, height),
        "response_format": "url",
    })
}

/// GPT image models' size set (landscape / portrait / square).
pub(crate) fn gpt_image_size(width: Option<f64>, height: Option<f64>) -> &'static str {
    match openai_size(width, height) {
        "1792x1024" => "1536x1024",
        "1024x1792" => "1024x1536",
        _ => "1024x1024",
    }
}

/// TS `mapToGeminiAspectRatio`.
fn gemini_aspect_ratio(width: Option<f64>, height: Option<f64>) -> Option<&'static str> {
    let (Some(w), Some(h)) = (width, height) else {
        return None;
    };
    let ratio = w / h;
    Some(if ratio > 1.6 {
        "16:9"
    } else if ratio > 1.3 {
        "4:3"
    } else if ratio < 0.625 {
        "9:16"
    } else if ratio < 0.77 {
        "3:4"
    } else {
        "1:1"
    })
}

pub(crate) fn provider_error(provider: &str, status: reqwest::StatusCode, body: &str) -> String {
    // TS: prefer the provider's error.message, else status + slice.
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(body) {
        if let Some(message) = json
            .get("error")
            .and_then(|e| e.get("message"))
            .and_then(serde_json::Value::as_str)
            .or_else(|| json.get("detail").and_then(serde_json::Value::as_str))
            .or_else(|| json.get("message").and_then(serde_json::Value::as_str))
        {
            return message.chars().take(ERROR_MESSAGE_CAP).collect();
        }
    }
    let mut msg = format!("{provider} returned {}", status.as_u16());
    if !body.is_empty() {
        msg.push_str(": ");
        msg.push_str(&body.chars().take(150).collect::<String>());
    }
    msg
}

/// OpenAI / OpenAI-compatible `images/generations` request. Shared verbatim
/// by the desktop Generate popover (its own client/UA) and the MCP jobs.
pub async fn generate_openai(
    client: &reqwest::Client,
    prompt: &str,
    profile: &ImageGenProfile,
    width: Option<f64>,
    height: Option<f64>,
) -> Result<String, ImageGenerateError> {
    let base = profile
        .base_url
        .as_deref()
        .filter(|b| !b.trim().is_empty())
        .unwrap_or("https://api.openai.com");
    let endpoint = format!("{}/v1/images/generations", base.trim_end_matches('/'));
    let resp = client
        .post(endpoint)
        .bearer_auth(profile.api_key.trim())
        .json(&openai_generation_body(
            &profile.model,
            prompt,
            width,
            height,
        ))
        .send()
        .await
        .map_err(|e| ImageGenerateError::Request {
            provider: "OpenAI",
            message: e.to_string(),
        })?;
    let (status, body) = read_provider_body("OpenAI", resp).await?;
    if !status.is_success() {
        return Err(ImageGenerateError::Provider(provider_error(
            "OpenAI", status, &body,
        )));
    }
    let json: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| ImageGenerateError::ResponseParse {
            provider: "OpenAI",
            message: e.to_string(),
        })?;
    json.get("data")
        .and_then(|d| d.as_array())
        .and_then(|d| d.first())
        .and_then(|d| {
            d.get("url")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
                .or_else(|| {
                    // Some OpenAI-compatible providers return b64_json.
                    d.get("b64_json")
                        .and_then(serde_json::Value::as_str)
                        .map(|b64| format!("data:image/png;base64,{b64}"))
                })
        })
        .ok_or(ImageGenerateError::MissingImageUrl)
}

/// Gemini `generateContent` inline-image request (shared desktop + web).
pub async fn generate_gemini(
    client: &reqwest::Client,
    prompt: &str,
    profile: &ImageGenProfile,
    width: Option<f64>,
    height: Option<f64>,
) -> Result<String, ImageGenerateError> {
    let base = profile
        .base_url
        .as_deref()
        .filter(|b| !b.trim().is_empty())
        .unwrap_or("https://generativelanguage.googleapis.com");
    let endpoint = format!(
        "{}/v1beta/models/{}:generateContent?key={}",
        base.trim_end_matches('/'),
        profile.model,
        profile.api_key.trim()
    );
    let mut generation_config = serde_json::json!({
        "responseModalities": ["TEXT", "IMAGE"],
    });
    if let Some(aspect) = gemini_aspect_ratio(width, height) {
        generation_config["imageConfig"] = serde_json::json!({ "aspectRatio": aspect });
    }
    let resp = client
        .post(endpoint)
        .json(&serde_json::json!({
            "contents": [{ "parts": [{ "text": prompt }] }],
            "generationConfig": generation_config,
        }))
        .send()
        .await
        // `without_url`: the Gemini endpoint carries `?key=…`, and a plain
        // reqwest error Display would echo that URL — API key included —
        // into the surfaced error string (shown in the popover / returned
        // to the browser).
        .map_err(|e| ImageGenerateError::Request {
            provider: "Gemini",
            message: e.without_url().to_string(),
        })?;
    let (status, body) = read_provider_body("Gemini", resp).await?;
    if !status.is_success() {
        return Err(ImageGenerateError::Provider(provider_error(
            "Gemini", status, &body,
        )));
    }
    let json: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| ImageGenerateError::ResponseParse {
            provider: "Gemini",
            message: e.to_string(),
        })?;
    let parts = json
        .get("candidates")
        .and_then(|c| c.as_array())
        .and_then(|c| c.first())
        .and_then(|c| c.get("content"))
        .and_then(|c| c.get("parts"))
        .and_then(|p| p.as_array());
    let Some(parts) = parts else {
        return Err(ImageGenerateError::MissingInlineImage);
    };
    for part in parts {
        let Some(inline) = part.get("inlineData") else {
            continue;
        };
        let mime = inline
            .get("mimeType")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        if !mime.starts_with("image/") {
            continue;
        }
        if let Some(data) = inline.get("data").and_then(serde_json::Value::as_str) {
            return Ok(format!("data:{mime};base64,{data}"));
        }
    }
    Err(ImageGenerateError::MissingInlineImage)
}

/// Replicate prediction create + poll (shared desktop + web).
pub async fn generate_replicate(
    client: &reqwest::Client,
    prompt: &str,
    profile: &ImageGenProfile,
    width: Option<f64>,
    height: Option<f64>,
) -> Result<String, ImageGenerateError> {
    let base = profile
        .base_url
        .as_deref()
        .filter(|b| !b.trim().is_empty())
        .unwrap_or("https://api.replicate.com");
    let base = base.trim_end_matches('/');
    let mut input = serde_json::json!({ "prompt": prompt });
    if let Some(w) = width {
        input["width"] = serde_json::json!(w as i64);
    }
    if let Some(h) = height {
        input["height"] = serde_json::json!(h as i64);
    }
    let resp = client
        .post(format!("{base}/v1/predictions"))
        .bearer_auth(profile.api_key.trim())
        .json(&serde_json::json!({ "model": profile.model, "input": input }))
        .send()
        .await
        .map_err(|e| ImageGenerateError::Request {
            provider: "Replicate",
            message: e.to_string(),
        })?;
    let (status, body) = read_provider_body("Replicate", resp).await?;
    if !status.is_success() {
        return Err(ImageGenerateError::Provider(provider_error(
            "Replicate",
            status,
            &body,
        )));
    }
    let prediction: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| ImageGenerateError::ResponseParse {
            provider: "Replicate",
            message: e.to_string(),
        })?;
    let Some(id) = prediction.get("id").and_then(serde_json::Value::as_str) else {
        return Err(ImageGenerateError::MissingPredictionId {
            provider: "Replicate",
        });
    };
    // Poll until terminal (TS: max 120 s, 2 s interval). The deadline is
    // wall-clock, not iteration-count: each poll also carries its own short
    // timeout so a black-holing endpoint can't stretch "60 polls" into
    // hours of a held connection thread (the shared client's 120 s
    // per-request timeout multiplies per iteration otherwise).
    let deadline = tokio::time::Instant::now() + Duration::from_secs(120);
    loop {
        tokio::time::sleep(Duration::from_secs(2)).await;
        // A poll never runs past the deadline: its own timeout is capped to
        // the time remaining, so the loop's total stays ~120 s instead of
        // "deadline + one full poll".
        let now = tokio::time::Instant::now();
        if now >= deadline {
            break;
        }
        let poll_timeout = Duration::from_secs(15).min(deadline - now);
        let resp = client
            .get(format!("{base}/v1/predictions/{id}"))
            .bearer_auth(profile.api_key.trim())
            .timeout(poll_timeout)
            .send()
            .await
            .map_err(|e| ImageGenerateError::PollRequest {
                provider: "Replicate",
                message: e.to_string(),
            })?;
        let (status, body) = read_provider_body("Replicate", resp).await?;
        if !status.is_success() {
            return Err(ImageGenerateError::PollStatus {
                provider: "Replicate",
                status: status.as_u16(),
                body: body.chars().take(ERROR_MESSAGE_CAP).collect::<String>(),
            });
        }
        let json: serde_json::Value =
            serde_json::from_str(&body).map_err(|e| ImageGenerateError::PollParse {
                provider: "Replicate",
                message: e.to_string(),
            })?;
        match json.get("status").and_then(serde_json::Value::as_str) {
            Some("succeeded") => {
                let output = json.get("output");
                let url = output
                    .and_then(|o| o.as_array())
                    .and_then(|a| a.first())
                    .and_then(serde_json::Value::as_str)
                    .or_else(|| output.and_then(serde_json::Value::as_str));
                return url
                    .map(str::to_string)
                    .ok_or(ImageGenerateError::OutputMissing {
                        provider: "Replicate",
                    });
            }
            Some(s @ ("failed" | "canceled")) => {
                let detail = json
                    .get("error")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("unknown error");
                return Err(ImageGenerateError::PredictionFailed {
                    provider: "Replicate",
                    state: s.to_string(),
                    detail: detail.to_string(),
                });
            }
            _ => {}
        }
    }
    Err(ImageGenerateError::PredictionTimeout {
        provider: "Replicate",
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_mappers_mirror_ts() {
        assert_eq!(openai_size(None, None), "1024x1024");
        assert_eq!(openai_size(Some(1600.0), Some(900.0)), "1792x1024");
        assert_eq!(openai_size(Some(900.0), Some(1600.0)), "1024x1792");
        assert_eq!(gemini_aspect_ratio(None, None), None);
        assert_eq!(
            gemini_aspect_ratio(Some(1920.0), Some(1080.0)),
            Some("16:9")
        );
        assert_eq!(gemini_aspect_ratio(Some(500.0), Some(500.0)), Some("1:1"));
    }

    #[test]
    fn provider_error_prefers_the_message_field() {
        let status = reqwest::StatusCode::BAD_GATEWAY;
        assert_eq!(
            provider_error(
                "OpenAI",
                status,
                r#"{"error":{"message":"quota exceeded"}}"#
            ),
            "quota exceeded"
        );
        assert_eq!(
            provider_error("Replicate", status, r#"{"detail":"invalid token"}"#),
            "invalid token"
        );
        assert!(provider_error("Gemini", status, "<html>boom</html>")
            .starts_with("Gemini returned 502"));
    }
}
