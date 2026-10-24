//! `remove-background` / `replace-background` through the user's image
//! provider (#11).
//!
//! No small pure-Rust background-removal model exists, so both transforms are
//! image edits on the provider the user configured:
//!
//! - OpenAI (and OpenAI-compatible custom endpoints): `images/edits` with a
//!   GPT image model. Removal asks for `background: transparent`, so the
//!   cutout comes back with real alpha.
//! - Gemini: an image-input `generateContent` edit. Gemini cannot return
//!   transparency, so removal asks for the subject on flat pure green and the
//!   green is keyed out locally ([`chroma_key_green`]).
//! - Replicate / Atlas profiles have no generic edit call and are refused.
//!
//! The result is a model re-render: edges and fine detail can differ slightly
//! from the source, which a segmentation model would not change.

use base64::Engine as _;

use de_editor_core::agent_settings::{ImageGenProfile, ImageGenProvider};

use super::source::{png_data_url, SourceImage};
use super::{GenerateJobError, GEMINI_FALLBACK_IMAGE_MODEL, OPENAI_FALLBACK_IMAGE_MODEL};
use crate::web_image_generate::{is_gpt_image_model, ImageGenerateError};

/// Which transform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditKind {
    RemoveBackground,
    /// Keep the subject; the prompt describes the new background.
    ReplaceBackground(String),
}

impl EditKind {
    fn operation(&self) -> &'static str {
        match self {
            EditKind::RemoveBackground => "remove-background",
            EditKind::ReplaceBackground(_) => "replace-background",
        }
    }
}

/// The instruction sent with the image.
pub fn edit_prompt(kind: &EditKind, provider: ImageGenProvider) -> String {
    match (kind, provider) {
        (EditKind::RemoveBackground, ImageGenProvider::Gemini) => {
            "Remove the background of this image. Keep the main subject exactly as it is: same \
             shape, colours, details and position, with clean edges. Place it on a perfectly \
             flat, uniform pure green (#00FF00) background with no shadows, gradients, \
             reflections or texture, and no green on the subject itself."
                .to_string()
        }
        (EditKind::RemoveBackground, _) => {
            "Remove the background of this image. Keep the main subject exactly as it is: same \
             shape, colours, details and position, with clean edges, on a fully transparent \
             background."
                .to_string()
        }
        (EditKind::ReplaceBackground(background), _) => format!(
            "Keep the main subject of this image exactly as it is: same shape, colours, details, \
             pose and position. Replace only the background with: {background}. Light the \
             subject so it sits naturally in the new background."
        ),
    }
}

/// Refuse a transform the configured provider cannot do, before anything is
/// written: Replicate / Atlas have no generic edit call, and the web daemon
/// (`screening`) cannot screen a custom endpoint for these direct calls.
pub fn check_supported(
    kind: &EditKind,
    profile: &ImageGenProfile,
    screening: bool,
) -> Result<(), GenerateJobError> {
    if matches!(
        profile.provider,
        ImageGenProvider::Replicate | ImageGenProvider::Atlas
    ) {
        return Err(GenerateJobError::Unsupported {
            operation: kind.operation(),
            provider: profile.provider.label(),
        });
    }
    let custom = profile
        .base_url
        .as_deref()
        .is_some_and(|base| !base.trim().is_empty());
    if screening && custom {
        return Err(GenerateJobError::CustomEndpointRefused);
    }
    Ok(())
}

/// Run one edit and return the result as a `data:` url.
pub async fn edit_image(
    client: &reqwest::Client,
    source: &SourceImage,
    kind: &EditKind,
    profile: &ImageGenProfile,
) -> Result<String, GenerateJobError> {
    check_supported(kind, profile, false)?;
    match profile.provider {
        ImageGenProvider::OpenAi | ImageGenProvider::Custom => {
            openai_edit(client, source, kind, profile).await
        }
        ImageGenProvider::Gemini => {
            let edited = gemini_edit(client, source, kind, profile).await?;
            if *kind != EditKind::RemoveBackground {
                return Ok(edited);
            }
            let bytes = decode_data_url_payload(&edited)?;
            let keyed = chroma_key_green(&bytes)?;
            Ok(png_data_url(&keyed))
        }
        ImageGenProvider::Replicate | ImageGenProvider::Atlas => {
            unreachable!("refused by check_supported")
        }
    }
}

/// The GPT image model an OpenAI edit uses: the profile's own when it is
/// one, else the default (DALL-E 3 has no edit endpoint). A custom
/// OpenAI-compatible endpoint keeps whatever model it names.
pub fn openai_edit_model(profile: &ImageGenProfile) -> String {
    if is_gpt_image_model(&profile.model) || profile.provider == ImageGenProvider::Custom {
        profile.model.trim().to_string()
    } else {
        OPENAI_FALLBACK_IMAGE_MODEL.to_string()
    }
}

/// The text fields of the `images/edits` form.
pub fn openai_edit_fields(kind: &EditKind, model: &str) -> Vec<(&'static str, String)> {
    let mut fields = vec![
        ("model", model.to_string()),
        ("prompt", edit_prompt(kind, ImageGenProvider::OpenAi)),
        ("n", "1".to_string()),
    ];
    if is_gpt_image_model(model) {
        fields.push(("output_format", "png".to_string()));
        if *kind == EditKind::RemoveBackground {
            fields.push(("background", "transparent".to_string()));
        }
    }
    fields
}

/// A `multipart/form-data` body: the text fields, then one file part.
pub fn multipart_body(
    boundary: &str,
    fields: &[(&'static str, String)],
    file_field: &str,
    file_name: &str,
    file_mime: &str,
    file: &[u8],
) -> Vec<u8> {
    let mut body = Vec::with_capacity(file.len() + 1024);
    for (name, value) in fields {
        body.extend_from_slice(
            format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n"
            )
            .as_bytes(),
        );
    }
    body.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"{file_field}\"; \
             filename=\"{file_name}\"\r\nContent-Type: {file_mime}\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(file);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    body
}

async fn openai_edit(
    client: &reqwest::Client,
    source: &SourceImage,
    kind: &EditKind,
    profile: &ImageGenProfile,
) -> Result<String, GenerateJobError> {
    let base = profile
        .base_url
        .as_deref()
        .filter(|b| !b.trim().is_empty())
        .unwrap_or("https://api.openai.com");
    let endpoint = format!("{}/v1/images/edits", base.trim_end_matches('/'));
    let model = openai_edit_model(profile);
    let boundary = format!("de-boundary-{:016x}", boundary_nonce());
    let extension = source.mime.trim_start_matches("image/");
    let body = multipart_body(
        &boundary,
        &openai_edit_fields(kind, &model),
        "image",
        &format!("source.{extension}"),
        source.mime,
        &source.bytes,
    );
    let response = client
        .post(endpoint)
        .bearer_auth(profile.api_key.trim())
        .header(
            reqwest::header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(body)
        .send()
        .await
        .map_err(|e| request_error("OpenAI", e.to_string()))?;
    let (status, text) = read_body("OpenAI", response).await?;
    if !status.is_success() {
        return Err(provider_error("OpenAI", status, &text));
    }
    let json: serde_json::Value = serde_json::from_str(&text).map_err(|e| {
        GenerateJobError::Image(ImageGenerateError::ResponseParse {
            provider: "OpenAI",
            message: e.to_string(),
        })
    })?;
    let first = json
        .get("data")
        .and_then(|d| d.as_array())
        .and_then(|d| d.first());
    if let Some(b64) = first
        .and_then(|d| d.get("b64_json"))
        .and_then(serde_json::Value::as_str)
    {
        return Ok(format!("data:image/png;base64,{b64}"));
    }
    // OpenAI-compatible endpoints may answer with a url instead.
    let url = first
        .and_then(|d| d.get("url"))
        .and_then(serde_json::Value::as_str)
        .ok_or(GenerateJobError::Image(ImageGenerateError::MissingImageUrl))?;
    crate::web_image_search::fetch_image_data_url(client, url)
        .await
        .ok_or(GenerateJobError::Image(ImageGenerateError::DownloadFailed))
}

/// The Gemini image model an edit uses.
pub fn gemini_edit_model(profile: &ImageGenProfile) -> String {
    let model = profile.model.trim();
    if model.contains("image") {
        model.to_string()
    } else {
        GEMINI_FALLBACK_IMAGE_MODEL.to_string()
    }
}

/// The Gemini `generateContent` body: the image, then the instruction.
pub fn gemini_edit_body(source: &SourceImage, kind: &EditKind) -> serde_json::Value {
    serde_json::json!({
        "contents": [{ "parts": [
            { "inlineData": { "mimeType": source.mime, "data": source.base64() } },
            { "text": edit_prompt(kind, ImageGenProvider::Gemini) },
        ]}],
        "generationConfig": { "responseModalities": ["TEXT", "IMAGE"] },
    })
}

async fn gemini_edit(
    client: &reqwest::Client,
    source: &SourceImage,
    kind: &EditKind,
    profile: &ImageGenProfile,
) -> Result<String, GenerateJobError> {
    let base = profile
        .base_url
        .as_deref()
        .filter(|b| !b.trim().is_empty())
        .unwrap_or("https://generativelanguage.googleapis.com");
    let endpoint = format!(
        "{}/v1beta/models/{}:generateContent",
        base.trim_end_matches('/'),
        gemini_edit_model(profile),
    );
    let response = client
        .post(endpoint)
        // The key rides a header, so no error text can echo it in a url.
        .header("x-goog-api-key", profile.api_key.trim())
        .json(&gemini_edit_body(source, kind))
        .send()
        .await
        .map_err(|e| request_error("Gemini", e.without_url().to_string()))?;
    let (status, text) = read_body("Gemini", response).await?;
    if !status.is_success() {
        return Err(provider_error("Gemini", status, &text));
    }
    gemini_inline_image(&text)
}

/// The first inline image part of a Gemini answer, as a data url.
pub fn gemini_inline_image(body: &str) -> Result<String, GenerateJobError> {
    let json: serde_json::Value = serde_json::from_str(body).map_err(|e| {
        GenerateJobError::Image(ImageGenerateError::ResponseParse {
            provider: "Gemini",
            message: e.to_string(),
        })
    })?;
    json.pointer("/candidates/0/content/parts")
        .and_then(|parts| parts.as_array())
        .into_iter()
        .flatten()
        .filter_map(|part| part.get("inlineData"))
        .find_map(|inline| {
            let mime = inline.get("mimeType")?.as_str()?;
            let data = inline.get("data")?.as_str()?;
            mime.starts_with("image/")
                .then(|| format!("data:{mime};base64,{data}"))
        })
        .ok_or(GenerateJobError::Image(
            ImageGenerateError::MissingInlineImage,
        ))
}

fn request_error(provider: &'static str, message: String) -> GenerateJobError {
    GenerateJobError::Image(ImageGenerateError::Request { provider, message })
}

fn provider_error(provider: &str, status: reqwest::StatusCode, body: &str) -> GenerateJobError {
    GenerateJobError::Image(ImageGenerateError::Provider(
        crate::web_image_generate::provider_error(provider, status, body),
    ))
}

async fn read_body(
    provider: &'static str,
    response: reqwest::Response,
) -> Result<(reqwest::StatusCode, String), GenerateJobError> {
    crate::web_image_generate::read_provider_body(provider, response)
        .await
        .map_err(GenerateJobError::Image)
}

fn decode_data_url_payload(url: &str) -> Result<Vec<u8>, GenerateJobError> {
    let payload = url
        .split_once(',')
        .map(|(_, payload)| payload)
        .unwrap_or_default();
    base64::engine::general_purpose::STANDARD
        .decode(payload)
        .map_err(|_| GenerateJobError::EditResult("not valid base64".into()))
}

/// A multipart boundary that cannot occur in the image bytes by chance.
fn boundary_nonce() -> u64 {
    let mut bytes = [0u8; 8];
    if getrandom::fill(&mut bytes).is_err() {
        return std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x5eed);
    }
    u64::from_le_bytes(bytes)
}

/// Key out a flat green background: green-dominant pixels turn transparent,
/// a soft band around the threshold keeps anti-aliased edges, and the green
/// spill left on edge pixels is pulled back. Returns a PNG with alpha.
pub fn chroma_key_green(bytes: &[u8]) -> Result<Vec<u8>, GenerateJobError> {
    let mut rgba = image::load_from_memory(bytes)
        .map_err(|e| GenerateJobError::EditResult(e.to_string()))?
        .to_rgba8();
    for pixel in rgba.pixels_mut() {
        let [r, g, b, a] = pixel.0;
        let (r, g, b) = (r as f32, g as f32, b as f32);
        // How much greener than the other channels this pixel is.
        let excess = g - r.max(b);
        let alpha = if excess >= 90.0 {
            0.0
        } else if excess > 30.0 {
            1.0 - (excess - 30.0) / 60.0
        } else {
            1.0
        };
        let g = if excess > 0.0 && alpha < 1.0 {
            g.min(r.max(b))
        } else {
            g
        };
        pixel.0 = [r as u8, g as u8, b as u8, (alpha * a as f32).round() as u8];
    }
    let mut out = Vec::new();
    rgba.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
        .map_err(|e| GenerateJobError::EditResult(format!("encode the cutout: {e}")))?;
    Ok(out)
}

#[cfg(test)]
#[path = "image_edit_tests.rs"]
mod tests;
