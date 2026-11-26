//! Which provider and model a `de agent` task runs on.
//!
//! Order, first match wins:
//!
//! 1. A provider name — the task's `provider`, `--provider`, or
//!    `DE_AGENT_PROVIDER` — picks a provider saved in the app (by id, then
//!    display name, then preset, case-insensitive for the last two).
//! 2. An API key in the environment — the variable `--api-key-env` names,
//!    else `DE_AGENT_API_KEY` — runs an unsaved provider for CI:
//!    `--api-format` / `DE_AGENT_API_FORMAT` (`anthropic`, the default, or
//!    `openai`), `--base-url` / `DE_AGENT_BASE_URL` (default: the format's
//!    public endpoint), and a model (required).
//! 3. Otherwise the first ready provider saved in the app.
//!
//! The model is the task's `model`, `--model`, or `DE_AGENT_MODEL`; a saved
//! provider's model must be one it saved (as in the app's model picker),
//! and without one it runs its first saved model. Saved providers are read
//! from `--settings`, `DE_AGENT_SETTINGS`, or the app's own settings file.
//! Keys are only ever read from the environment or the settings file, never
//! from the command line, and never printed.

use std::path::PathBuf;
use std::sync::OnceLock;

use de_editor_core::{BuiltinAgentConfig, BuiltinAgentKind, BuiltinAgentPresetKey};
use de_host_services::settings_io;

pub(crate) use crate::agent_provider_error::AgentProviderError;

pub(crate) const ENV_API_KEY: &str = "DE_AGENT_API_KEY";
pub(crate) const ENV_API_FORMAT: &str = "DE_AGENT_API_FORMAT";
pub(crate) const ENV_BASE_URL: &str = "DE_AGENT_BASE_URL";
pub(crate) const ENV_MODEL: &str = "DE_AGENT_MODEL";
pub(crate) const ENV_PROVIDER: &str = "DE_AGENT_PROVIDER";
pub(crate) const ENV_SETTINGS: &str = "DE_AGENT_SETTINGS";

/// Provider flags from the command line (`--provider` and `--model` reach
/// the tasks as run options instead).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ProviderFlags {
    pub api_format: Option<String>,
    pub base_url: Option<String>,
    pub api_key_env: Option<String>,
    pub settings: Option<PathBuf>,
}

/// Where a resolved provider came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ProviderSource {
    Saved,
    Environment { key_var: String },
}

/// A provider ready to hand to the engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResolvedProvider {
    pub config: BuiltinAgentConfig,
    pub model: String,
    pub source: ProviderSource,
}

impl ResolvedProvider {
    /// A key-free description for progress and reports.
    pub(crate) fn label(&self) -> String {
        match &self.source {
            ProviderSource::Saved => {
                format!("{} ({})", self.config.display_name.trim(), self.config.id)
            }
            ProviderSource::Environment { key_var } => format!("environment ({key_var})"),
        }
    }
}

/// Environment lookup, injectable for tests.
pub(crate) type EnvLookup = Box<dyn Fn(&str) -> Option<String> + Send + Sync>;

/// The process environment, blank values treated as unset.
pub(crate) fn process_env() -> EnvLookup {
    Box::new(|name| {
        std::env::var(name)
            .ok()
            .filter(|value| !value.trim().is_empty())
    })
}

/// Resolves providers for every task of one run; reads the settings file at
/// most once.
pub(crate) struct ProviderResolver {
    flags: ProviderFlags,
    env: EnvLookup,
    saved: OnceLock<Result<(String, Vec<BuiltinAgentConfig>), AgentProviderError>>,
}

impl ProviderResolver {
    pub(crate) fn new(flags: ProviderFlags, env: EnvLookup) -> Self {
        Self {
            flags,
            env,
            saved: OnceLock::new(),
        }
    }

    /// The provider and model for a task with these `provider` / `model`
    /// options (already layered task > file defaults > command line).
    pub(crate) fn resolve(
        &self,
        provider: Option<&str>,
        model: Option<&str>,
    ) -> Result<ResolvedProvider, AgentProviderError> {
        let model = model
            .map(str::to_string)
            .or_else(|| (self.env)(ENV_MODEL))
            .map(|model| model.trim().to_string());
        let named = provider
            .map(str::to_string)
            .or_else(|| (self.env)(ENV_PROVIDER));
        if let Some(query) = named {
            return self.saved_provider(&query, model);
        }
        if let Some((key_var, key)) = self.env_key()? {
            return self.environment_provider(key_var, key, model);
        }
        let (settings, agents) = self.saved()?;
        let config = agents.iter().find(|agent| agent.ready()).ok_or_else(|| {
            AgentProviderError::NoProvider {
                settings: settings.clone(),
            }
        })?;
        finish_saved(config, model)
    }

    /// The saved providers, for `--list-providers`: `(settings path, list)`.
    pub(crate) fn saved(&self) -> Result<&(String, Vec<BuiltinAgentConfig>), AgentProviderError> {
        self.saved
            .get_or_init(|| {
                let path = self
                    .flags
                    .settings
                    .clone()
                    .or_else(|| (self.env)(ENV_SETTINGS).map(PathBuf::from))
                    .or_else(settings_io::settings_file_path)
                    .ok_or(AgentProviderError::NoSettingsPath)?;
                let label = path.display().to_string();
                let agents = settings_io::read_builtin_agents(&path).map_err(|error| {
                    AgentProviderError::Settings {
                        path: label.clone(),
                        detail: error.to_string(),
                    }
                })?;
                Ok((label, agents))
            })
            .as_ref()
            .map_err(Clone::clone)
    }

    fn env_key(&self) -> Result<Option<(String, String)>, AgentProviderError> {
        match &self.flags.api_key_env {
            Some(var) => match (self.env)(var) {
                Some(key) => Ok(Some((var.clone(), key))),
                None => Err(AgentProviderError::ApiKeyEnvUnset { var: var.clone() }),
            },
            None => Ok((self.env)(ENV_API_KEY).map(|key| (ENV_API_KEY.to_string(), key))),
        }
    }

    fn saved_provider(
        &self,
        query: &str,
        model: Option<String>,
    ) -> Result<ResolvedProvider, AgentProviderError> {
        let (_, agents) = self.saved()?;
        let config = find_saved(agents, query.trim())?;
        finish_saved(config, model)
    }

    fn environment_provider(
        &self,
        key_var: String,
        key: String,
        model: Option<String>,
    ) -> Result<ResolvedProvider, AgentProviderError> {
        let format = self
            .flags
            .api_format
            .clone()
            .or_else(|| (self.env)(ENV_API_FORMAT))
            .unwrap_or_else(|| "anthropic".to_string());
        let kind = parse_api_format(&format)?;
        let base_url = self
            .flags
            .base_url
            .clone()
            .or_else(|| (self.env)(ENV_BASE_URL))
            .map(|url| url.trim().to_string())
            .unwrap_or_default();
        if !base_url.is_empty() && !is_http_url(&base_url) {
            return Err(AgentProviderError::InvalidBaseUrl { value: base_url });
        }
        let model =
            model
                .filter(|model| !model.is_empty())
                .ok_or(AgentProviderError::MissingModel {
                    key_var: key_var.clone(),
                })?;
        let config = BuiltinAgentConfig {
            id: "environment".into(),
            preset: BuiltinAgentPresetKey::Custom,
            display_name: "environment".into(),
            kind,
            api_key: key,
            models: vec![model.clone()],
            base_url,
            enabled: true,
        };
        Ok(ResolvedProvider {
            config,
            model,
            source: ProviderSource::Environment { key_var },
        })
    }
}

/// `anthropic` or `openai` (also `openai-compat`), case-insensitive.
pub(crate) fn parse_api_format(value: &str) -> Result<BuiltinAgentKind, AgentProviderError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "anthropic" => Ok(BuiltinAgentKind::Anthropic),
        "openai" | "openai-compat" | "openai_compat" => Ok(BuiltinAgentKind::OpenAiCompat),
        _ => Err(AgentProviderError::UnknownApiFormat {
            value: value.to_string(),
        }),
    }
}

fn is_http_url(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

/// Id first, then display name, then preset key (both case-insensitive).
fn find_saved<'a>(
    agents: &'a [BuiltinAgentConfig],
    query: &str,
) -> Result<&'a BuiltinAgentConfig, AgentProviderError> {
    if let Some(agent) = agents.iter().find(|agent| agent.id == query) {
        return Ok(agent);
    }
    let matches_by = |test: &dyn Fn(&BuiltinAgentConfig) -> bool| -> Vec<&'a BuiltinAgentConfig> {
        agents.iter().filter(|agent| test(agent)).collect()
    };
    for found in [
        matches_by(&|agent| agent.display_name.trim().eq_ignore_ascii_case(query)),
        matches_by(&|agent| agent.preset.as_str().eq_ignore_ascii_case(query)),
    ] {
        match found.as_slice() {
            [] => continue,
            [one] => return Ok(one),
            many => {
                return Err(AgentProviderError::AmbiguousProvider {
                    query: query.to_string(),
                    ids: many.iter().map(|agent| agent.id.clone()).collect(),
                })
            }
        }
    }
    Err(AgentProviderError::UnknownProvider {
        query: query.to_string(),
        known: agents
            .iter()
            .map(|agent| format!("{} ({})", agent.display_name.trim(), agent.id))
            .collect(),
    })
}

fn finish_saved(
    config: &BuiltinAgentConfig,
    model: Option<String>,
) -> Result<ResolvedProvider, AgentProviderError> {
    if !config.ready() {
        return Err(AgentProviderError::ProviderNotReady {
            id: config.id.clone(),
        });
    }
    let model = match model.filter(|model| !model.is_empty()) {
        Some(model) if config.has_model(&model) => model,
        Some(model) => {
            return Err(AgentProviderError::ModelNotSaved {
                provider: config.id.clone(),
                model,
                saved: config.models.clone(),
            })
        }
        None => config.first_model().map(str::to_string).ok_or_else(|| {
            AgentProviderError::ProviderNotReady {
                id: config.id.clone(),
            }
        })?,
    };
    Ok(ResolvedProvider {
        config: config.clone(),
        model,
        source: ProviderSource::Saved,
    })
}

#[cfg(test)]
#[path = "agent_provider_tests.rs"]
mod tests;
