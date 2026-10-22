//! Labels for the chat's thinking mode, which cycles
//! [`ChatState::thinking_mode`] Adaptive → Disabled → Enabled → Adaptive
//! from its row in the composer's "+" menu (#26).
//!
//! The state is the *user's* choice. A model whose profile is
//! `thinking_disabled` (glm-5.x, MiniMax) is forced off at turn launch by
//! `design_turn_thinking_mode` no matter what the row shows.
//!
//! [`ChatState::thinking_mode`]: de_editor_core::chat::ChatState::thinking_mode

use de_editor_core::chat::ThinkingMode;

/// Translation key for the "+" menu row in `mode` ("Thinking: Auto").
pub(crate) fn thinking_tooltip_key(mode: ThinkingMode) -> &'static str {
    match mode {
        ThinkingMode::Adaptive => "ai.thinking.adaptive",
        ThinkingMode::Disabled => "ai.thinking.disabled",
        ThinkingMode::Enabled => "ai.thinking.enabled",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_mode_has_its_own_label_key() {
        let keys = [
            ThinkingMode::Adaptive,
            ThinkingMode::Disabled,
            ThinkingMode::Enabled,
        ]
        .map(thinking_tooltip_key);
        assert_ne!(keys[0], keys[1]);
        assert_ne!(keys[1], keys[2]);
        assert_ne!(keys[0], keys[2]);
    }
}
