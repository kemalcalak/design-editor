//! The source image of a transform (`vectorize-image`, `remove-background`,
//! `replace-background`): a url the document already holds, checked against
//! pen.dev's limits (JPEG / PNG / WEBP, at most 16 MB, 256 to 4096 px a side).

use base64::Engine as _;

use super::GenerateJobError;

pub const MAX_SOURCE_BYTES: usize = 16 * 1024 * 1024;
/// pen.dev's lower bound for the background transforms.
pub const MIN_SOURCE_PX: u32 = 256;
/// Vectorize takes small logos and marks too.
pub const MIN_TRACE_SOURCE_PX: u32 = 16;
pub const MAX_SOURCE_PX: u32 = 4096;

/// Where a transform's source comes from. A remote url is fetched on the
/// job's worker thread, never on the caller's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceRef {
    Data(String),
    Remote(String),
}

impl SourceRef {
    /// Classify a url taken from the document. `allow_remote` is off on the
    /// web daemon, whose documents come from the browser: fetching their
    /// urls from the daemon's host would be an SSRF hop.
    pub fn from_url(url: &str, allow_remote: bool) -> Result<Self, GenerateJobError> {
        let url = url.trim();
        if url.starts_with(super::PENDING_IMAGE_PREFIX) {
            return Err(GenerateJobError::Source(
                "the image has not arrived yet; transform it in a later call".into(),
            ));
        }
        if url.starts_with("data:") {
            return Ok(SourceRef::Data(url.to_string()));
        }
        if url.starts_with("https://") || url.starts_with("http://") {
            if !allow_remote {
                return Err(GenerateJobError::Source(
                    "remote image urls are not fetched by the web daemon; embed the image first"
                        .into(),
                ));
            }
            return Ok(SourceRef::Remote(url.to_string()));
        }
        Err(GenerateJobError::Source(format!(
            "unsupported image url {:?}",
            url.chars().take(60).collect::<String>()
        )))
    }

    /// The decoded, checked source (sides at least `min_px`). Network for a
    /// remote url.
    pub fn load(&self, min_px: u32) -> Result<SourceImage, GenerateJobError> {
        match self {
            SourceRef::Data(url) => SourceImage::from_data_url(url, min_px),
            SourceRef::Remote(url) => {
                let client = super::backend::provider_client()?;
                let fetched = crate::chat_runtime::block_on_anywhere(
                    de_image_enrich::net::providers::fetch_image_bytes(
                        &client,
                        url,
                        MAX_SOURCE_BYTES,
                    ),
                );
                let (_, bytes) = fetched.ok_or_else(|| {
                    GenerateJobError::Source("the image could not be downloaded".into())
                })?;
                SourceImage::from_bytes(bytes, min_px)
            }
        }
    }
}

/// A checked source image.
#[derive(Debug, Clone)]
pub struct SourceImage {
    pub mime: &'static str,
    pub bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

impl SourceImage {
    pub fn from_data_url(url: &str, min_px: u32) -> Result<Self, GenerateJobError> {
        let payload = url
            .strip_prefix("data:")
            .and_then(|rest| rest.split_once(','))
            .filter(|(meta, _)| meta.contains(";base64"))
            .map(|(_, payload)| payload)
            .ok_or_else(|| GenerateJobError::Source("not a base64 data url".into()))?;
        if payload.len() / 4 * 3 > MAX_SOURCE_BYTES {
            return Err(GenerateJobError::Source("larger than 16 MB".into()));
        }
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(payload.trim())
            .map_err(|_| GenerateJobError::Source("invalid base64 payload".into()))?;
        Self::from_bytes(bytes, min_px)
    }

    pub fn from_bytes(bytes: Vec<u8>, min_px: u32) -> Result<Self, GenerateJobError> {
        if bytes.len() > MAX_SOURCE_BYTES {
            return Err(GenerateJobError::Source("larger than 16 MB".into()));
        }
        let format = image::guess_format(&bytes)
            .map_err(|_| GenerateJobError::Source("not a JPEG, PNG or WEBP image".into()))?;
        let mime = match format {
            image::ImageFormat::Png => "image/png",
            image::ImageFormat::Jpeg => "image/jpeg",
            image::ImageFormat::WebP => "image/webp",
            _ => {
                return Err(GenerateJobError::Source(
                    "not a JPEG, PNG or WEBP image".into(),
                ))
            }
        };
        let (width, height) = image::ImageReader::with_format(std::io::Cursor::new(&bytes), format)
            .into_dimensions()
            .map_err(|e| GenerateJobError::Source(format!("unreadable image: {e}")))?;
        let range = min_px..=MAX_SOURCE_PX;
        if !range.contains(&width) || !range.contains(&height) {
            return Err(GenerateJobError::Source(format!(
                "{width}x{height} px is outside {min_px} to {MAX_SOURCE_PX} px a side"
            )));
        }
        Ok(Self {
            mime,
            bytes,
            width,
            height,
        })
    }

    /// The source as a base64 payload (no `data:` prefix).
    pub fn base64(&self) -> String {
        base64::engine::general_purpose::STANDARD.encode(&self.bytes)
    }
}

/// `bytes` (PNG) as a data url.
pub fn png_data_url(bytes: &[u8]) -> String {
    format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    )
}
