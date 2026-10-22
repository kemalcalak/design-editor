use super::*;

#[test]
fn tabs_and_more_menu_cover_every_agent_once() {
    let mut seen: Vec<McpCli> = HOME_MCP_TABS.to_vec();
    seen.extend(home_mcp_more_agents());
    assert_eq!(seen.len(), McpCli::ALL.len());
    for cli in McpCli::ALL {
        assert_eq!(seen.iter().filter(|c| **c == cli).count(), 1, "{cli:?}");
    }
}

#[test]
fn snippets_point_at_the_live_port() {
    for cli in McpCli::ALL {
        let snippet = home_mcp_manual_snippet(cli, "http://127.0.0.1:4321/mcp");
        assert!(
            snippet.contains("http://127.0.0.1:4321/mcp"),
            "{cli:?}: {snippet}"
        );
        assert!(snippet.contains("design-editor"), "{cli:?}: {snippet}");
    }
}

#[test]
fn claude_code_gets_the_cli_command_and_codex_the_toml_table() {
    assert_eq!(
        home_mcp_manual_snippet(McpCli::ClaudeCode, "http://127.0.0.1:3100/mcp"),
        "claude mcp add --transport http design-editor http://127.0.0.1:3100/mcp"
    );
    assert_eq!(
        home_mcp_manual_snippet(McpCli::Codex, "http://127.0.0.1:3100/mcp"),
        "[mcp_servers.design-editor]\nurl = \"http://127.0.0.1:3100/mcp\""
    );
    assert_eq!(home_mcp_config_path(McpCli::Codex), "~/.codex/config.toml");
}

#[test]
fn snippets_wrap_at_the_column_count_and_keep_their_breaks() {
    let lines = wrap_snippet("abcdefghij\nxy", 8);
    assert_eq!(lines, vec!["abcdefgh", "ij", "xy"]);
    assert_eq!(wrap_snippet("a\n\nb", 20), vec!["a", "", "b"]);
}
