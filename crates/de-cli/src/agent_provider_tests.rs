use std::collections::BTreeMap;
use std::path::PathBuf;

use super::*;

const SETTINGS: &str = r#"{"version":1,"builtin_agents":[
  {"id":"builtin-1","preset":"anthropic","display_name":"Claude","kind":"anthropic","api_key":"sk-saved-1","models":["claude-a","claude-b"],"base_url":"","enabled":true},
  {"id":"builtin-2","preset":"openai","display_name":"Work OpenAI","kind":"openai-compat","api_key":"sk-saved-2","model":"gpt-x","base_url":"https://api.openai.com/v1","enabled":true},
  {"id":"builtin-3","preset":"custom","display_name":"Off","kind":"anthropic","api_key":"","model":"m","base_url":"","enabled":true}
]}"#;

fn settings_file(tag: &str, contents: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("de-agent-provider-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("settings.json");
    std::fs::write(&path, contents).unwrap();
    path
}

fn resolver(tag: &str, flags: ProviderFlags, env: &[(&str, &str)]) -> ProviderResolver {
    let env: BTreeMap<String, String> = env
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    let flags = ProviderFlags {
        settings: flags
            .settings
            .or_else(|| Some(settings_file(tag, SETTINGS))),
        ..flags
    };
    ProviderResolver::new(flags, Box::new(move |name| env.get(name).cloned()))
}

#[test]
fn without_names_or_keys_the_first_ready_saved_provider_runs_its_first_model() {
    let resolved = resolver("first", ProviderFlags::default(), &[])
        .resolve(None, None)
        .unwrap();
    assert_eq!(resolved.config.id, "builtin-1");
    assert_eq!(resolved.model, "claude-a");
    assert_eq!(resolved.source, ProviderSource::Saved);
    assert_eq!(resolved.label(), "Claude (builtin-1)");
}

#[test]
fn a_saved_provider_is_found_by_id_name_or_preset() {
    let r = resolver("find", ProviderFlags::default(), &[]);
    for query in ["builtin-2", "work openai", "OPENAI"] {
        let resolved = r.resolve(Some(query), None).unwrap();
        assert_eq!(resolved.config.id, "builtin-2", "{query}");
        assert_eq!(resolved.model, "gpt-x");
    }
    assert_eq!(
        r.resolve(Some("builtin-1"), Some("claude-b"))
            .unwrap()
            .model,
        "claude-b"
    );
}

#[test]
fn saved_provider_errors_are_specific_and_keyless() {
    let r = resolver("errors", ProviderFlags::default(), &[]);
    let unknown = r.resolve(Some("gemini"), None).unwrap_err();
    assert!(matches!(
        unknown,
        AgentProviderError::UnknownProvider { .. }
    ));
    assert!(unknown.to_string().contains("Claude (builtin-1)"));
    assert_eq!(
        r.resolve(Some("builtin-3"), None).unwrap_err(),
        AgentProviderError::ProviderNotReady {
            id: "builtin-3".into()
        }
    );
    let not_saved = r.resolve(Some("Claude"), Some("claude-z")).unwrap_err();
    assert_eq!(
        not_saved.to_string(),
        "model \"claude-z\" is not saved on provider builtin-1 (saved: claude-a, claude-b)"
    );
    for error in [unknown, not_saved] {
        assert!(!error.to_string().contains("sk-saved"));
    }
}

#[test]
fn two_providers_with_one_name_are_ambiguous() {
    let path = settings_file(
        "ambiguous",
        r#"{"version":1,"builtin_agents":[
          {"id":"builtin-1","display_name":"Same","kind":"anthropic","api_key":"k","model":"a","base_url":"","enabled":true},
          {"id":"builtin-2","display_name":"same","kind":"anthropic","api_key":"k2","model":"b","base_url":"https://example.com","enabled":true}
        ]}"#,
    );
    let r = resolver(
        "ambiguous",
        ProviderFlags {
            settings: Some(path),
            ..ProviderFlags::default()
        },
        &[],
    );
    assert_eq!(
        r.resolve(Some("SAME"), None).unwrap_err(),
        AgentProviderError::AmbiguousProvider {
            query: "SAME".into(),
            ids: vec!["builtin-1".into(), "builtin-2".into()],
        }
    );
}

#[test]
fn an_environment_key_wins_over_saved_providers_and_needs_a_model() {
    let r = resolver("env", ProviderFlags::default(), &[(ENV_API_KEY, "sk-env")]);
    assert_eq!(
        r.resolve(None, None).unwrap_err(),
        AgentProviderError::MissingModel {
            key_var: ENV_API_KEY.into()
        }
    );
    let resolved = r.resolve(None, Some("claude-ci")).unwrap();
    assert_eq!(
        resolved.source,
        ProviderSource::Environment {
            key_var: ENV_API_KEY.into()
        }
    );
    assert_eq!(resolved.config.kind, BuiltinAgentKind::Anthropic);
    assert_eq!(resolved.config.api_key, "sk-env");
    assert_eq!(resolved.config.base_url, "");
    assert_eq!(resolved.model, "claude-ci");
    assert_eq!(resolved.label(), "environment (DE_AGENT_API_KEY)");
    assert!(!format!("{resolved:?}").contains("sk-env"));
}

#[test]
fn environment_settings_come_from_flags_then_variables() {
    let env = [
        ("MY_KEY", "sk-mine"),
        (ENV_API_FORMAT, "openai"),
        (ENV_BASE_URL, "http://127.0.0.1:9/v1"),
        (ENV_MODEL, "env-model"),
    ];
    let from_env = resolver(
        "env-vars",
        ProviderFlags {
            api_key_env: Some("MY_KEY".into()),
            ..ProviderFlags::default()
        },
        &env,
    )
    .resolve(None, None)
    .unwrap();
    assert_eq!(from_env.config.kind, BuiltinAgentKind::OpenAiCompat);
    assert_eq!(from_env.config.base_url, "http://127.0.0.1:9/v1");
    assert_eq!(from_env.model, "env-model");

    let from_flags = resolver(
        "env-flags",
        ProviderFlags {
            api_key_env: Some("MY_KEY".into()),
            api_format: Some("anthropic".into()),
            base_url: Some("https://proxy.example".into()),
            ..ProviderFlags::default()
        },
        &env,
    )
    .resolve(None, Some("flag-model"))
    .unwrap();
    assert_eq!(from_flags.config.kind, BuiltinAgentKind::Anthropic);
    assert_eq!(from_flags.config.base_url, "https://proxy.example");
    assert_eq!(from_flags.model, "flag-model");
}

#[test]
fn a_named_provider_beats_an_environment_key() {
    let r = resolver(
        "named",
        ProviderFlags::default(),
        &[(ENV_API_KEY, "sk-env"), (ENV_PROVIDER, "builtin-2")],
    );
    // A name from the task or --provider comes first...
    assert_eq!(
        r.resolve(Some("builtin-1"), None).unwrap().config.id,
        "builtin-1"
    );
    // ...and DE_AGENT_PROVIDER is a name too, so it beats the key.
    assert_eq!(r.resolve(None, None).unwrap().config.id, "builtin-2");
    let key_only = resolver(
        "key-only",
        ProviderFlags::default(),
        &[(ENV_API_KEY, "sk-env")],
    );
    assert_eq!(
        key_only.resolve(None, Some("m")).unwrap().source,
        ProviderSource::Environment {
            key_var: ENV_API_KEY.into()
        }
    );
}

#[test]
fn environment_input_is_checked() {
    let unset = resolver(
        "unset",
        ProviderFlags {
            api_key_env: Some("NOPE".into()),
            ..ProviderFlags::default()
        },
        &[],
    );
    assert_eq!(
        unset.resolve(None, Some("m")).unwrap_err(),
        AgentProviderError::ApiKeyEnvUnset { var: "NOPE".into() }
    );
    let bad_format = resolver(
        "format",
        ProviderFlags {
            api_format: Some("gemini".into()),
            ..ProviderFlags::default()
        },
        &[(ENV_API_KEY, "k")],
    );
    assert_eq!(
        bad_format.resolve(None, Some("m")).unwrap_err().to_string(),
        "unknown API format \"gemini\" (use anthropic or openai)"
    );
    let bad_url = resolver(
        "url",
        ProviderFlags {
            base_url: Some("ftp://x".into()),
            ..ProviderFlags::default()
        },
        &[(ENV_API_KEY, "k")],
    );
    assert!(matches!(
        bad_url.resolve(None, Some("m")).unwrap_err(),
        AgentProviderError::InvalidBaseUrl
    ));
}

#[test]
fn no_ready_provider_and_unreadable_settings_are_reported() {
    let empty = settings_file("empty", r#"{"version":1}"#);
    let r = resolver(
        "empty",
        ProviderFlags {
            settings: Some(empty.clone()),
            ..ProviderFlags::default()
        },
        &[],
    );
    assert_eq!(
        r.resolve(None, None).unwrap_err(),
        AgentProviderError::NoProvider {
            settings: empty.display().to_string()
        }
    );
    let broken = settings_file("broken", "{");
    let r = resolver(
        "broken",
        ProviderFlags {
            settings: Some(broken),
            ..ProviderFlags::default()
        },
        &[],
    );
    assert!(matches!(
        r.resolve(None, None).unwrap_err(),
        AgentProviderError::Settings { .. }
    ));
}

#[test]
fn the_settings_path_can_come_from_the_environment() {
    let path = settings_file("env-path", SETTINGS);
    let env: BTreeMap<String, String> =
        BTreeMap::from([(ENV_SETTINGS.to_string(), path.display().to_string())]);
    let r = ProviderResolver::new(
        ProviderFlags::default(),
        Box::new(move |name| env.get(name).cloned()),
    );
    assert_eq!(r.saved().unwrap().1.len(), 3);
}

#[test]
fn errors_never_echo_values_that_may_be_secrets() {
    // A key passed where the variable name belongs is not repeated.
    let key_as_name = resolver(
        "key-as-name",
        ProviderFlags {
            api_key_env: Some("sk-ant-api03-SECRETSECRET".into()),
            ..ProviderFlags::default()
        },
        &[],
    )
    .resolve(None, Some("m"))
    .unwrap_err()
    .to_string();
    assert!(!key_as_name.contains("SECRET"), "{key_as_name}");
    assert!(
        key_as_name.contains("25 characters, not shown"),
        "{key_as_name}"
    );
    // A real variable name is fine to show.
    let named = AgentProviderError::ApiKeyEnvUnset {
        var: "CI_ANTHROPIC_KEY".into(),
    };
    assert!(named.to_string().contains("CI_ANTHROPIC_KEY"));

    // A base URL can carry credentials.
    let url = resolver(
        "url-secret",
        ProviderFlags {
            base_url: Some("ftp://user:hunter2@proxy?key=SECRET".into()),
            ..ProviderFlags::default()
        },
        &[(ENV_API_KEY, "k")],
    )
    .resolve(None, Some("m"))
    .unwrap_err()
    .to_string();
    assert!(!url.contains("hunter2") && !url.contains("SECRET"), "{url}");

    // serde_json quotes the offending value; only the position is kept.
    let broken = settings_file(
        "secret-parse",
        r#"{"version":1,"builtin_agents":[{"id":"b","display_name":"x","kind":"anthropic","api_key":"k","base_url":"","enabled":"sk-LEAK"}]}"#,
    );
    let settings = resolver(
        "secret-parse",
        ProviderFlags {
            settings: Some(broken),
            ..ProviderFlags::default()
        },
        &[],
    )
    .resolve(None, None)
    .unwrap_err()
    .to_string();
    assert!(!settings.contains("sk-LEAK"), "{settings}");
    assert!(
        settings.contains("not valid settings JSON (line 1 column "),
        "{settings}"
    );
}

#[test]
fn variable_names_are_recognised() {
    use crate::agent_provider_error::looks_like_variable_name;
    for good in ["DE_AGENT_API_KEY", "_x", "a1"] {
        assert!(looks_like_variable_name(good), "{good}");
    }
    for bad in ["", "1A", "sk-abc", "has space", "ключ"] {
        assert!(!looks_like_variable_name(bad), "{bad}");
    }
}
