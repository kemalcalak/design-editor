//! Typed failures of the MCP `generate` jobs (#10).

use std::fmt;

use crate::web_image_generate::ImageGenerateError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GenerateJobError {
    /// No Images-tab profile and no OpenAI / Gemini chat key: nothing may be
    /// called, so nothing is written.
    NotConfigured,
    /// Too many generations still running in this process.
    Busy { limit: usize },
    /// A stock search found nothing usable.
    NoStockResult,
    /// An image provider call failed.
    Image(ImageGenerateError),
    /// The caller's deadline passed before or during the provider call.
    TimedOut,
    /// The worker panicked; its slot is released and the job reported failed.
    Panicked,
    /// The worker thread could not be started.
    Spawn(String),
}

impl fmt::Display for GenerateJobError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GenerateJobError::NotConfigured => f.write_str(
                "no image provider configured: add an image generation profile in \
                 Settings > Images, or an OpenAI or Gemini API key in Settings > Agents",
            ),
            GenerateJobError::Busy { limit } => write!(
                f,
                "{limit} generations are already running; wait for one to finish"
            ),
            GenerateJobError::NoStockResult => f.write_str("no stock photo matched the query"),
            GenerateJobError::Image(error) => error.fmt(f),
            GenerateJobError::TimedOut => f.write_str("the generation ran out of time"),
            GenerateJobError::Panicked => f.write_str("the generation failed unexpectedly"),
            GenerateJobError::Spawn(message) => {
                write!(f, "could not start the generation: {message}")
            }
        }
    }
}

impl std::error::Error for GenerateJobError {}
