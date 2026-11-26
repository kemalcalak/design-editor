//! Read-only access to the saved built-in providers, for processes that use
//! the app's provider settings without being the app (the `de agent` CLI).
//!
//! Nothing here writes: no corrupt-file backup, no save pinning, no locale
//! seeding. The file is parsed with the same payload shape and the same
//! dedupe the app's own load applies, so the CLI sees the providers the
//! Agents settings show.

use std::path::{Path, PathBuf};

use de_editor_core::BuiltinAgentConfig;

use super::{SettingsIoError, SettingsPayload, SETTINGS_VERSION};
use crate::settings_payload::{builtin_agent_from_payload, dedupe_builtin_agents};

/// The `settings.json` the app reads and writes (`None` when the platform
/// has no config directory).
pub fn settings_file_path() -> Option<PathBuf> {
    super::settings_path()
}

/// The built-in providers saved in the settings file at `path`, in the
/// order the app lists them. A missing file is a first run: no providers.
pub fn read_builtin_agents(path: &Path) -> Result<Vec<BuiltinAgentConfig>, SettingsIoError> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(SettingsIoError::Read {
                detail: error.to_string(),
            })
        }
    };
    let payload: SettingsPayload =
        serde_json::from_slice(&bytes).map_err(|error| SettingsIoError::Parse {
            detail: error.to_string(),
        })?;
    if payload.version != SETTINGS_VERSION {
        return Err(SettingsIoError::UnsupportedVersion {
            found: payload.version,
            expected: SETTINGS_VERSION,
        });
    }
    let agents = payload
        .builtin_agents
        .unwrap_or_default()
        .into_iter()
        .filter_map(builtin_agent_from_payload)
        .collect();
    Ok(dedupe_builtin_agents(agents))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_file(tag: &str, contents: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "de-settings-providers-{tag}-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join("settings.json");
        std::fs::write(&path, contents).expect("write settings");
        path
    }

    #[test]
    fn a_missing_file_has_no_providers() {
        let path = std::env::temp_dir().join("de-settings-providers-missing/settings.json");
        assert_eq!(read_builtin_agents(&path), Ok(Vec::new()));
    }

    #[test]
    fn saved_providers_are_read_in_order() {
        let path = temp_file(
            "ok",
            r#"{"version":1,"builtin_agents":[
                {"id":"builtin-1","display_name":"Claude","kind":"anthropic","api_key":"k1","model":"claude-x","base_url":"","enabled":true},
                {"id":"builtin-2","preset":"openai","display_name":"OpenAI","kind":"openai-compat","api_key":"k2","models":["gpt-a","gpt-b"],"base_url":"https://api.openai.com/v1","enabled":false}
            ]}"#,
        );
        let agents = read_builtin_agents(&path).expect("read");
        assert_eq!(agents.len(), 2);
        assert_eq!(agents[0].id, "builtin-1");
        assert_eq!(agents[0].models, vec!["claude-x".to_string()]);
        assert!(agents[0].ready());
        assert_eq!(agents[1].models.len(), 2);
        assert!(!agents[1].ready());
    }

    #[test]
    fn unreadable_json_and_foreign_versions_are_errors() {
        let bad = temp_file("bad", "{ not json");
        assert!(matches!(
            read_builtin_agents(&bad),
            Err(SettingsIoError::Parse { .. })
        ));
        let future = temp_file("future", r#"{"version":99}"#);
        assert_eq!(
            read_builtin_agents(&future),
            Err(SettingsIoError::UnsupportedVersion {
                found: 99,
                expected: SETTINGS_VERSION,
            })
        );
    }
}
