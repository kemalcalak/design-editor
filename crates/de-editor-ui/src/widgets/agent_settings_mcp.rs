//! MCP tab of the settings modal (issue #22, Settings v2).
//!
//! Top to bottom: the tab intro, a server card (status, endpoint + Copy,
//! port, Start / Stop), the AGENTS list — one row per external agent with
//! Set up / Remove and an expandable manual snippet — and the last MCP
//! client that connected. Set up flips the same `mcp_cli_enabled` slot the
//! Home "Connect an agent" popover does, and the desktop reconciles the
//! agent's config file after the event, so both surfaces stay one switch.
//!
//! Geometry and hit-testing live here; painting is in
//! `agent_settings_mcp_paint.rs`.

use crate::widgets::agent_settings_metrics::CONTENT_TAIL_PAD;
use crate::widgets::agent_settings_rows::{tab_intro_height, SECTION_GAP, SECTION_HEADER_H};
use crate::{Point2D, Rect};
use de_editor_core::agent_settings::{AgentSettings, McpCli};
use de_editor_core::home_mcp_setup::{home_mcp_manual_snippet, wrap_snippet};
use de_i18n::Locale;

#[path = "agent_settings_mcp_paint.rs"]
mod paint;
pub(super) use paint::paint_mcp_tab;

const INTRO_HAS_DESC: bool = true;
const CARD_H: f32 = 80.0;
const CARD_PAD: f32 = 16.0;
const BTN_W: f32 = 72.0;
const BTN_H: f32 = 30.0;
const PORT_FIELD_W: f32 = 64.0;
const PORT_FIELD_H: f32 = 30.0;
pub(super) const ENDPOINT_FONT: f32 = 11.5;
const ENDPOINT_FIELD_H: f32 = 28.0;
/// Text inset of the endpoint field, and the gap between its text and the
/// Copy chip pinned inside its right end.
const ENDPOINT_FIELD_PAD: f32 = 10.0;
const ENDPOINT_COPY_INSET: f32 = 3.0;
pub(super) const ROW_H: f32 = 46.0;
/// Rows the collapsed list shows before "Show N more".
pub(super) const COLLAPSED_ROWS: usize = 6;
const MORE_ROW_H: f32 = 34.0;
const ACTION_H: f32 = 28.0;
const SET_UP_W: f32 = 80.0;
const REMOVE_W: f32 = 76.0;
pub(super) const SNIPPET_FONT: f32 = 11.0;
pub(super) const SNIPPET_LINE: f32 = 17.6;
const SNIPPET_PAD_LEFT: f32 = 40.0;
const SNIPPET_COPY_H: f32 = 26.0;
/// Copy chip label: size and the lead (pad + icon + gap) / tail before and
/// after it. Paint draws the label at `COPY_CHIP_LEAD`, so the chip's width
/// comes from these and the label, never from a constant (#86: "Kopyala"
/// ran past a fixed 58 px chip).
pub(super) const COPY_CHIP_FONT: f32 = 11.0;
pub(super) const COPY_CHIP_PAD: f32 = 8.0;
pub(super) const COPY_CHIP_ICON: f32 = 12.0;
pub(super) const COPY_CHIP_GAP: f32 = 5.0;
const COPY_CHIP_LEAD: f32 = COPY_CHIP_PAD + COPY_CHIP_ICON + COPY_CHIP_GAP;
/// Headroom over the backend-free width estimate: hit-test has no backend,
/// so the chip is sized from `estimate_width`, and the real glyphs must
/// still fit inside it.
const COPY_LABEL_SLACK: f32 = 1.12;
const CONNECTED_H: f32 = 20.0;
/// Same span as Home's acknowledgement, so the two read alike.
pub(super) const COPY_FEEDBACK_MS: u64 = de_editor_core::HOME_COPIED_MS;

/// Build capability: the agent list writes MCP endpoints into external
/// CLI config files (`~/.claude.json` etc.) via the desktop MCP runtime.
/// The web host has no consumer — hide the list there instead of painting
/// controls that silently do nothing.
const CLI_INTEGRATIONS_PLATFORM: bool = !cfg!(target_arch = "wasm32");

/// Whether the agent list exists on this surface at all (the build
/// capability above).
pub(super) fn cli_integrations_visible() -> bool {
    CLI_INTEGRATIONS_PLATFORM
}

fn body_top(content: Rect) -> f32 {
    content.origin.y + tab_intro_height(INTRO_HAS_DESC)
}

pub(super) fn server_card_rect(content: Rect) -> Rect {
    Rect::xywh(content.origin.x, body_top(content), content.size.x, CARD_H)
}

pub(super) fn server_button_rect(content: Rect) -> Rect {
    let card = server_card_rect(content);
    Rect::xywh(
        card.origin.x + card.size.x - CARD_PAD - BTN_W,
        card.origin.y + (CARD_H - BTN_H) / 2.0,
        BTN_W,
        BTN_H,
    )
}

pub(super) fn port_field_rect(content: Rect) -> Rect {
    let btn = server_button_rect(content);
    Rect::xywh(
        btn.origin.x - 8.0 - PORT_FIELD_W,
        btn.origin.y,
        PORT_FIELD_W,
        PORT_FIELD_H,
    )
}

/// Width of the Copy chip in `locale`: lead + the wider of its two labels
/// ("Copy" / "Copied", so the acknowledgement never overflows either) +
/// the trailing pad.
pub(super) fn copy_chip_width(locale: Locale) -> f32 {
    let label = |key| {
        crate::widgets::home_surface::estimate_width(
            de_i18n::translate(locale, key),
            COPY_CHIP_FONT,
        )
    };
    let text = label("home.mcp.copy").max(label("home.mcp.copied"));
    (COPY_CHIP_LEAD + text * COPY_LABEL_SLACK + COPY_CHIP_PAD).ceil()
}

/// The widest Copy chip across every shipped locale. The snippet box
/// reserves this much so its wrapping does not change with the language.
fn copy_chip_reserve() -> f32 {
    Locale::ALL
        .iter()
        .map(|locale| copy_chip_width(*locale))
        .fold(0.0, f32::max)
}

/// The endpoint field: an inset mono well holding the URL, with the Copy
/// chip pinned inside its right end. Sized from the (monospace, so
/// exactly measurable) endpoint text and the chip.
pub(super) fn endpoint_field_rect(content: Rect, endpoint: &str, locale: Locale) -> Rect {
    let card = server_card_rect(content);
    let text_w = endpoint.chars().count() as f32 * ENDPOINT_FONT * 0.6;
    let w = ENDPOINT_FIELD_PAD
        + text_w
        + ENDPOINT_FIELD_PAD
        + copy_chip_width(locale)
        + ENDPOINT_COPY_INSET;
    Rect::xywh(
        card.origin.x + CARD_PAD,
        card.origin.y + 40.0,
        w,
        ENDPOINT_FIELD_H,
    )
}

/// The endpoint's Copy chip, inside the right end of its field.
pub(super) fn endpoint_copy_rect(content: Rect, endpoint: &str, locale: Locale) -> Rect {
    let field = endpoint_field_rect(content, endpoint, locale);
    let w = copy_chip_width(locale);
    Rect::xywh(
        field.origin.x + field.size.x - ENDPOINT_COPY_INSET - w,
        field.origin.y + ENDPOINT_COPY_INSET,
        w,
        field.size.y - ENDPOINT_COPY_INSET * 2.0,
    )
}

/// Kept under its old name for the hosts' hover plumbing: the endpoint
/// Copy chip is the tab's one copy-config action now.
pub(super) fn client_config_copy_button_rect(
    content: Rect,
    settings: &AgentSettings,
    locale: Locale,
) -> Rect {
    endpoint_copy_rect(content, &settings.mcp_endpoint_url(), locale)
}

/// The Copy chip on an expanded agent's snippet box, top-right.
pub(super) fn snippet_copy_rect(snippet: Rect, locale: Locale) -> Rect {
    let w = copy_chip_width(locale);
    Rect::xywh(
        snippet.origin.x + snippet.size.x - 8.0 - w,
        snippet.origin.y + 5.0,
        w,
        SNIPPET_COPY_H,
    )
}

pub(super) fn agents_header_top(content: Rect) -> f32 {
    server_card_rect(content).origin.y + CARD_H + SECTION_GAP
}

/// Kept under its old name: the layout tests anchor the list to it.
#[cfg(test)]
pub(super) fn integrations_top(content: Rect) -> f32 {
    agents_header_top(content)
}

fn list_top(content: Rect) -> f32 {
    agents_header_top(content) + SECTION_HEADER_H
}

/// The agents the list shows, in Settings row order.
pub(super) fn visible_agents(settings: &AgentSettings) -> &'static [McpCli] {
    if settings.mcp_show_all_agents {
        &McpCli::DISPLAY
    } else {
        &McpCli::DISPLAY[..COLLAPSED_ROWS]
    }
}

/// The snippet column count for the expanded row, from the box width.
fn snippet_columns(content: Rect) -> usize {
    let box_w = content.size.x - SNIPPET_PAD_LEFT - 14.0 - 20.0 - copy_chip_reserve() - 8.0;
    (box_w / (SNIPPET_FONT * 0.6)).floor() as usize
}

pub(super) fn snippet_lines(content: Rect, settings: &AgentSettings, cli: McpCli) -> Vec<String> {
    wrap_snippet(
        &home_mcp_manual_snippet(cli, &settings.mcp_endpoint_url()),
        snippet_columns(content),
    )
}

fn snippet_box_h(lines: usize) -> f32 {
    (lines as f32 * SNIPPET_LINE + 16.0).max(36.0)
}

/// One agent's block: the 46 px row plus, when expanded, its snippet.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct AgentRow {
    pub cli: McpCli,
    pub row: Rect,
    /// The snippet box (zero when collapsed).
    pub snippet: Rect,
    pub action: Rect,
}

pub(super) fn agent_rows(content: Rect, settings: &AgentSettings) -> Vec<AgentRow> {
    let mut y = list_top(content);
    let x = content.origin.x;
    let w = content.size.x;
    visible_agents(settings)
        .iter()
        .map(|cli| {
            let row = Rect::xywh(x, y, w, ROW_H);
            let enabled = settings.mcp_cli_enabled[cli.index()];
            let action_w = if enabled { REMOVE_W } else { SET_UP_W };
            let action = Rect::xywh(
                x + w - 14.0 - action_w,
                y + (ROW_H - ACTION_H) / 2.0,
                action_w,
                ACTION_H,
            );
            y += ROW_H;
            let snippet = if settings.mcp_expanded_agent == Some(*cli) {
                let lines = snippet_lines(content, settings, *cli).len();
                let h = snippet_box_h(lines);
                let rect = Rect::xywh(x + SNIPPET_PAD_LEFT, y, w - SNIPPET_PAD_LEFT - 14.0, h);
                y += h + 14.0;
                rect
            } else {
                Rect::ZERO
            };
            AgentRow {
                cli: *cli,
                row,
                snippet,
                action,
            }
        })
        .collect()
}

/// The list's outer frame, including the "Show N more" row.
pub(super) fn list_frame(content: Rect, settings: &AgentSettings) -> Rect {
    let top = list_top(content);
    Rect::xywh(
        content.origin.x,
        top,
        content.size.x,
        list_bottom(content, settings) - top,
    )
}

fn rows_bottom(content: Rect, settings: &AgentSettings) -> f32 {
    agent_rows(content, settings)
        .last()
        .map(|last| {
            if last.snippet.size.y > 0.0 {
                last.snippet.origin.y + last.snippet.size.y + 14.0
            } else {
                last.row.origin.y + ROW_H
            }
        })
        .unwrap_or_else(|| list_top(content))
}

/// The "Show N more agents" / "Show fewer" toggle row.
pub(super) fn show_more_rect(content: Rect, settings: &AgentSettings) -> Rect {
    Rect::xywh(
        content.origin.x,
        rows_bottom(content, settings),
        content.size.x,
        MORE_ROW_H,
    )
}

fn list_bottom(content: Rect, settings: &AgentSettings) -> f32 {
    let more = show_more_rect(content, settings);
    more.origin.y + more.size.y
}

/// The CONNECTED line under the list (or under the card on hosts without
/// the list).
pub(super) fn connected_rect(content: Rect, settings: &AgentSettings) -> Rect {
    let y = if cli_integrations_visible() {
        list_bottom(content, settings) + 16.0
    } else {
        server_card_rect(content).origin.y + CARD_H + 16.0
    };
    Rect::xywh(content.origin.x, y, content.size.x, CONNECTED_H)
}

/// Row 0's box, for the layout tests that walk rows.
#[cfg(test)]
pub(super) fn cli_row_rect(content: Rect, index: usize) -> Rect {
    Rect::xywh(
        content.origin.x,
        list_top(content) + index as f32 * ROW_H,
        content.size.x,
        ROW_H,
    )
}

/// Row boxes for this tab, paired with their line count — walked by the
/// traversal layout test.
#[cfg(test)]
pub(super) fn row_boxes(
    content: Rect,
) -> Vec<(Rect, crate::widgets::agent_settings_rows::RowLines)> {
    use crate::widgets::agent_settings_rows::RowLines;
    let settings = AgentSettings::default();
    let mut boxes = vec![(server_card_rect(content), RowLines::Two)];
    if cli_integrations_visible() {
        boxes.extend(
            agent_rows(content, &settings)
                .into_iter()
                .map(|row| (row.row, RowLines::Two)),
        );
    }
    boxes
}

pub(super) fn content_height(settings: &AgentSettings) -> f32 {
    content_height_at(settings, 600.0)
}

/// The tab's height when laid out `width` wide: an expanded snippet wraps
/// onto more lines in a narrow window.
pub(super) fn content_height_at(settings: &AgentSettings, width: f32) -> f32 {
    let origin = Rect::xywh(0.0, 0.0, width, 0.0);
    connected_rect(origin, settings).origin.y + CONNECTED_H - origin.origin.y + CONTENT_TAIL_PAD
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpHit {
    ToggleServer,
    /// Set up / Remove on an agent row (flips its config slot).
    ToggleCli(McpCli),
    /// The endpoint's Copy chip.
    CopyClientConfig,
    FocusPort,
    /// A press on an agent row outside its action: expand / collapse.
    ExpandCli(McpCli),
    CopySnippet(McpCli),
    ToggleShowAll,
    None,
}

pub fn hit_test(
    content: Rect,
    settings: &AgentSettings,
    locale: Locale,
    scrolled: Point2D,
) -> McpHit {
    // Host-managed (embed): the extension owns the lifecycle — no
    // start/stop, no port editing; the endpoint still copies.
    let host_managed = settings.mcp_host_managed();
    if !host_managed && server_button_rect(content).contains(scrolled) {
        return McpHit::ToggleServer;
    }
    if client_config_copy_button_rect(content, settings, locale).contains(scrolled) {
        return McpHit::CopyClientConfig;
    }
    if !host_managed && !settings.mcp_server.running && port_field_rect(content).contains(scrolled)
    {
        return McpHit::FocusPort;
    }
    if !cli_integrations_visible() {
        return McpHit::None;
    }
    for row in agent_rows(content, settings) {
        if row.action.contains(scrolled) {
            return McpHit::ToggleCli(row.cli);
        }
        if row.snippet.size.y > 0.0 && snippet_copy_rect(row.snippet, locale).contains(scrolled) {
            return McpHit::CopySnippet(row.cli);
        }
        if row.row.contains(scrolled) {
            return McpHit::ExpandCli(row.cli);
        }
    }
    if show_more_rect(content, settings).contains(scrolled) {
        return McpHit::ToggleShowAll;
    }
    McpHit::None
}

/// Whether a copy acknowledgement started at `copied_at` still shows.
pub(super) fn copy_feedback_active(copied_at: Option<u64>, now_ms: u64) -> bool {
    copied_at.is_some_and(|at| now_ms.saturating_sub(at) < COPY_FEEDBACK_MS)
}

/// When the Settings ▸ MCP "Copied" chips revert, so the frame loop wakes
/// for it instead of waiting on the next input event.
pub fn copy_feedback_deadline_ms(settings: &AgentSettings, now_ms: u64) -> Option<u64> {
    [
        settings.mcp_client_config_copied_at_ms,
        settings.mcp_snippet_copied_at_ms,
    ]
    .into_iter()
    .flatten()
    .map(|at| at.saturating_add(COPY_FEEDBACK_MS))
    .filter(|deadline| *deadline > now_ms)
    .min()
}

#[cfg(test)]
#[path = "agent_settings_mcp_tests.rs"]
mod tests;
