//! Typed failures of the MCP `generate` jobs (#10, #11).

use std::fmt;

use crate::web_image_generate::ImageGenerateError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GenerateJobError {
    /// No Images-tab profile and no OpenAI / Gemini chat key: nothing may be
    /// called, so nothing is written.
    NotConfigured,
    /// No chat provider with a key for an LLM-backed job (SVG artwork).
    NoChatProvider,
    /// The configured provider cannot do this operation.
    Unsupported {
        operation: &'static str,
        provider: &'static str,
    },
    /// Too many generations still running in this process.
    Busy { limit: usize },
    /// A stock search found nothing usable.
    NoStockResult,
    /// An image provider call failed.
    Image(ImageGenerateError),
    /// The source image could not be read or decoded.
    Source(String),
    /// Any other failure, already phrased for the client.
    Failed(String),
}

impl fmt::Display for GenerateJobError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GenerateJobError::NotConfigured => f.write_str(
                "no image provider configured: add an image generation profile in \
                 Settings > Images, or an OpenAI or Gemini API key in Settings > Agents",
            ),
            GenerateJobError::NoChatProvider => f.write_str(
                "no chat provider configured: add a built-in provider with an API key \
                 in Settings > Agents",
            ),
            GenerateJobError::Unsupported {
                operation,
                provider,
            } => write!(
                f,
                "{operation} is not supported by the {provider} image provider; \
                 configure an OpenAI or Gemini image profile"
            ),
            GenerateJobError::Busy { limit } => write!(
                f,
                "{limit} generations are already running; wait for one to finish"
            ),
            GenerateJobError::NoStockResult => f.write_str("no stock photo matched the query"),
            GenerateJobError::Image(error) => error.fmt(f),
            GenerateJobError::Source(message) => write!(f, "source image: {message}"),
            GenerateJobError::Failed(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for GenerateJobError {}

impl From<ImageGenerateError> for GenerateJobError {
    fn from(error: ImageGenerateError) -> Self {
        match error {
            ImageGenerateError::NotConfigured => GenerateJobError::NotConfigured,
            other => GenerateJobError::Image(other),
        }
    }
}
