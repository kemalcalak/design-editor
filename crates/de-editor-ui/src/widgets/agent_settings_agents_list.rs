//! The Agents tab's left column (#26): API providers (saved ones and the
//! featured presets not set up yet) closed by "Add custom endpoint", then
//! the agents on this machine (CLI agents and ACP agents) closed by "Add
//! ACP agent". Each add button ends its own section so neither falls
//! below the fold. Rects here are the single source paint, hit-test and
//! hover read.

use crate::theme::Theme;
use crate::widgets::agent_settings_detail_chrome::{paint_button, text, ButtonKind, BUTTON_H};
use crate::widgets::agent_settings_i18n::t as t_settings;
use crate::widgets::agent_settings_rows::{fit_text, SETTINGS_FONT_FAMILY};
use crate::widgets::icons::Icon;
use crate::widgets::PaintCx;
use crate::{Color, Point2D, Rect};
use de_editor_core::agent_settings::{AgentProvider, AgentSettings, AgentsEntry};
use de_editor_core::agent_settings_connection::ProviderConnectPhase;
use de_editor_core::editor_ui_state::EditorUiState;
use de_editor_core::{builtin_agent_preset, BuiltinAgentPresetKey};

/// Left column width; the detail pane takes the rest.
pub(crate) const LIST_W: f32 = 220.0;
/// Space between the list's divider and the detail pane on each side.
pub(crate) const COLUMN_GAP: f32 = 24.0;
const SECTION_H: f32 = 32.0;
const ROW_H: f32 = 44.0;
const SECTION_GAP: f32 = 12.0;
const ADD_GAP: f32 = 8.0;
const NAME_FONT: f32 = 13.5;
const SUB_FONT: f32 = 12.0;
const DOT: f32 = 7.0;

/// Presets offered on the list before they are set up; the rest are
/// reached through "Add custom endpoint"'s provider select.
pub(crate) const FEATURED_PRESETS: [BuiltinAgentPresetKey; 5] = [
    BuiltinAgentPresetKey::Anthropic,
    BuiltinAgentPresetKey::OpenAi,
    BuiltinAgentPresetKey::OpenRouter,
    BuiltinAgentPresetKey::DeepSeek,
    BuiltinAgentPresetKey::Gemini,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ListSection {
    Api,
    Machine,
}

/// One painted line of the list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ListItem {
    Section(ListSection),
    Builtin(usize),
    Preset(BuiltinAgentPresetKey),
    Cli(AgentProvider),
    Acp(usize),
    AddCustom,
    AddAcp,
}

/// The two columns inside the tab body.
pub(crate) fn columns(body: Rect) -> (Rect, Rect) {
    let list = Rect::xywh(body.origin.x, body.origin.y, LIST_W, body.size.y);
    let detail_x = body.origin.x + LIST_W + COLUMN_GAP * 2.0;
    let detail = Rect::xywh(
        detail_x,
        body.origin.y,
        (body.origin.x + body.size.x - detail_x).max(0.0),
        body.size.y,
    );
    (list, detail)
}

/// The x of the divider between the columns.
pub(crate) fn divider_x(body: Rect) -> f32 {
    body.origin.x + LIST_W + COLUMN_GAP
}

fn items(settings: &AgentSettings, external: bool) -> Vec<ListItem> {
    let mut out = vec![ListItem::Section(ListSection::Api)];
    out.extend((0..settings.builtin_agents.len()).map(ListItem::Builtin));
    out.extend(
        FEATURED_PRESETS
            .iter()
            .filter(|key| {
                !settings
                    .builtin_agents
                    .iter()
                    .any(|agent| agent.preset == **key)
            })
            .map(|key| ListItem::Preset(*key)),
    );
    out.push(ListItem::AddCustom);
    if external {
        out.push(ListItem::Section(ListSection::Machine));
        out.extend(AgentProvider::DISPLAY.iter().map(|p| ListItem::Cli(*p)));
        out.extend((0..settings.acp_agents.len()).map(ListItem::Acp));
        out.push(ListItem::AddAcp);
    }
    out
}

/// Every list line with its rect, top to bottom from `list`'s top.
pub(crate) fn list_rows(
    list: Rect,
    settings: &AgentSettings,
    external: bool,
) -> Vec<(ListItem, Rect)> {
    let mut y = list.origin.y;
    let mut out = Vec::new();
    let mut previous: Option<ListItem> = None;
    for item in items(settings, external) {
        let h = match item {
            ListItem::Section(_) => {
                if previous.is_some() {
                    y += SECTION_GAP;
                }
                SECTION_H
            }
            ListItem::AddCustom | ListItem::AddAcp => {
                y += ADD_GAP;
                BUTTON_H
            }
            _ => ROW_H,
        };
        out.push((item, Rect::xywh(list.origin.x, y, list.size.x, h)));
        y += h;
        previous = Some(item);
    }
    out
}

/// Height of the whole list.
pub(crate) fn list_height(settings: &AgentSettings, external: bool) -> f32 {
    let list = Rect::xywh(0.0, 0.0, LIST_W, 0.0);
    list_rows(list, settings, external)
        .last()
        .map(|(_, rect)| rect.origin.y + rect.size.y)
        .unwrap_or(0.0)
}

/// The pressable line under `point`, and its index for hover.
pub(crate) fn row_at(
    list: Rect,
    settings: &AgentSettings,
    external: bool,
    point: Point2D,
) -> Option<(usize, ListItem)> {
    list_rows(list, settings, external)
        .into_iter()
        .enumerate()
        .find(|(_, (item, rect))| !matches!(item, ListItem::Section(_)) && rect.contains(point))
        .map(|(index, (item, _))| (index, item))
}

/// Whether `item` is the row the detail pane shows.
pub(crate) fn is_selected(
    item: ListItem,
    selected: Option<&AgentsEntry>,
    settings: &AgentSettings,
) -> bool {
    let draft_preset = settings
        .builtin_agent_draft
        .as_ref()
        .map(|draft| draft.preset);
    match (item, selected) {
        (ListItem::Builtin(index), Some(AgentsEntry::Builtin(id))) => settings
            .builtin_agents
            .get(index)
            .is_some_and(|agent| agent.id == *id),
        (ListItem::Preset(key), Some(AgentsEntry::BuiltinDraft)) => draft_preset == Some(key),
        (ListItem::AddCustom, Some(AgentsEntry::BuiltinDraft)) => {
            draft_preset.is_some_and(|key| !FEATURED_PRESETS.contains(&key))
        }
        (ListItem::Cli(a), Some(AgentsEntry::Cli(b))) => a == *b,
        (ListItem::Acp(index), Some(AgentsEntry::Acp(id))) => settings
            .acp_agents
            .get(index)
            .is_some_and(|agent| agent.id == *id),
        (ListItem::AddAcp, Some(AgentsEntry::AcpDraft)) => true,
        _ => false,
    }
}

/// Status dot colour and the second line of a row.
fn row_status(
    theme: &Theme,
    ui: &EditorUiState,
    settings: &AgentSettings,
    item: ListItem,
) -> (Color, String) {
    let off = theme.muted_subtle;
    let ok = theme.status_success;
    match item {
        ListItem::Builtin(index) => {
            let Some(agent) = settings.builtin_agents.get(index) else {
                return (off, String::new());
            };
            let kind = if agent.preset == BuiltinAgentPresetKey::Custom {
                t_settings(ui, "builtin.custom").to_string()
            } else {
                builtin_agent_preset(agent.preset).display_name.to_string()
            };
            let count = t_settings(ui, "settings.agents.modelCount")
                .replace("{{count}}", &agent.models.len().to_string());
            (
                if agent.ready() { ok } else { off },
                format!("{kind} · {count}"),
            )
        }
        ListItem::Preset(_) => (off, t_settings(ui, "settings.agents.notSet").to_string()),
        ListItem::Cli(provider) => {
            let index = AgentSettings::provider_index(provider);
            let connection = &settings.provider_connection[index];
            let (color, key) = if settings.provider_verified_connected_at(index) {
                (ok, "settings.agents.statusConnected")
            } else if connection.phase == ProviderConnectPhase::Probing {
                (theme.status_warning, "settings.agents.statusChecking")
            } else if connection.not_installed {
                (off, "settings.agents.notInstalled")
            } else {
                (off, "settings.agents.statusNotConnected")
            };
            (color, format!("CLI · {}", t_settings(ui, key)))
        }
        ListItem::Acp(index) => {
            let connected = settings.acp_agent_verified_connected_at(index);
            let key = if connected {
                "settings.agents.statusConnected"
            } else {
                "settings.agents.statusNotConnected"
            };
            (
                if connected { ok } else { off },
                format!("ACP · {}", t_settings(ui, key)),
            )
        }
        _ => (off, String::new()),
    }
}

fn row_name(settings: &AgentSettings, item: ListItem) -> String {
    match item {
        ListItem::Builtin(index) => settings
            .builtin_agents
            .get(index)
            .map(|agent| agent.display_name.clone())
            .unwrap_or_default(),
        ListItem::Preset(key) => builtin_agent_preset(key).display_name.to_string(),
        ListItem::Cli(provider) => provider.name().to_string(),
        ListItem::Acp(index) => settings
            .acp_agents
            .get(index)
            .map(|agent| agent.display_name.clone())
            .unwrap_or_default(),
        _ => String::new(),
    }
}

/// Paint the column. `hover` is the hovered line's index.
pub(crate) fn paint_list(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    ui: &EditorUiState,
    settings: &AgentSettings,
    list: Rect,
    external: bool,
    hover: Option<usize>,
) {
    let selected = settings.resolved_agents_entry(external);
    for (index, (item, rect)) in list_rows(list, settings, external).into_iter().enumerate() {
        let active = is_selected(item, selected.as_ref(), settings);
        match item {
            ListItem::Section(section) => {
                let key = match section {
                    ListSection::Api => "settings.agents.apiProviders",
                    ListSection::Machine => "settings.agents.onThisMachine",
                };
                let title = t_settings(ui, key).to_uppercase();
                let title = fit_text(cx, &title, rect.size.x, 11.0);
                text(
                    cx,
                    &title,
                    11.0,
                    theme.muted_foreground,
                    600,
                    SETTINGS_FONT_FAMILY,
                    rect.origin.x + 4.0,
                    rect.origin.y + 20.0,
                );
            }
            ListItem::AddCustom | ListItem::AddAcp => {
                let key = if item == ListItem::AddCustom {
                    "settings.agents.addCustomEndpoint"
                } else {
                    "settings.agents.addAcpAgent"
                };
                if active || hover == Some(index) {
                    cx.backend.fill_round_rect(rect, 8.0, theme.muted);
                }
                paint_button(
                    cx,
                    theme,
                    rect,
                    t_settings(ui, key),
                    Some(Icon::Plus),
                    ButtonKind::Outline,
                );
                if active || hover == Some(index) {
                    cx.backend.stroke_round_rect(rect, 8.0, theme.primary, 1.0);
                }
            }
            _ => {
                if active {
                    cx.backend.fill_round_rect(rect, 8.0, theme.muted);
                } else if hover == Some(index) {
                    cx.backend
                        .fill_round_rect(rect, 8.0, theme.muted.with_alpha(0.55));
                }
                let (dot, status) = row_status(theme, ui, settings, item);
                cx.backend.fill_round_rect(
                    Rect::xywh(rect.origin.x + 12.0, rect.origin.y + 13.0, DOT, DOT),
                    DOT / 2.0,
                    dot,
                );
                let x = rect.origin.x + 28.0;
                let budget = (rect.origin.x + rect.size.x - 8.0 - x).max(0.0);
                let name = fit_text(cx, &row_name(settings, item), budget, NAME_FONT);
                text(
                    cx,
                    &name,
                    NAME_FONT,
                    if active {
                        theme.foreground
                    } else {
                        theme.card_foreground
                    },
                    if active { 600 } else { 500 },
                    SETTINGS_FONT_FAMILY,
                    x,
                    rect.origin.y + 18.0,
                );
                let status = fit_text(cx, &status, budget, SUB_FONT);
                text(
                    cx,
                    &status,
                    SUB_FONT,
                    theme.muted_foreground,
                    400,
                    SETTINGS_FONT_FAMILY,
                    x,
                    rect.origin.y + 34.0,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list() -> Rect {
        Rect::xywh(0.0, 100.0, LIST_W, 600.0)
    }

    #[test]
    fn featured_presets_hide_once_set_up() {
        let mut settings = AgentSettings::default();
        let rows = list_rows(list(), &settings, false);
        assert!(rows
            .iter()
            .any(|(item, _)| *item == ListItem::Preset(BuiltinAgentPresetKey::Anthropic)));
        settings.add_builtin_agent_with_defaults("Mine", "sk", "m");
        let rows = list_rows(list(), &settings, false);
        assert!(rows.iter().any(|(item, _)| *item == ListItem::Builtin(0)));
        assert!(!rows
            .iter()
            .any(|(item, _)| *item == ListItem::Preset(BuiltinAgentPresetKey::Anthropic)));
    }

    #[test]
    fn the_web_list_has_no_machine_section() {
        let settings = AgentSettings::default();
        let web = list_rows(list(), &settings, false);
        assert!(!web
            .iter()
            .any(|(item, _)| matches!(item, ListItem::Cli(_) | ListItem::AddAcp)));
        let desktop = list_rows(list(), &settings, true);
        assert_eq!(
            desktop
                .iter()
                .filter(|(item, _)| matches!(item, ListItem::Cli(_)))
                .count(),
            AgentProvider::DISPLAY.len()
        );
        assert_eq!(desktop.last().unwrap().0, ListItem::AddAcp);
    }

    #[test]
    fn rows_stack_without_overlap_and_sections_do_not_hit() {
        let settings = AgentSettings::default();
        let rows = list_rows(list(), &settings, true);
        for pair in rows.windows(2) {
            assert!(pair[0].1.origin.y + pair[0].1.size.y <= pair[1].1.origin.y + 0.01);
        }
        let header = rows[0].1;
        let on_header = Point2D::new(header.origin.x + 10.0, header.origin.y + 10.0);
        assert_eq!(row_at(list(), &settings, true, on_header), None);
        let first = rows[1].1;
        let on_first = Point2D::new(first.origin.x + 10.0, first.origin.y + 10.0);
        assert_eq!(
            row_at(list(), &settings, true, on_first),
            Some((1, rows[1].0))
        );
        assert_eq!(
            list_height(&settings, true),
            rows.last().unwrap().1.origin.y + BUTTON_H - list().origin.y
        );
    }

    #[test]
    fn a_draft_highlights_its_preset_or_the_custom_button() {
        let mut settings = AgentSettings::default();
        settings.begin_builtin_agent_draft();
        settings.set_builtin_agent_draft_preset(BuiltinAgentPresetKey::OpenRouter);
        let selected = settings.resolved_agents_entry(true);
        assert!(is_selected(
            ListItem::Preset(BuiltinAgentPresetKey::OpenRouter),
            selected.as_ref(),
            &settings
        ));
        assert!(!is_selected(
            ListItem::AddCustom,
            selected.as_ref(),
            &settings
        ));
        settings.set_builtin_agent_draft_preset(BuiltinAgentPresetKey::Doubao);
        assert!(is_selected(
            ListItem::AddCustom,
            selected.as_ref(),
            &settings
        ));
    }
}
