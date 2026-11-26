//! Typed failures for the shared image-generation backends
//! (`web_image_generate.rs`): the OpenAI / Gemini / Replicate / Atlas
//! provider calls used by the MCP `generate` jobs and the desktop Generate
//! popover (`de-host-desktop::image_generate_host`).
//!
//! Style follows `de_orchestrator::OrchestratorError`: a plain enum plus a
//! hand-written `Display`, no `thiserror` and no new dependency. Nearly every
//! variant carries STRUCTURED fields and `Display` re-formats the sentence,
//! so the user-visible text is reproduced byte for byte.
//!
//! Two things are deliberately NOT typed here:
//!
//! - [`ImageGenerateError::Dial`] carries the connect-time endpoint guard's
//!   sentence already rendered. `provider_dial::ProviderDialError` is
//!   crate-private while this enum must be `pub` (the desktop host consumes
//!   it), so embedding it would leak a private type into a public interface.
//! - [`ImageGenerateError::Provider`] carries whatever `provider_error`
//!   extracted from the PROVIDER's own error body (its `error.message` /
//!   `detail`, or a status + body slice). The shape is the provider's, not
//!   ours, so it rides verbatim.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageGenerateError {
    // --- endpoint screening (SSRF guards) ---
    /// The custom `base_url` failed URL-shape screening. The underlying
    /// reason is deliberately NOT surfaced, so the guard is no probe oracle.
    EndpointNotAllowed,
    /// The provider's RESULT url failed screening before download. Same
    /// reasoning: an allowlisted endpoint is not an allowlisted download
    /// host.
    ResultUrlNotAllowed,
    /// The connect-time dial guard refused. Text is that guard's own
    /// sentence, verbatim.
    Dial(String),

    // --- provider round-trips ---
    /// reqwest refused to build the HTTP client (local configuration).
    ClientBuild { message: String },
    /// The provider's response body exceeded the streaming size cap.
    BodyTooLarge { provider: &'static str },
    /// The request to the provider never completed. Note the Gemini call
    /// site passes `reqwest::Error::without_url()`'s text — the Gemini
    /// endpoint carries `?key=…` and a plain `Display` would echo the API
    /// key into this message.
    Request {
        provider: &'static str,
        message: String,
    },
    /// An asynchronous provider poll request never completed.
    PollRequest {
        provider: &'static str,
        message: String,
    },
    /// The provider answered with a non-success status; text is
    /// `provider_error`'s extraction from the body.
    Provider(String),
    /// The provider's body was not valid JSON.
    ResponseParse {
        provider: &'static str,
        message: String,
    },
    /// An asynchronous provider poll body was not valid JSON.
    PollParse {
        provider: &'static str,
        message: String,
    },

    // --- provider payload shape ---
    /// OpenAI answered 2xx but carried neither `url` nor `b64_json`.
    MissingImageUrl,
    /// Gemini answered 2xx but carried no `inlineData` image part.
    MissingInlineImage,
    /// An asynchronous provider accepted a prediction but returned no id.
    MissingPredictionId { provider: &'static str },
    /// An asynchronous provider poll answered with a non-success status.
    PollStatus {
        provider: &'static str,
        status: u16,
        body: String,
    },
    /// A prediction succeeded but carried no output url.
    OutputMissing { provider: &'static str },
    /// A prediction reached a terminal failure state.
    PredictionFailed {
        provider: &'static str,
        state: String,
        detail: String,
    },
    /// A provider poll loop hit its wall-clock deadline.
    PredictionTimeout { provider: &'static str },
    /// The generated image's remote url could not be fetched into a
    /// `data:` URL.
    DownloadFailed,
}

impl fmt::Display for ImageGenerateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImageGenerateError::EndpointNotAllowed => {
                f.write_str("provider endpoint is not allowed")
            }
            ImageGenerateError::ResultUrlNotAllowed => {
                f.write_str("generated image URL is not allowed")
            }
            ImageGenerateError::Dial(message) | ImageGenerateError::Provider(message) => {
                f.write_str(message)
            }
            ImageGenerateError::ClientBuild { message } => write!(f, "http client: {message}"),
            ImageGenerateError::BodyTooLarge { provider } => {
                write!(f, "{provider} response exceeded the size limit")
            }
            ImageGenerateError::Request { provider, message } => {
                write!(f, "{provider} request failed: {message}")
            }
            ImageGenerateError::PollRequest { provider, message } => {
                write!(f, "{provider} poll request failed: {message}")
            }
            ImageGenerateError::ResponseParse { provider, message } => {
                write!(f, "{provider} response parse: {message}")
            }
            ImageGenerateError::PollParse { provider, message } => {
                write!(f, "{provider} poll parse: {message}")
            }
            ImageGenerateError::MissingImageUrl => f.write_str("OpenAI response missing image URL"),
            ImageGenerateError::MissingInlineImage => {
                f.write_str("Gemini response missing inline image data")
            }
            ImageGenerateError::MissingPredictionId { provider } => {
                write!(f, "{provider} response missing prediction ID")
            }
            ImageGenerateError::PollStatus {
                provider,
                status,
                body,
            } => {
                write!(f, "{provider} poll returned {status}: {body}")
            }
            ImageGenerateError::OutputMissing { provider } => {
                write!(f, "{provider} succeeded but output is missing")
            }
            ImageGenerateError::PredictionFailed {
                provider,
                state,
                detail,
            } => {
                write!(f, "{provider} prediction {state}: {detail}")
            }
            ImageGenerateError::PredictionTimeout { provider } => {
                write!(f, "{provider} prediction timed out after 120 seconds")
            }
            ImageGenerateError::DownloadFailed => {
                f.write_str("generated image could not be downloaded")
            }
        }
    }
}

impl std::error::Error for ImageGenerateError {}

/// Single-table adaptation of the connect-time endpoint guard, so the dial
/// sites collapse to `?`. Both `Display` impls render the same sentence, so
/// the JSON error body is unchanged byte for byte.
impl From<crate::provider_dial::ProviderDialError> for ImageGenerateError {
    fn from(error: crate::provider_dial::ProviderDialError) -> ImageGenerateError {
        ImageGenerateError::Dial(error.to_string())
    }
}
