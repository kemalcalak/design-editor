//! Shim: the MCP tool schema catalog moved to `de_chat_agent::tool_schemas`
//! (pure code motion) so the design-agent tool defs derive from it on every
//! host; existing `mcp_serve::schemas::…` paths stay valid.

pub use de_chat_agent::tool_schemas::*;
