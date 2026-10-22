//! Geometry and hit-test contract for the v2 MCP tab.

use super::*;

fn content() -> Rect {
    Rect::xywh(300.0, 150.0, 620.0, 600.0)
}

fn center(rect: Rect) -> Point2D {
    Point2D::new(
        rect.origin.x + rect.size.x / 2.0,
        rect.origin.y + rect.size.y / 2.0,
    )
}

#[test]
fn the_collapsed_list_shows_six_agents_then_every_agent() {
    let mut settings = AgentSettings::default();
    assert_eq!(agent_rows(content(), &settings).len(), COLLAPSED_ROWS);
    settings.mcp_show_all_agents = true;
    assert_eq!(
        agent_rows(content(), &settings).len(),
        McpCli::DISPLAY.len()
    );
}

#[test]
fn rows_route_set_up_expand_and_copy() {
    let mut settings = AgentSettings::default();
    let rows = agent_rows(content(), &settings);
    let codex = rows[1];
    assert_eq!(
        hit_test(content(), &settings, center(codex.action), true),
        McpHit::ToggleCli(codex.cli)
    );
    assert_eq!(
        hit_test(content(), &settings, center(codex.row), true),
        McpHit::ExpandCli(codex.cli)
    );

    settings.mcp_expanded_agent = Some(codex.cli);
    let rows = agent_rows(content(), &settings);
    let open = rows[1];
    assert!(open.snippet.size.y > 0.0);
    assert_eq!(
        hit_test(content(), &settings, center(open.snippet_copy), true),
        McpHit::CopySnippet(open.cli)
    );
    // The expanded snippet pushes the next row down.
    assert!(rows[2].row.origin.y >= open.snippet.origin.y + open.snippet.size.y);
}

#[test]
fn the_server_card_routes_its_controls() {
    let mut settings = AgentSettings::default();
    assert_eq!(
        hit_test(
            content(),
            &settings,
            center(server_button_rect(content())),
            true
        ),
        McpHit::ToggleServer
    );
    assert_eq!(
        hit_test(
            content(),
            &settings,
            center(client_config_copy_button_rect(content(), &settings)),
            true
        ),
        McpHit::CopyClientConfig
    );
    assert_eq!(
        hit_test(
            content(),
            &settings,
            center(port_field_rect(content())),
            true
        ),
        McpHit::FocusPort
    );
    settings.mcp_server.running = true;
    assert_ne!(
        hit_test(
            content(),
            &settings,
            center(port_field_rect(content())),
            true
        ),
        McpHit::FocusPort,
        "the port is read-only while the server runs"
    );
}

#[test]
fn without_external_clis_the_list_is_gone() {
    let settings = AgentSettings::default();
    let row = agent_rows(content(), &settings)[0];
    assert_eq!(
        hit_test(content(), &settings, center(row.action), false),
        McpHit::None
    );
    assert!(
        connected_rect(content(), &settings, false).origin.y
            < connected_rect(content(), &settings, true).origin.y
    );
}

#[test]
fn content_height_grows_with_the_full_list() {
    let mut settings = AgentSettings::default();
    let collapsed = content_height(&settings, true);
    settings.mcp_show_all_agents = true;
    assert!(content_height(&settings, true) > collapsed);
}
