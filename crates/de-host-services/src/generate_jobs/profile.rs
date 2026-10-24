//! Which image provider an MCP `generate` call uses.
//!
//! The app is local-only: every provider call rides a key the user entered
//! in Settings. The Images tab's active generation profile wins; without one,
//! a built-in chat agent saved with an OpenAI or Gemini key is reused with
//! that provider's image model. No key means no call — the caller reports
//! [`GenerateJobError::NotConfigured`] and writes nothing.

use de_editor_core::agent_settings::{
    AgentSettings, ImageGenProfile, ImageGenProvider, ImageTestStatus,
};
use de_editor_core::agent_settings_builtin_presets::BuiltinAgentPresetKey;

use super::GenerateJobError;

/// Image model used when an OpenAI chat key stands in for an image profile.
pub const OPENAI_FALLBACK_IMAGE_MODEL: &str = "gpt-image-1";
/// Image model used when a Gemini chat key stands in for an image profile.
pub const GEMINI_FALLBACK_IMAGE_MODEL: &str = "gemini-2.5-flash-image";

/// The image profile a generation should use, or `NotConfigured`.
pub fn resolve_image_profile(
    settings: &AgentSettings,
) -> Result<ImageGenProfile, GenerateJobError> {
    // The popover's own pick: the active profile, else the first one.
    if let Some(profile) = settings
        .active_image_gen_profile()
        .filter(|p| !p.api_key.trim().is_empty() && !p.model.trim().is_empty())
    {
        return Ok(profile.clone());
    }
    for (preset, provider, model) in [
        (
            BuiltinAgentPresetKey::OpenAi,
            ImageGenProvider::OpenAi,
            OPENAI_FALLBACK_IMAGE_MODEL,
        ),
        (
            BuiltinAgentPresetKey::Gemini,
            ImageGenProvider::Gemini,
            GEMINI_FALLBACK_IMAGE_MODEL,
        ),
    ] {
        let Some(agent) = settings.builtin_agents.iter().find(|agent| {
            agent.preset == preset && agent.enabled && !agent.api_key.trim().is_empty()
        }) else {
            continue;
        };
        return Ok(ImageGenProfile {
            id: format!("builtin:{}", agent.id),
            name: agent.display_name.clone(),
            provider,
            api_key: agent.api_key.clone(),
            model: model.to_string(),
            // The chat presets point at chat endpoints (Gemini's OpenAI-compat
            // path); image calls use each provider's own default host.
            base_url: None,
            test_status: ImageTestStatus::Idle,
        });
    }
    Err(GenerateJobError::NotConfigured)
}

#[cfg(test)]
mod tests {
    use super::*;
    use de_editor_core::agent_settings::{BuiltinAgentConfig, BuiltinAgentKind};

    fn agent(preset: BuiltinAgentPresetKey, key: &str) -> BuiltinAgentConfig {
        BuiltinAgentConfig {
            id: preset.as_str().to_string(),
            preset,
            display_name: preset.as_str().to_string(),
            kind: BuiltinAgentKind::OpenAiCompat,
            api_key: key.to_string(),
            models: vec!["chat-model".into()],
            base_url: "https://example.invalid/v1".into(),
            enabled: true,
        }
    }

    #[test]
    fn no_key_is_not_configured() {
        let settings = AgentSettings::default();
        assert_eq!(
            resolve_image_profile(&settings),
            Err(GenerateJobError::NotConfigured)
        );
    }

    #[test]
    fn first_image_profile_wins_over_chat_keys() {
        let mut settings = AgentSettings::default();
        settings
            .builtin_agents
            .push(agent(BuiltinAgentPresetKey::OpenAi, "sk-chat"));
        settings.image_gen_profiles.push(ImageGenProfile {
            id: "p1".into(),
            name: "Mine".into(),
            provider: ImageGenProvider::Replicate,
            api_key: "r8".into(),
            model: "flux".into(),
            base_url: None,
            test_status: ImageTestStatus::Idle,
        });
        let profile = resolve_image_profile(&settings).expect("profile");
        assert_eq!(profile.provider, ImageGenProvider::Replicate);
        assert_eq!(profile.api_key, "r8");
    }

    #[test]
    fn openai_chat_key_is_preferred_and_falls_back_to_gpt_image() {
        let mut settings = AgentSettings::default();
        settings
            .builtin_agents
            .push(agent(BuiltinAgentPresetKey::Gemini, "g-key"));
        settings
            .builtin_agents
            .push(agent(BuiltinAgentPresetKey::OpenAi, "sk-chat"));
        let profile = resolve_image_profile(&settings).expect("profile");
        assert_eq!(profile.provider, ImageGenProvider::OpenAi);
        assert_eq!(profile.model, OPENAI_FALLBACK_IMAGE_MODEL);
        assert_eq!(profile.base_url, None);
    }

    #[test]
    fn gemini_chat_key_falls_back_to_gemini_image() {
        let mut settings = AgentSettings::default();
        settings
            .builtin_agents
            .push(agent(BuiltinAgentPresetKey::Gemini, "g-key"));
        let profile = resolve_image_profile(&settings).expect("profile");
        assert_eq!(profile.provider, ImageGenProvider::Gemini);
        assert_eq!(profile.model, GEMINI_FALLBACK_IMAGE_MODEL);
    }

    #[test]
    fn disabled_or_keyless_agents_are_skipped() {
        let mut settings = AgentSettings::default();
        let mut disabled = agent(BuiltinAgentPresetKey::OpenAi, "sk");
        disabled.enabled = false;
        settings.builtin_agents.push(disabled);
        settings
            .builtin_agents
            .push(agent(BuiltinAgentPresetKey::Gemini, "  "));
        assert_eq!(
            resolve_image_profile(&settings),
            Err(GenerateJobError::NotConfigured)
        );
    }
}
