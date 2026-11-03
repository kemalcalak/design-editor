//! The Agents tab's CLI agent detail (#26): the probe status with Check
//! again, the agent's discovered models as a checklist (unticked ones
//! leave the chat picker, D6), and Connect / Disconnect. A missing agent's
//! card carries its whole install command with a copy button (#304).

use crate::theme::Theme;
use crate::widgets::agent_settings_detail_chrome::{
    self as chrome, button_width_estimate, checklist_height, checklist_row_at, paint_button,
    paint_checklist, paint_header, paint_label, paint_status, text, ButtonKind, CheckRow,
    BODY_TOP_GAP, BUTTON_H, FIELD_GAP, FOOTER_GAP, HEADER_H, LABEL_FONT, LABEL_H, VALUE_FONT,
};
use crate::widgets::agent_settings_i18n::t as t_settings;
use crate::widgets::agent_settings_install_command::{self as install, CommandLayout};
use crate::widgets::agent_settings_rows::{fit_text, SETTINGS_FONT_FAMILY};
use crate::widgets::icons::{draw_icon, Icon};
use crate::widgets::PaintCx;
use crate::{Point2D, Rect};
use de_editor_core::agent_settings::{AgentProvider, AgentSettings};
use de_editor_core::agent_settings_connection::ProviderConnectPhase;
use de_editor_core::editor_ui_state::EditorUiState;
use de_editor_core::ModelEntry;

const STATUS_H: f32 = 64.0;
const EMPTY_H: f32 = 52.0;
/// The not-installed card: the headline row above the command well.
const HEAD_ROW_H: f32 = 52.0;
const CARD_PAD: f32 = 14.0;
/// Glyph and gap before the headline text.
const HEADLINE_X: f32 = 16.0 + 18.0 + 14.0;
/// Check again stays on the headline row only while the headline keeps
/// this much room; below it the button drops under the command.
const MIN_HEADLINE_W: f32 = 96.0;

/// The agent's discovered model ids, in discovery order.
pub(crate) fn cli_model_rows(discovered: &[ModelEntry], provider: AgentProvider) -> Vec<&str> {
    agent_models(discovered, provider)
        .map(|entry| entry.value.as_str())
        .collect()
}

fn agent_models(
    discovered: &[ModelEntry],
    provider: AgentProvider,
) -> impl Iterator<Item = &ModelEntry> {
    discovered
        .iter()
        .filter(move |entry| entry.provider == provider && entry.builtin_provider_id.is_none())
}

/// The command to show when `provider` is known to be missing: the
/// not-installed state of [`status_lines`], with something to install.
pub(crate) fn install_command(settings: &AgentSettings, provider: AgentProvider) -> Option<&str> {
    let index = AgentSettings::provider_index(provider);
    let connection = &settings.provider_connection[index];
    let missing = !settings.provider_verified_connected_at(index)
        && connection.phase != ProviderConnectPhase::Probing
        && connection.not_installed;
    connection
        .install_command
        .as_deref()
        .filter(|command| missing && !command.trim().is_empty())
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct CliLayout {
    pub(crate) status: Rect,
    pub(crate) recheck: Rect,
    /// The install command's well, when the agent is missing.
    pub(crate) command: Option<CommandLayout>,
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
    let mut y = detail.origin.y + HEADER_H + BODY_TOP_GAP + LABEL_H;
    let recheck_w = button_width_estimate(t_settings(ui, "settings.agents.checkAgain"), true)
        .min((w - CARD_PAD * 2.0).max(0.0));
    let (status, recheck, command) = match install_command(settings, provider) {
        Some(text) => status_with_command(x, y, w, recheck_w, text),
        None => {
            let status = Rect::xywh(x, y, w, STATUS_H);
            let recheck = Rect::xywh(
                x + w - CARD_PAD - recheck_w,
                y + (STATUS_H - BUTTON_H) / 2.0,
                recheck_w,
                BUTTON_H,
            );
            (status, recheck, None)
        }
    };
    y += status.size.y + FIELD_GAP + LABEL_H;
    let model_rows = agent_models(discovered, provider).count();
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
        command,
        models,
        model_rows,
        footer,
        connect,
        height: y + BUTTON_H - detail.origin.y,
    }
}

/// The not-installed card at (`x`, `y`): headline row, then the command
/// well across the card, with Check again on the headline row when it
/// leaves the headline [`MIN_HEADLINE_W`], else under the well.
fn status_with_command(
    x: f32,
    y: f32,
    w: f32,
    recheck_w: f32,
    command: &str,
) -> (Rect, Rect, Option<CommandLayout>) {
    let well = install::command_layout(
        x + CARD_PAD,
        y + HEAD_ROW_H,
        (w - CARD_PAD * 2.0).max(0.0),
        command,
    );
    let well_bottom = well.well.origin.y + well.well.size.y;
    let beside = w - CARD_PAD - recheck_w - 12.0 - HEADLINE_X >= MIN_HEADLINE_W;
    let (recheck, bottom) = if beside {
        let recheck = Rect::xywh(
            x + w - CARD_PAD - recheck_w,
            y + (HEAD_ROW_H - BUTTON_H) / 2.0,
            recheck_w,
            BUTTON_H,
        );
        (recheck, well_bottom)
    } else {
        let recheck = Rect::xywh(x + CARD_PAD, well_bottom + 10.0, recheck_w, BUTTON_H);
        (recheck, well_bottom + 10.0 + BUTTON_H)
    };
    let status = Rect::xywh(x, y, w, bottom + CARD_PAD - y);
    (status, recheck, Some(well))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CliHit {
    Recheck,
    CopyInstall,
    ToggleModel(usize),
    Connect,
    None,
}

pub(crate) fn cli_hit(settings: &AgentSettings, layout: &CliLayout, point: Point2D) -> CliHit {
    if layout.recheck.contains(point) {
        return CliHit::Recheck;
    }
    if layout
        .command
        .is_some_and(|command| command.copy.contains(point))
    {
        return CliHit::CopyInstall;
    }
    if layout.model_rows > 0 {
        let scroll =
            chrome::checklist_scroll(settings.agents_checklist_scroll.offset, layout.model_rows);
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
    now_ms: u64,
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
    paint_status_card(cx, theme, settings, ui, &layout, provider, now_ms);
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
            chrome::checklist_scroll(settings.agents_checklist_scroll.offset, ids.len()),
            settings.agents_checklist_hover,
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

/// The status card's glyph, headline and detail line (or command well).
#[allow(clippy::too_many_arguments)]
fn paint_status_card(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    settings: &AgentSettings,
    ui: &EditorUiState,
    layout: &CliLayout,
    provider: AgentProvider,
    now_ms: u64,
) {
    let (icon, color, headline, detail_line) = status_lines(theme, ui, settings, provider);
    let glyph = 18.0;
    let x = layout.status.origin.x + 16.0;
    let text_x = layout.status.origin.x + HEADLINE_X;
    // The headline runs up to Check again when they share a row.
    let row_h = if layout.command.is_some() {
        HEAD_ROW_H
    } else {
        STATUS_H
    };
    let shares_row = layout.recheck.origin.y < layout.status.origin.y + row_h;
    let right = if shares_row {
        layout.recheck.origin.x - 12.0
    } else {
        layout.status.origin.x + layout.status.size.x - CARD_PAD
    };
    let budget = (right - text_x).max(0.0);
    draw_icon(
        cx.backend,
        icon,
        Point2D::new(x, layout.status.origin.y + (row_h - glyph) / 2.0),
        glyph,
        color,
        1.7,
    );
    let headline = fit_text(cx, &headline, budget, VALUE_FONT + 0.5);
    if let (Some(command_layout), Some(command)) =
        (layout.command, install_command(settings, provider))
    {
        let row = Rect::xywh(
            layout.status.origin.x,
            layout.status.origin.y,
            layout.status.size.x,
            HEAD_ROW_H,
        );
        text(
            cx,
            &headline,
            VALUE_FONT + 0.5,
            theme.foreground,
            600,
            SETTINGS_FONT_FAMILY,
            text_x,
            chrome::centred_baseline(row, VALUE_FONT + 0.5),
        );
        let copied = install::copied(settings.install_command_copied.as_ref(), command, now_ms);
        install::paint_command(cx, theme, &command_layout, command, copied);
        return;
    }
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
        text(
            cx,
            &line,
            LABEL_FONT,
            theme.muted_foreground,
            400,
            SETTINGS_FONT_FAMILY,
            text_x,
            layout.status.origin.y + 46.0,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn detail() -> Rect {
        Rect::xywh(300.0, 100.0, 460.0, 600.0)
    }

    fn overlaps(a: Rect, b: Rect) -> bool {
        a.origin.x < b.origin.x + b.size.x
            && b.origin.x < a.origin.x + a.size.x
            && a.origin.y < b.origin.y + b.size.y
            && b.origin.y < a.origin.y + a.size.y
    }

    fn inside(inner: Rect, outer: Rect) -> bool {
        inner.origin.x >= outer.origin.x - 0.01
            && inner.origin.y >= outer.origin.y - 0.01
            && inner.origin.x + inner.size.x <= outer.origin.x + outer.size.x + 0.01
            && inner.origin.y + inner.size.y <= outer.origin.y + outer.size.y + 0.01
    }

    /// #304: at every detail width down to the 620 px modal's (~96 px), in
    /// every locale, Check again never covers the install command's well,
    /// the well and button stay inside the card, the copy button resolves,
    /// and the wrapped lines carry the whole command.
    #[test]
    fn the_install_command_is_never_covered_by_check_again() {
        let commands = [
            "npm install -g @anthropic-ai/claude-code",
            "curl -fsSL https://antigravity.google/cli/install.sh | bash",
            "irm https://antigravity.google/cli/install.ps1 | iex",
            "brew install github/copilot/copilot",
        ];
        for locale in de_i18n::Locale::ALL {
            let ui = EditorUiState {
                locale,
                ..EditorUiState::default()
            };
            for command in commands {
                let mut settings = AgentSettings::default();
                let connection =
                    &mut settings.provider_connection[AgentProvider::ClaudeCode.index()];
                connection.not_installed = true;
                connection.install_command = Some(command.into());
                for w in [96.0, 140.0, 220.0, 300.0, 460.0, 660.0] {
                    let detail = Rect::xywh(300.0, 100.0, w, 600.0);
                    let layout =
                        cli_layout(detail, &settings, &ui, &[], AgentProvider::ClaudeCode);
                    let well = layout.command.expect("a missing agent shows its command");
                    let case = format!("{locale:?} {command} w={w}");
                    assert!(!overlaps(layout.recheck, well.well), "{case}");
                    assert!(inside(well.well, layout.status), "{case}");
                    assert!(inside(layout.recheck, layout.status), "{case}");
                    let lines = install::wrap_command(command, well.columns);
                    assert_eq!(
                        lines.concat().replace(' ', ""),
                        command.replace(' ', ""),
                        "{case}"
                    );
                    let centre = Point2D::new(
                        well.copy.origin.x + well.copy.size.x / 2.0,
                        well.copy.origin.y + well.copy.size.y / 2.0,
                    );
                    assert_eq!(cli_hit(&settings, &layout, centre), CliHit::CopyInstall);
                    assert!(layout.models.origin.y > layout.status.origin.y + layout.status.size.y);
                }
            }
        }
    }

    #[test]
    fn an_installed_agent_keeps_the_compact_card() {
        let settings = AgentSettings::default();
        let layout = cli_layout(
            detail(),
            &settings,
            &EditorUiState::default(),
            &[],
            AgentProvider::ClaudeCode,
        );
        assert!(layout.command.is_none());
        assert_eq!(layout.status.size.y, STATUS_H);
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
