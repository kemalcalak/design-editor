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
        hit_test(content(), &settings, Locale::EnUs, center(codex.action)),
        McpHit::ToggleCli(codex.cli)
    );
    assert_eq!(
        hit_test(content(), &settings, Locale::EnUs, center(codex.row)),
        McpHit::ExpandCli(codex.cli)
    );

    settings.mcp_expanded_agent = Some(codex.cli);
    let rows = agent_rows(content(), &settings);
    let open = rows[1];
    assert!(open.snippet.size.y > 0.0);
    assert_eq!(
        hit_test(
            content(),
            &settings,
            Locale::EnUs,
            center(snippet_copy_rect(open.snippet, Locale::EnUs))
        ),
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
            Locale::EnUs,
            center(server_button_rect(content()))
        ),
        McpHit::ToggleServer
    );
    assert_eq!(
        hit_test(
            content(),
            &settings,
            Locale::EnUs,
            center(client_config_copy_button_rect(
                content(),
                &settings,
                Locale::EnUs
            ))
        ),
        McpHit::CopyClientConfig
    );
    assert_eq!(
        hit_test(
            content(),
            &settings,
            Locale::EnUs,
            center(port_field_rect(content()))
        ),
        McpHit::FocusPort
    );
    settings.mcp_server.running = true;
    assert_ne!(
        hit_test(
            content(),
            &settings,
            Locale::EnUs,
            center(port_field_rect(content()))
        ),
        McpHit::FocusPort,
        "the port is read-only while the server runs"
    );
}

#[test]
fn content_height_grows_with_the_full_list() {
    let mut settings = AgentSettings::default();
    let collapsed = content_height(&settings);
    settings.mcp_show_all_agents = true;
    assert!(content_height(&settings) > collapsed);
}

/// The wider Copy-chip label in `locale`, by the same estimate the chip is
/// sized from (hit-test has no backend).
fn label_w(locale: Locale) -> f32 {
    let w = |key| {
        crate::widgets::home_surface::estimate_width(
            de_i18n::translate(locale, key),
            COPY_CHIP_FONT,
        )
    };
    w("home.mcp.copy").max(w("home.mcp.copied"))
}

fn label_run(locale: Locale) -> f32 {
    COPY_CHIP_PAD + COPY_CHIP_ICON + COPY_CHIP_GAP + label_w(locale) + COPY_CHIP_PAD
}

#[test]
fn the_copy_chip_fits_its_label_in_every_locale() {
    let settings = AgentSettings::default();
    for locale in Locale::ALL {
        let chip = client_config_copy_button_rect(content(), &settings, locale);
        assert!(
            chip.size.x >= label_run(locale),
            "{locale:?}: chip {} < label run {}",
            chip.size.x,
            label_run(locale)
        );
        // The chip sits inside its endpoint field, which stays in the card.
        let field = endpoint_field_rect(content(), &settings.mcp_endpoint_url(), locale);
        assert!(chip.origin.x >= field.origin.x);
        assert!(chip.origin.x + chip.size.x <= field.origin.x + field.size.x);
        let card = server_card_rect(content());
        assert!(field.origin.x + field.size.x <= card.origin.x + card.size.x);
    }
}

#[test]
fn the_longest_locale_label_fits_and_turkish_no_longer_overflows() {
    let longest = Locale::ALL
        .into_iter()
        .max_by(|a, b| label_w(*a).total_cmp(&label_w(*b)))
        .expect("locales");
    let settings = AgentSettings::default();
    for locale in [longest, Locale::Tr] {
        let chip = client_config_copy_button_rect(content(), &settings, locale);
        assert!(chip.size.x >= label_run(locale));
        // The snippet chip uses the same width, and the snippet box
        // reserves room for the widest one.
        let snippet = Rect::xywh(0.0, 0.0, 400.0, 40.0);
        assert_eq!(snippet_copy_rect(snippet, locale).size.x, chip.size.x);
        assert!(copy_chip_reserve() >= chip.size.x);
    }
    // "Kopyala" ran past the old fixed 58 px chip.
    assert!(
        client_config_copy_button_rect(content(), &settings, Locale::Tr)
            .size
            .x
            > 58.0
    );
}
