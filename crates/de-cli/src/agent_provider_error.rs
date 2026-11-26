//! Typed failures of `de agent`'s provider selection.
//!
//! None of them may show a secret. Values that could be one are never
//! printed: a base URL can carry credentials (`https://user:key@host`,
//! `?key=`), a settings parse error can quote a key, and `--api-key-env` may
//! have been handed the key itself instead of a variable name.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AgentProviderError {
    /// The platform has no config directory and `--settings` was not given.
    NoSettingsPath,
    /// The settings file exists but could not be read or parsed. `detail`
    /// is already secret-free ([`settings_detail`]).
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
    /// `--base-url` / `DE_AGENT_BASE_URL` is not an http(s) URL. The value
    /// is not kept: it may hold credentials.
    InvalidBaseUrl,
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
            Self::ApiKeyEnvUnset { var } if looks_like_variable_name(var) => {
                write!(f, "environment variable {var} (--api-key-env) is not set")
            }
            Self::ApiKeyEnvUnset { var } => write!(
                f,
                "--api-key-env takes the NAME of an environment variable, and the value \
                 given ({} characters, not shown) is not one; pass e.g. \
                 --api-key-env ANTHROPIC_API_KEY",
                var.chars().count()
            ),
            Self::MissingModel { key_var } => write!(
                f,
                "an API key was given in {key_var}, so a model is required: pass --model \
                 or set DE_AGENT_MODEL"
            ),
            Self::UnknownApiFormat { value } => {
                write!(f, "unknown API format {value:?} (use anthropic or openai)")
            }
            Self::InvalidBaseUrl => f.write_str(
                "the base URL (--base-url / DE_AGENT_BASE_URL) must start with http:// or \
                 https:// (the value is not shown: it may hold credentials)",
            ),
        }
    }
}

impl std::error::Error for AgentProviderError {}

/// An environment variable name as shells write them: a letter or `_`,
/// then letters, digits or `_`, at most 128 characters. Anything else is
/// treated as a possible secret and not echoed.
pub(crate) fn looks_like_variable_name(value: &str) -> bool {
    let mut chars = value.chars();
    value.len() <= 128
        && chars
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// A settings read / parse message without file content: `serde_json`
/// quotes offending values (`invalid type: string "sk-…"`), so only the
/// position survives.
pub(crate) fn settings_detail(detail: &str) -> String {
    match detail.rfind(" at line ") {
        Some(at) => format!(
            "not valid settings JSON ({})",
            detail[at + " at ".len()..].trim()
        ),
        None if detail.contains('"') => "not valid settings JSON".to_string(),
        None => detail.to_string(),
    }
}
