//! State-layer mirror of the AI chat panel's hoverable buttons.
//!
//! The chat panel's full hit enum (`de_editor_ui::widgets::AIChatHit`)
//! carries owned `String` payloads, so it is not `Copy` and can't live
//! on `EditorUiState`. These small `Copy` enums capture just the chat
//! controls that need hover paint state — same wasm32-clean discipline
//! as `chrome_button_state` / `statusbar_state`.

/// Which bare header button of the AI chat panel the cursor is over.
/// `None` on `EditorUiState.chat_header_hover` = no hover wash. The
/// send / attach / model / effort controls already paint their own
/// backgrounds and so are intentionally excluded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatHeaderButton {
    /// Chevron at the top-left — collapses the panel to a pill.
    ToggleCollapse,
    /// Maximize / restore glyph in the header.
    ToggleMaximize,
    /// Plus glyph in the header — starts a new chat.
    NewChat,
    /// Session select — opens the list of this file's chats.
    SessionMenu,
}

/// Which footer control of the AI chat panel the cursor is over.
/// `None` on `EditorUiState.chat_footer_hover` = no hover wash.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatFooterButton {
    /// The "+" button — opens the menu holding attach, the style library,
    /// the thinking mode and the parallel-agents count.
    PlusMenu,
    /// "model · provider" chip — opens the model picker.
    ModelPicker,
    /// Send button in its normal idle state.
    Send,
    /// Stop button shown while a response streams.
    Stop,
}
