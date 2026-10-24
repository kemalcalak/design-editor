//! The source image of a transform (`vectorize-image`, `remove-background`,
//! `replace-background`): a url the document already holds, checked against
//! pen.dev's limits (JPEG / PNG / WEBP, at most 16 MB, 256 to 4096 px a side).

use base64::Engine as _;

use super::GenerateJobError;

pub const MAX_SOURCE_BYTES: usize = 16 * 1024 * 1024;
pub const MIN_SOURCE_PX: u32 = 256;
pub const MAX_SOURCE_PX: u32 = 4096;

/// Where a transform's source comes from. A remote url is fetched on the
/// job's worker thread, never on the caller's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceRef {
    Data(String),
    Remote(String),
}

impl SourceRef {
    /// Classify a url taken from the document.
    pub fn from_url(url: &str) -> Result<Self, GenerateJobError> {
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
            return Ok(SourceRef::Remote(url.to_string()));
        }
        Err(GenerateJobError::Source(format!(
            "unsupported image url {:?}",
            url.chars().take(60).collect::<String>()
        )))
    }

    /// The decoded, checked source. Network for a remote url.
    pub fn load(&self) -> Result<SourceImage, GenerateJobError> {
        match self {
            SourceRef::Data(url) => SourceImage::from_data_url(url),
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
                SourceImage::from_bytes(bytes)
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
    pub fn from_data_url(url: &str) -> Result<Self, GenerateJobError> {
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
        Self::from_bytes(bytes)
    }

    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, GenerateJobError> {
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
        let range = MIN_SOURCE_PX..=MAX_SOURCE_PX;
        if !range.contains(&width) || !range.contains(&height) {
            return Err(GenerateJobError::Source(format!(
                "{width}x{height} px is outside {MIN_SOURCE_PX} to {MAX_SOURCE_PX} px a side"
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
