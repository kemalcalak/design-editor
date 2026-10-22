//! The Agents tab's CLI agent detail (#26): the probe status with Check
//! again, the agent's discovered models as a checklist (unticked ones
//! leave the chat picker, D6), and Connect / Disconnect.

use crate::theme::Theme;
use crate::widgets::agent_settings_detail_chrome::{
    self as chrome, button_width_estimate, checklist_height, checklist_row_at, paint_button,
    paint_checklist, paint_header, paint_label, paint_status, text, ButtonKind, CheckRow, BUTTON_H,
    FIELD_GAP, HEADER_H, LABEL_FONT, LABEL_H, VALUE_FONT,
};
use crate::widgets::agent_settings_i18n::t as t_settings;
use crate::widgets::agent_settings_rows::{fit_text, SETTINGS_FONT_FAMILY};
use crate::widgets::home_surface::MONO;
use crate::widgets::icons::{draw_icon, Icon};
use crate::widgets::PaintCx;
use crate::{Point2D, Rect};
use de_editor_core::agent_settings::{AgentProvider, AgentSettings};
use de_editor_core::agent_settings_connection::ProviderConnectPhase;
use de_editor_core::editor_ui_state::EditorUiState;
use de_editor_core::ModelEntry;

const STATUS_H: f32 = 64.0;
const EMPTY_H: f32 = 52.0;
const FOOTER_GAP: f32 = 28.0;

/// The agent's discovered model ids, in discovery order.
pub(crate) fn cli_model_rows(discovered: &[ModelEntry], provider: AgentProvider) -> Vec<&str> {
    discovered
        .iter()
        .filter(|entry| entry.provider == provider && entry.builtin_provider_id.is_none())
        .map(|entry| entry.value.as_str())
        .collect()
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct CliLayout {
    pub(crate) status: Rect,
    pub(crate) recheck: Rect,
    pub(crate) models: Rect,
    pub(crate) model_rows: usize,
    pub(crate) footer: Rect,
    pub(crate) connect: Rect,
    pub(crate) height: f32,
}

fn connect_label(settings: &AgentSettings, provider: AgentProvider) -> &'static str {
    if settings.provider_verified_connected(provider) {
        "common.disconnect"
    } else {
        "common.connect"
    }
}

pub(crate) fn cli_layout(
    detail: Rect,
    settings: &AgentSettings,
    ui: &EditorUiState,
    discovered: &[ModelEntry],
    provider: AgentProvider,
) -> CliLayout {
    let x = detail.origin.x;
    let w = detail.size.x;
    let mut y = detail.origin.y + HEADER_H + 8.0 + LABEL_H;
    let status = Rect::xywh(x, y, w, STATUS_H);
    let recheck_w = button_width_estimate(t_settings(ui, "settings.agents.checkAgain"), true);
    let recheck = Rect::xywh(
        x + w - 14.0 - recheck_w,
        status.origin.y + (STATUS_H - BUTTON_H) / 2.0,
        recheck_w,
        BUTTON_H,
    );
    y += STATUS_H + FIELD_GAP + LABEL_H;
    let model_rows = cli_model_rows(discovered, provider).len();
    let models_h = if model_rows == 0 {
        EMPTY_H
    } else {
        checklist_height(model_rows)
    };
    let models = Rect::xywh(x, y, w, models_h);
    y += models_h + FOOTER_GAP;
    let footer = Rect::xywh(x, y, w, BUTTON_H);
    let label = t_settings(ui, connect_label(settings, provider));
    let connect_w = button_width_estimate(label, false);
    let connect = Rect::xywh(x + w - connect_w, y, connect_w, BUTTON_H);
    CliLayout {
        status,
        recheck,
        models,
        model_rows,
        footer,
        connect,
        height: y + BUTTON_H - detail.origin.y,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CliHit {
    Recheck,
    ToggleModel(usize),
    Connect,
    None,
}

pub(crate) fn cli_hit(settings: &AgentSettings, layout: &CliLayout, point: Point2D) -> CliHit {
    if layout.recheck.contains(point) {
        return CliHit::Recheck;
    }
    if layout.model_rows > 0 {
        let scroll = settings.builtin_model_menu_scroll.offset;
        if let Some(row) = checklist_row_at(layout.models, layout.model_rows, scroll, point) {
            return CliHit::ToggleModel(row);
        }
    }
    if layout.connect.contains(point) {
        return CliHit::Connect;
    }
    CliHit::None
}

/// Icon, colour, headline and detail line for the probe state.
fn status_lines(
    theme: &Theme,
    ui: &EditorUiState,
    settings: &AgentSettings,
    provider: AgentProvider,
) -> (Icon, crate::Color, String, String) {
    let index = AgentSettings::provider_index(provider);
    let connection = &settings.provider_connection[index];
    let version = connection
        .version
        .as_deref()
        .map(|version| format!(" · {version}"))
        .unwrap_or_default();
    if settings.provider_verified_connected_at(index) {
        return (
            Icon::CheckCircle,
            theme.status_success,
            format!(
                "{}{version}",
                t_settings(ui, "settings.agents.statusConnected")
            ),
            connection
                .warning
                .clone()
                .or_else(|| connection.info.clone())
                .unwrap_or_else(|| t_settings(ui, "settings.agents.cliIdleNote").to_string()),
        );
    }
    match connection.phase {
        ProviderConnectPhase::Probing => (
            Icon::RefreshCw,
            theme.status_warning,
            t_settings(ui, "settings.agents.statusChecking").to_string(),
            String::new(),
        ),
        _ if connection.not_installed => (
            Icon::Terminal,
            theme.muted_foreground,
            t_settings(ui, "settings.agents.notInstalled").to_string(),
            connection.install_command.clone().unwrap_or_default(),
        ),
        ProviderConnectPhase::Error => (
            Icon::AlertTriangle,
            theme.destructive,
            t_settings(ui, "settings.agents.statusNotConnected").to_string(),
            connection.error.clone().unwrap_or_default(),
        ),
        _ => (
            Icon::Info,
            theme.muted_foreground,
            t_settings(ui, "settings.agents.statusNotConnected").to_string(),
            t_settings(ui, "settings.agents.cliConnectHint").to_string(),
        ),
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn paint_cli_detail(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    settings: &AgentSettings,
    ui: &EditorUiState,
    detail: Rect,
    discovered: &[ModelEntry],
    provider: AgentProvider,
) {
    let layout = cli_layout(detail, settings, ui, discovered, provider);
    paint_header(
        cx,
        theme,
        detail,
        provider.name(),
        t_settings(ui, "settings.agents.cliSubtitle"),
        None,
    );
    paint_label(
        cx,
        theme,
        t_settings(ui, "settings.agents.status"),
        layout.status,
    );
    cx.backend
        .fill_round_rect(layout.status, 8.0, theme.background);
    cx.backend
        .stroke_round_rect(layout.status, 8.0, theme.border, 1.0);
    let (icon, color, headline, detail_line) = status_lines(theme, ui, settings, provider);
    let glyph = 18.0;
    let x = layout.status.origin.x + 16.0;
    draw_icon(
        cx.backend,
        icon,
        Point2D::new(x, layout.status.origin.y + (STATUS_H - glyph) / 2.0),
        glyph,
        color,
        1.7,
    );
    let text_x = x + glyph + 14.0;
    let budget = (layout.recheck.origin.x - 12.0 - text_x).max(0.0);
    let headline = fit_text(cx, &headline, budget, VALUE_FONT + 0.5);
    text(
        cx,
        &headline,
        VALUE_FONT + 0.5,
        theme.foreground,
        600,
        SETTINGS_FONT_FAMILY,
        text_x,
        layout.status.origin.y + 27.0,
    );
    if !detail_line.is_empty() {
        let line = fit_text(cx, &detail_line, budget, LABEL_FONT);
        let mono = !settings.provider_verified_connected(provider)
            && settings.provider_connection[AgentSettings::provider_index(provider)].not_installed;
        text(
            cx,
            &line,
            LABEL_FONT,
            theme.muted_foreground,
            400,
            if mono { MONO } else { SETTINGS_FONT_FAMILY },
            text_x,
            layout.status.origin.y + 46.0,
        );
    }
    paint_button(
        cx,
        theme,
        layout.recheck,
        t_settings(ui, "settings.agents.checkAgain"),
        Some(Icon::RefreshCw),
        ButtonKind::Outline,
    );
    paint_label(
        cx,
        theme,
        t_settings(ui, "settings.agents.models"),
        layout.models,
    );
    let ids = cli_model_rows(discovered, provider);
    if ids.is_empty() {
        cx.backend
            .fill_round_rect(layout.models, 8.0, theme.background);
        cx.backend
            .stroke_round_rect(layout.models, 8.0, theme.border, 1.0);
        let hint = fit_text(
            cx,
            t_settings(ui, "settings.agents.cliNoModels"),
            layout.models.size.x - 32.0,
            LABEL_FONT,
        );
        text(
            cx,
            &hint,
            LABEL_FONT,
            theme.muted_foreground,
            400,
            SETTINGS_FONT_FAMILY,
            layout.models.origin.x + 16.0,
            chrome::centred_baseline(layout.models, LABEL_FONT),
        );
    } else {
        let rows: Vec<CheckRow<'_>> = ids
            .iter()
            .map(|id| CheckRow {
                label: id,
                checked: !settings.cli_model_hidden(provider, id),
            })
            .collect();
        paint_checklist(
            cx,
            theme,
            layout.models,
            &rows,
            settings.builtin_model_menu_scroll.offset,
            settings.builtin_model_menu_hover,
        );
    }
    paint_status(
        cx,
        theme,
        layout.footer.origin.x,
        layout.footer,
        Icon::Info,
        theme.muted_foreground,
        t_settings(ui, "settings.agents.checkedModelsNote"),
        (layout.connect.origin.x - 12.0 - layout.footer.origin.x).max(0.0),
    );
    let connected = settings.provider_verified_connected(provider);
    paint_button(
        cx,
        theme,
        layout.connect,
        t_settings(ui, connect_label(settings, provider)),
        None,
        if connected {
            ButtonKind::Outline
        } else {
            ButtonKind::Primary
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn detail() -> Rect {
        Rect::xywh(300.0, 100.0, 460.0, 600.0)
    }

    #[test]
    fn only_the_agents_own_models_are_listed() {
        let discovered = vec![
            ModelEntry::new(AgentProvider::ClaudeCode, "opus", "Opus"),
            ModelEntry::new(AgentProvider::CodexCli, "gpt-5.5", "GPT-5.5"),
            ModelEntry::new(AgentProvider::ClaudeCode, "sonnet", "Sonnet"),
        ];
        assert_eq!(
            cli_model_rows(&discovered, AgentProvider::ClaudeCode),
            ["opus", "sonnet"]
        );
    }

    #[test]
    fn check_again_rows_and_connect_resolve() {
        let settings = AgentSettings::default();
        let ui = EditorUiState::default();
        let discovered = vec![
            ModelEntry::new(AgentProvider::ClaudeCode, "opus", "Opus"),
            ModelEntry::new(AgentProvider::ClaudeCode, "sonnet", "Sonnet"),
        ];
        let layout = cli_layout(
            detail(),
            &settings,
            &ui,
            &discovered,
            AgentProvider::ClaudeCode,
        );
        let centre = |rect: Rect| {
            Point2D::new(
                rect.origin.x + rect.size.x / 2.0,
                rect.origin.y + rect.size.y / 2.0,
            )
        };
        assert_eq!(
            cli_hit(&settings, &layout, centre(layout.recheck)),
            CliHit::Recheck
        );
        assert_eq!(
            cli_hit(&settings, &layout, centre(layout.connect)),
            CliHit::Connect
        );
        let row = chrome::checklist_row_rect(layout.models, 1, 0.0);
        assert_eq!(
            cli_hit(
                &settings,
                &layout,
                Point2D::new(row.origin.x + 20.0, row.origin.y + 10.0)
            ),
            CliHit::ToggleModel(1)
        );
    }
}
