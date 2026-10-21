//! Re-export shim: the unresolved-blocker scan moved to `de-chat-agent`
//! (pure code motion); existing paths stay valid.

pub use de_chat_agent::loop_blocker_ledger::*;
