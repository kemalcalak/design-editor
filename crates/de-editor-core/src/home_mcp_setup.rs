//! Per-agent copy for Home's "Connect an agent" popover: the agent tabs,
//! the config file each agent reads, and the snippet a user pastes when
//! they set the agent up by hand.
//!
//! Shared by Home's "Connect an agent" popover and Settings ▸ MCP. The
//! automatic path does not live here: "Set up" calls
//! `AgentSettings::set_mcp_cli_enabled`, and the desktop's
//! `mcp_integrations` writer reconciles the file. The snippets below
//! restate the entry that writer emits (a desktop test pins the paths to
//! these), so the manual and the automatic routes leave the same thing
//! behind.

use crate::agent_settings::McpCli;

/// The agents that get a tab of their own; the rest sit behind the ▾ menu
/// in `McpCli::DISPLAY` order.
pub const HOME_MCP_TABS: [McpCli; 5] = [
    McpCli::ClaudeCode,
    McpCli::Codex,
    McpCli::Cursor,
    McpCli::GeminiCli,
    McpCli::OpenCode,
];

/// The agents listed in the ▾ menu, in Settings ▸ MCP row order.
pub fn home_mcp_more_agents() -> impl Iterator<Item = McpCli> {
    McpCli::DISPLAY
        .into_iter()
        .filter(|cli| !HOME_MCP_TABS.contains(cli))
}

/// The short name a tab or a "Add to …" title shows. `McpCli::label`
/// carries a "CLI" suffix that only reads well in the Settings grid.
pub const fn home_mcp_agent_name(cli: McpCli) -> &'static str {
    match cli {
        McpCli::ClaudeCode => "Claude Code",
        McpCli::Codex => "Codex",
        McpCli::OpenCode => "OpenCode",
        McpCli::Kiro => "Kiro",
        McpCli::GithubCopilot => "Copilot",
        McpCli::Antigravity => "Antigravity",
        McpCli::GrokBuild => "Grok Build",
        McpCli::GeminiCli => "Gemini CLI",
        McpCli::QwenCode => "Qwen Code",
        McpCli::Cursor => "Cursor",
        McpCli::Kimi => "Kimi",
        McpCli::ZCode => "ZCode",
        McpCli::Dsh => "DeepSeek Harness",
    }
}

/// The config file the automatic set-up writes, as the user would type
/// it. Mirrors the desktop's `mcp_integrations::config_path` defaults
/// (environment overrides such as `CODEX_HOME` are not reflected).
pub const fn home_mcp_config_path(cli: McpCli) -> &'static str {
    match cli {
        McpCli::ClaudeCode => "~/.claude.json",
        McpCli::Codex => "~/.codex/config.toml",
        McpCli::OpenCode => "~/.config/opencode/opencode.json",
        McpCli::Kiro => "~/.kiro/settings/mcp.json",
        McpCli::GithubCopilot => "~/.config/github-copilot/mcp.json",
        McpCli::Antigravity => "~/.gemini/config/mcp_config.json",
        McpCli::GrokBuild => "~/.grok/config.toml",
        McpCli::GeminiCli => "~/.gemini/settings.json",
        McpCli::QwenCode => "~/.qwen/settings.json",
        McpCli::Cursor => "~/.cursor/mcp.json",
        McpCli::Kimi => "~/.kimi-code/mcp.json",
        McpCli::ZCode => "~/.zcode/cli/config.json",
        McpCli::Dsh => "~/.dsh/cordis.patch.yml",
    }
}

/// What a user pastes to add Design Editor to `cli` by hand, pointing at
/// `url` (`AgentSettings::mcp_endpoint_url`). Claude Code gets its own
/// `claude mcp add` command; every other agent gets the entry its config
/// file expects.
pub fn home_mcp_manual_snippet(cli: McpCli, url: &str) -> String {
    match cli {
        McpCli::ClaudeCode => {
            format!("claude mcp add --transport http design-editor {url}")
        }
        McpCli::Codex | McpCli::GrokBuild => {
            format!("[mcp_servers.design-editor]\nurl = \"{url}\"")
        }
        McpCli::OpenCode => format!(
            "\"mcp\": {{\n  \"design-editor\": {{ \"type\": \"remote\", \"url\": \"{url}\", \"enabled\": true }}\n}}"
        ),
        McpCli::Kiro => format!(
            "\"mcpServers\": {{\n  \"design-editor\": {{ \"url\": \"{url}\", \"disabled\": false }}\n}}"
        ),
        McpCli::Kimi => format!(
            "\"mcpServers\": {{\n  \"design-editor\": {{ \"url\": \"{url}\", \"transport\": \"http\" }}\n}}"
        ),
        McpCli::QwenCode => format!(
            "\"mcpServers\": {{\n  \"design-editor\": {{ \"httpUrl\": \"{url}\" }}\n}}"
        ),
        McpCli::Antigravity => format!(
            "\"mcpServers\": {{\n  \"design-editor\": {{ \"serverUrl\": \"{url}\" }}\n}}"
        ),
        McpCli::ZCode => format!(
            "\"mcp\": {{ \"servers\": {{\n  \"design-editor\": {{ \"type\": \"http\", \"url\": \"{url}\" }}\n}} }}"
        ),
        // The markers are what lets the writer recognise (and remove) the
        // block later; without them a pasted entry is refused as manual.
        McpCli::Dsh => format!(
            "# design-editor-mcp-begin (managed by Design Editor; do not edit)\n- insert:\n    - id: mcp-design-editor\n      name: '@deepseek-ai/dsh-mcp-client'\n      config:\n        serverName: design-editor\n        transport: streamable-http\n        url: {url}\n# design-editor-mcp-end"
        ),
        McpCli::GithubCopilot | McpCli::GeminiCli | McpCli::Cursor => format!(
            "\"mcpServers\": {{\n  \"design-editor\": {{ \"type\": \"http\", \"url\": \"{url}\" }}\n}}"
        ),
    }
}

/// Hard-wrap a snippet into lines of at most `columns` characters,
/// keeping its own line breaks. Snippets render in a monospace face, so a
/// column count is an exact width.
pub fn wrap_snippet(text: &str, columns: usize) -> Vec<String> {
    let columns = columns.max(8);
    let mut lines = Vec::new();
    for raw in text.lines() {
        let chars: Vec<char> = raw.chars().collect();
        if chars.is_empty() {
            lines.push(String::new());
        }
        for chunk in chars.chunks(columns) {
            lines.push(chunk.iter().collect());
        }
    }
    lines
}

#[cfg(test)]
#[path = "home_mcp_setup_tests.rs"]
mod tests;
