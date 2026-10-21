//! Re-export shim: the chat CRUD canvas tool surface moved to
//! `de-chat-agent` (pure code motion); every existing
//! `de_host_services::chat_canvas_tools::…` path stays valid.

pub use de_chat_agent::chat_canvas_tools::*;
