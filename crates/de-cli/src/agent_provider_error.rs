//! Typed failures of `de agent`'s provider selection. None of them ever
//! carries an API key.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AgentProviderError {
    /// The platform has no config directory and `--settings` was not given.
    NoSettingsPath,
    /// The settings file exists but could not be read or parsed.
    Settings { path: String, detail: String },
    /// No saved provider is ready and no key was given in the environment.
    NoProvider { settings: String },
    /// `--provider` / `provider:` matched no saved provider.
    UnknownProvider { query: String, known: Vec<String> },
    /// `--provider` matched more than one saved provider by name or preset.
    AmbiguousProvider { query: String, ids: Vec<String> },
    /// The chosen saved provider is disabled or lacks a key or model.
    ProviderNotReady { id: String },
    /// The model is not one of the saved provider's models.
    ModelNotSaved {
        provider: String,
        model: String,
        saved: Vec<String>,
    },
    /// `--api-key-env NAME` names a variable that is unset or empty.
    ApiKeyEnvUnset { var: String },
    /// A key came from the environment but no model was named.
    MissingModel { key_var: String },
    /// `--api-format` / `DE_AGENT_API_FORMAT` is neither anthropic nor openai.
    UnknownApiFormat { value: String },
    /// `--base-url` / `DE_AGENT_BASE_URL` is not an http(s) URL.
    InvalidBaseUrl { value: String },
}

impl fmt::Display for AgentProviderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoSettingsPath => f.write_str(
                "cannot locate the Design Editor settings file; pass --settings <path> \
                 or put an API key in DE_AGENT_API_KEY",
            ),
            Self::Settings { path, detail } => {
                write!(f, "cannot read provider settings {path}: {detail}")
            }
            Self::NoProvider { settings } => write!(
                f,
                "no agent provider is ready: save one with an API key and a model in \
                 Design Editor (Settings > Agents; read from {settings}), or set \
                 DE_AGENT_API_KEY and --model for this run"
            ),
            Self::UnknownProvider { query, known } if known.is_empty() => {
                write!(f, "no saved provider matches {query:?}; none are saved")
            }
            Self::UnknownProvider { query, known } => write!(
                f,
                "no saved provider matches {query:?}; saved providers: {}",
                known.join(", ")
            ),
            Self::AmbiguousProvider { query, ids } => write!(
                f,
                "{query:?} matches several saved providers ({}); pass one of their ids",
                ids.join(", ")
            ),
            Self::ProviderNotReady { id } => write!(
                f,
                "saved provider {id} is not ready (it needs to be enabled, with an API key \
                 and a saved model)"
            ),
            Self::ModelNotSaved {
                provider,
                model,
                saved,
            } => write!(
                f,
                "model {model:?} is not saved on provider {provider} (saved: {})",
                saved.join(", ")
            ),
            Self::ApiKeyEnvUnset { var } => {
                write!(f, "environment variable {var} (--api-key-env) is not set")
            }
            Self::MissingModel { key_var } => write!(
                f,
                "an API key was given in {key_var}, so a model is required: pass --model \
                 or set DE_AGENT_MODEL"
            ),
            Self::UnknownApiFormat { value } => {
                write!(f, "unknown API format {value:?} (use anthropic or openai)")
            }
            Self::InvalidBaseUrl { value } => {
                write!(
                    f,
                    "base URL must start with http:// or https://, got {value:?}"
                )
            }
        }
    }
}

impl std::error::Error for AgentProviderError {}
