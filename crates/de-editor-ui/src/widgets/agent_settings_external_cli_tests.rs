//! Settings-modal coverage for the external-CLI surfaces: the Agents
//! tab's list (CLI agents and the ACP add button), and the MCP tab's
//! terminal-integration list. Every host that paints the modal can run
//! agent CLIs, so each surface must stay reachable — every provider keeps
//! a list row, a hover target, and a painted name, and every CLI keeps
//! its MCP toggle.

use crate::widgets::agent_settings_panel::{AgentSettingsHit, AgentSettingsPanel};
use crate::widgets::test_capture_backend::CaptureBackend;
use crate::widgets::{PaintCx, Widget};
use crate::{Point2D, Rect};
use de_editor_core::agent_settings::{AgentSettingsTab, McpCli};
use de_editor_core::{AgentProvider, EditorState};

const VIEWPORT_W: f32 = 1200.0;
const VIEWPORT_H: f32 = 800.0;
/// Probe grid over the scrollable body. Every control on these tabs is at
/// least 24 pt tall and 64 pt wide, so an 8 pt lattice cannot step over
/// one.
const PROBE_STEP: f32 = 8.0;
/// Scroll stride for the sweep. Smaller than the shortest row so no band
/// of the document can hide between two sampled offsets.
const SCROLL_STEP: f32 = 24.0;

fn state_for(tab: AgentSettingsTab) -> EditorState {
    let mut state = EditorState::default();
    state.editor_ui.locale = de_i18n::Locale::EnUs;
    state.editor_ui.agent_settings.tab = tab;
    state
}

fn panel_rect(state: &EditorState) -> Rect {
    AgentSettingsPanel::for_editor(state).rect(VIEWPORT_W, VIEWPORT_H)
}

/// Every distinct control hit reachable anywhere in the scrollable body,
/// across the whole scroll range. This is the "no live hit rect" oracle:
/// a control that is not in this set cannot be pressed by any gesture the
/// modal accepts.
fn reachable_hits(state: &mut EditorState) -> Vec<AgentSettingsHit> {
    let rect = panel_rect(state);
    let max_scroll = AgentSettingsPanel::for_editor(state).max_scroll(rect);
    let restore = state.editor_ui.agent_settings.scroll_y.offset;
    let mut hits: Vec<AgentSettingsHit> = Vec::new();
    let mut scroll = 0.0_f32;
    loop {
        state.editor_ui.agent_settings.scroll_y.offset = scroll;
        {
            let panel = AgentSettingsPanel::for_editor(state);
            let content = panel.resolved_content_viewport(rect);
            let mut y = content.origin.y;
            while y <= content.origin.y + content.size.y {
                let mut x = content.origin.x;
                while x <= content.origin.x + content.size.x {
                    let hit = panel.hit_test(rect, Point2D::new(x, y));
                    if !matches!(hit, AgentSettingsHit::Inside | AgentSettingsHit::Outside)
                        && !hits.contains(&hit)
                    {
                        hits.push(hit);
                    }
                    x += PROBE_STEP;
                }
                y += PROBE_STEP;
            }
        }
        if scroll >= max_scroll {
            break;
        }
        scroll = (scroll + SCROLL_STEP).min(max_scroll);
    }
    state.editor_ui.agent_settings.scroll_y.offset = restore;
    hits
}

/// Every Agents list line the modal offers a hover target for, across
/// the scroll range — the hover ladder's own view of the list.
fn hovered_row_indices(state: &mut EditorState) -> Vec<usize> {
    let rect = panel_rect(state);
    let max_scroll = AgentSettingsPanel::for_editor(state).max_scroll(rect);
    let restore = state.editor_ui.agent_settings.scroll_y.offset;
    let mut cards: Vec<usize> = Vec::new();
    let mut scroll = 0.0_f32;
    loop {
        state.editor_ui.agent_settings.scroll_y.offset = scroll;
        {
            let panel = AgentSettingsPanel::for_editor(state);
            let content = panel.resolved_content_viewport(rect);
            let mut y = content.origin.y;
            while y <= content.origin.y + content.size.y {
                let mut x = content.origin.x;
                while x <= content.origin.x + content.size.x {
                    if let Some(index) = panel.agents_row_at(rect, Point2D::new(x, y)) {
                        if !cards.contains(&index) {
                            cards.push(index);
                        }
                    }
                    x += PROBE_STEP;
                }
                y += PROBE_STEP;
            }
        }
        if scroll >= max_scroll {
            break;
        }
        scroll = (scroll + SCROLL_STEP).min(max_scroll);
    }
    state.editor_ui.agent_settings.scroll_y.offset = restore;
    cards
}

/// Every string the modal paints for `state`, across the scroll range.
fn painted_texts(state: &mut EditorState) -> Vec<String> {
    let rect = panel_rect(state);
    let max_scroll = AgentSettingsPanel::for_editor(state).max_scroll(rect);
    let restore = state.editor_ui.agent_settings.scroll_y.offset;
    let mut texts: Vec<String> = Vec::new();
    let mut scroll = 0.0_f32;
    loop {
        state.editor_ui.agent_settings.scroll_y.offset = scroll;
        {
            let panel = AgentSettingsPanel::for_editor(state);
            let mut backend = CaptureBackend::default();
            let mut cx = PaintCx {
                backend: &mut backend,
            };
            panel.paint(&mut cx, rect);
            texts.extend(backend.texts.into_iter().map(|(text, _)| text));
        }
        if scroll >= max_scroll {
            break;
        }
        scroll = (scroll + SCROLL_STEP).min(max_scroll);
    }
    state.editor_ui.agent_settings.scroll_y.offset = restore;
    texts
}

fn cli_row_hits(hits: &[AgentSettingsHit]) -> Vec<AgentProvider> {
    hits.iter()
        .filter_map(|hit| match hit {
            AgentSettingsHit::SelectCliEntry(provider) => Some(*provider),
            _ => None,
        })
        .collect()
}

fn cli_toggle_hits(hits: &[AgentSettingsHit]) -> Vec<McpCli> {
    hits.iter()
        .filter_map(|hit| match hit {
            AgentSettingsHit::ToggleMcpCli(cli) => Some(*cli),
            _ => None,
        })
        .collect()
}

// ---------------------------------------------------------------------
// Agents tab
// ---------------------------------------------------------------------

#[test]
fn agents_tab_offers_every_external_provider_row() {
    let mut state = state_for(AgentSettingsTab::Agents);
    let hits = reachable_hits(&mut state);

    let mut rows = cli_row_hits(&hits);
    rows.sort_by_key(|provider| provider.index());
    let mut expected = AgentProvider::ALL.to_vec();
    expected.sort_by_key(|provider| provider.index());
    assert_eq!(rows, expected, "every provider keeps a list row");
    assert!(
        hits.iter()
            .any(|hit| matches!(hit, AgentSettingsHit::Connect(_))),
        "the selected agent's detail offers Connect"
    );
    assert!(
        hits.contains(&AgentSettingsHit::AddAcpAgent),
        "the ACP section's add action belongs to the external-CLI half"
    );
    assert!(
        hits.contains(&AgentSettingsHit::AddProvider),
        "the built-in API-key section's add action must be reachable"
    );
}

#[test]
fn every_list_row_has_a_hover_target() {
    let mut state = state_for(AgentSettingsTab::Agents);
    let settings = state.editor_ui.agent_settings.clone();
    let pressable = crate::widgets::agent_settings_agents_list::list_rows(
        Rect::xywh(0.0, 0.0, 220.0, 0.0),
        &settings,
        true,
    )
    .iter()
    .filter(|(item, _)| {
        !matches!(
            item,
            crate::widgets::agent_settings_agents_list::ListItem::Section(_)
        )
    })
    .count();
    assert_eq!(
        hovered_row_indices(&mut state).len(),
        pressable,
        "hover covers every list row"
    );
}

#[test]
fn agents_tab_paints_every_provider_name() {
    let mut state = state_for(AgentSettingsTab::Agents);
    let texts = painted_texts(&mut state);
    for provider in AgentProvider::ALL {
        assert!(
            texts.iter().any(|text| text.contains(provider.name())),
            "the Agents tab must paint {:?}",
            provider.name()
        );
    }
}

// ---------------------------------------------------------------------
// MCP tab
// ---------------------------------------------------------------------

#[test]
fn mcp_tab_offers_every_cli_toggle() {
    let mut state = state_for(AgentSettingsTab::Mcp);
    state.editor_ui.agent_settings.mcp_server.running = true;
    // The collapsed list shows six agents; "Show more" reveals the rest.
    state.editor_ui.agent_settings.mcp_show_all_agents = true;
    let hits = reachable_hits(&mut state);

    let mut toggles = cli_toggle_hits(&hits);
    toggles.sort_by_key(|cli| cli.index());
    let mut expected = McpCli::ALL.to_vec();
    expected.sort_by_key(|cli| cli.index());
    assert_eq!(
        toggles, expected,
        "every agent stays reachable from the MCP tab"
    );
}
