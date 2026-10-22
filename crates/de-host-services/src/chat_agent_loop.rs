//! Re-export shim: the tool-executing agent loop moved to `de-chat-agent`
//! (pure code motion); every
//! existing `de_host_services::chat_agent_loop::…` path stays valid.

pub use de_chat_agent::chat_agent_loop::*;
