//! The Agents tab's ACP agent detail (#26): for a new agent a preset
//! picker, then the connection type, the command and arguments (or the
//! remote URL), the install hint for a preset that is missing, and the
//! environment. The footer connects a saved agent or adds the draft.

use crate::theme::Theme;
use crate::widgets::agent_settings_caret;
use crate::widgets::agent_settings_detail_chrome::{
    self as chrome, button_width_estimate, next_field, paint_button, paint_header,
    paint_icon_button, paint_input_box, paint_input_value, paint_label, paint_segmented,
    paint_status, segment_at, segmented_width, text, ButtonKind, BODY_TOP_GAP, BUTTON_H, FIELD_GAP,
    FOOTER_GAP, HEADER_H, INPUT_H, INPUT_PAD_X, LABEL_FONT, VALUE_FONT,
};
use crate::widgets::agent_settings_i18n::t as t_settings;
use crate::widgets::agent_settings_rows::fit_text;
use crate::widgets::home_surface::MONO;
use crate::widgets::icons::{draw_icon, Icon};
use crate::widgets::PaintCx;
use crate::{Point2D, Rect};
use de_editor_core::acp_agent_presets::{
    acp_agent_preset, matches_preset_transport, AcpAgentPreset, ACP_AGENT_PRESETS,
};
use de_editor_core::agent_settings::{
    AcpAgentConfig, AcpAgentConnectPhase, AcpAgentField, AcpConnectionType, AgentSettings,
    AgentsEntry, SettingsFocus,
};
use de_editor_core::editor_ui_state::EditorUiState;

const HINT_H: f32 = 40.0;
/// Space between the Command and Arguments inputs.
const ARGS_GAP: f32 = 16.0;

/// The agent the pane edits: a saved one by index, or the draft.
#[derive(Clone, Copy)]
pub(crate) struct AcpTarget<'a> {
    pub(crate) agent: &'a AcpAgentConfig,
    pub(crate) index: Option<usize>,
}

impl AcpTarget<'_> {
    fn focus(&self, field: AcpAgentField) -> SettingsFocus {
        match self.index {
            Some(index) => SettingsFocus::AcpAgent { index, field },
            None => SettingsFocus::AcpAgentDraft(field),
        }
    }

    /// The preset this agent's transport is, if any.
    fn preset(&self) -> Option<&'static AcpAgentPreset> {
        acp_agent_preset(&self.agent.id).or_else(|| {
            ACP_AGENT_PRESETS.iter().find(|preset| {
                matches_preset_transport(preset, &self.agent.command, &self.agent.args)
            })
        })
    }
}

pub(crate) fn acp_target<'a>(
    settings: &'a AgentSettings,
    entry: &AgentsEntry,
) -> Option<AcpTarget<'a>> {
    match entry {
        AgentsEntry::Acp(id) => settings
            .acp_agents
            .iter()
            .enumerate()
            .find(|(_, agent)| agent.id == *id)
            .map(|(index, agent)| AcpTarget {
                agent,
                index: Some(index),
            }),
        AgentsEntry::AcpDraft => settings
            .acp_agent_draft
            .as_ref()
            .map(|agent| AcpTarget { agent, index: None }),
        _ => None,
    }
}

fn preset_labels(ui: &EditorUiState) -> Vec<&str> {
    let mut labels: Vec<&str> = ACP_AGENT_PRESETS
        .iter()
        .map(|preset| preset.display_name)
        .collect();
    labels.push(t_settings(ui, "builtin.custom"));
    labels
}

fn connection_labels(ui: &EditorUiState) -> [&str; 2] {
    [
        t_settings(ui, "settings.agents.localCommand"),
        t_settings(ui, "settings.agents.remoteUrl"),
    ]
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct AcpLayout {
    pub(crate) preset: Option<Rect>,
    pub(crate) connection: Rect,
    pub(crate) name: Rect,
    pub(crate) command: Option<Rect>,
    pub(crate) args: Option<Rect>,
    pub(crate) hint: Option<Rect>,
    pub(crate) env: Option<Rect>,
    pub(crate) url: Option<Rect>,
    pub(crate) footer: Rect,
    pub(crate) primary: Rect,
    pub(crate) secondary: Rect,
    pub(crate) height: f32,
}

/// Whether to show the install line: a preset agent whose binary is not
/// known to be on PATH (a host that cannot look reports nothing).
fn shows_install_hint(settings: &AgentSettings, target: AcpTarget<'_>) -> bool {
    target.agent.connection_type == AcpConnectionType::Local
        && target
            .preset()
            .is_some_and(|preset| settings.acp_preset_installed.get(preset.id) == Some(&false))
}

fn primary_label<'a>(
    ui: &'a EditorUiState,
    settings: &AgentSettings,
    target: AcpTarget<'_>,
) -> &'a str {
    match target.index {
        None => t_settings(ui, "settings.agents.addAgent"),
        Some(index) if settings.acp_agent_verified_connected_at(index) => {
            t_settings(ui, "common.disconnect")
        }
        Some(_) => t_settings(ui, "common.connect"),
    }
}

pub(crate) fn acp_layout(
    detail: Rect,
    settings: &AgentSettings,
    ui: &EditorUiState,
    target: AcpTarget<'_>,
) -> AcpLayout {
    let x = detail.origin.x;
    let w = detail.size.x;
    let mut y = detail.origin.y + HEADER_H + BODY_TOP_GAP;
    let preset = target.index.is_none().then(|| {
        next_field(
            x,
            &mut y,
            segmented_width(&preset_labels(ui)).min(w),
            INPUT_H,
        )
    });
    let connection = next_field(
        x,
        &mut y,
        segmented_width(&connection_labels(ui)).min(w),
        INPUT_H,
    );
    let name = next_field(x, &mut y, w, INPUT_H);
    let (command, args, hint, env, url) = match target.agent.connection_type {
        AcpConnectionType::Local => {
            let half = (w - ARGS_GAP) / 2.0;
            let command = next_field(x, &mut y, half, INPUT_H);
            let args = Rect::xywh(x + half + ARGS_GAP, command.origin.y, half, INPUT_H);
            // The install line hangs under the command row, closer than a
            // field gap: it explains that row, it is not a field of its own.
            let hint = shows_install_hint(settings, target).then(|| {
                y -= FIELD_GAP / 2.0;
                let rect = Rect::xywh(x, y, w, HINT_H);
                y += HINT_H + FIELD_GAP;
                rect
            });
            let env = next_field(x, &mut y, w, INPUT_H);
            (Some(command), Some(args), hint, Some(env), None)
        }
        AcpConnectionType::Remote => (
            None,
            None,
            None,
            None,
            Some(next_field(x, &mut y, w, INPUT_H)),
        ),
    };
    let footer = Rect::xywh(x, y - FIELD_GAP + FOOTER_GAP, w, BUTTON_H);
    let primary_w = button_width_estimate(primary_label(ui, settings, target), false);
    let primary = Rect::xywh(x + w - primary_w, footer.origin.y, primary_w, BUTTON_H);
    let secondary_w = match target.index {
        Some(_) => BUTTON_H,
        None => button_width_estimate(t_settings(ui, "common.cancel"), false),
    };
    let secondary = Rect::xywh(
        primary.origin.x - 10.0 - secondary_w,
        footer.origin.y,
        secondary_w,
        BUTTON_H,
    );
    AcpLayout {
        preset,
        connection,
        name,
        command,
        args,
        hint,
        env,
        url,
        footer,
        primary,
        secondary,
        height: footer.origin.y + BUTTON_H - detail.origin.y,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AcpHit {
    /// A preset segment (`None` = Custom).
    SelectPreset(Option<usize>),
    SetConnection(AcpConnectionType),
    Focus(AcpAgentField),
    Primary,
    Secondary,
    None,
}

pub(crate) fn acp_hit(layout: &AcpLayout, point: Point2D) -> AcpHit {
    if let Some(rect) = layout.preset {
        if let Some(segment) = segment_at(rect, ACP_AGENT_PRESETS.len() + 1, point) {
            return AcpHit::SelectPreset((segment < ACP_AGENT_PRESETS.len()).then_some(segment));
        }
    }
    if let Some(segment) = segment_at(layout.connection, 2, point) {
        return AcpHit::SetConnection(if segment == 0 {
            AcpConnectionType::Local
        } else {
            AcpConnectionType::Remote
        });
    }
    for (rect, field) in [
        (Some(layout.name), AcpAgentField::DisplayName),
        (layout.command, AcpAgentField::Command),
        (layout.args, AcpAgentField::Args),
        (layout.env, AcpAgentField::Env),
        (layout.url, AcpAgentField::Url),
    ] {
        if rect.is_some_and(|rect| rect.contains(point)) {
            return AcpHit::Focus(field);
        }
    }
    if layout.primary.contains(point) {
        return AcpHit::Primary;
    }
    if layout.secondary.contains(point) {
        return AcpHit::Secondary;
    }
    AcpHit::None
}

pub(crate) fn focused_input_rect(layout: &AcpLayout, field: AcpAgentField) -> Option<Rect> {
    match field {
        AcpAgentField::DisplayName => Some(layout.name),
        AcpAgentField::Command => layout.command,
        AcpAgentField::Args => layout.args,
        AcpAgentField::Env => layout.env,
        AcpAgentField::Url => layout.url,
    }
}

fn field_value(agent: &AcpAgentConfig, field: AcpAgentField) -> String {
    match field {
        AcpAgentField::DisplayName => agent.display_name.clone(),
        AcpAgentField::Command => agent.command.clone(),
        AcpAgentField::Args => agent.args_text(),
        AcpAgentField::Env => agent.env_text().replace('\n', ", "),
        AcpAgentField::Url => agent.url.clone().unwrap_or_default(),
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn paint_acp_detail(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    settings: &AgentSettings,
    ui: &EditorUiState,
    detail: Rect,
    target: AcpTarget<'_>,
    now_ms: u64,
) {
    let layout = acp_layout(detail, settings, ui, target);
    let agent = target.agent;
    let title = match target.index {
        Some(_) => agent.display_name.as_str(),
        None => t_settings(ui, "settings.agents.addAcpAgent"),
    };
    paint_header(
        cx,
        theme,
        detail,
        title,
        t_settings(ui, "settings.agents.acpDetailSubtitle"),
        None,
    );
    if let Some(rect) = layout.preset {
        paint_label(cx, theme, t_settings(ui, "settings.agents.preset"), rect);
        let active = target
            .preset()
            .and_then(|preset| ACP_AGENT_PRESETS.iter().position(|p| p.id == preset.id))
            .unwrap_or(ACP_AGENT_PRESETS.len());
        paint_segmented(cx, theme, rect, &preset_labels(ui), Some(active));
    }
    paint_label(
        cx,
        theme,
        t_settings(ui, "settings.agents.connection"),
        layout.connection,
    );
    paint_segmented(
        cx,
        theme,
        layout.connection,
        &connection_labels(ui),
        Some(match agent.connection_type {
            AcpConnectionType::Local => 0,
            AcpConnectionType::Remote => 1,
        }),
    );
    let fields = [
        (
            Some(layout.name),
            AcpAgentField::DisplayName,
            "acp.displayName",
            "acp.displayNamePlaceholder",
            false,
        ),
        (
            layout.command,
            AcpAgentField::Command,
            "acp.command",
            "acp.commandPlaceholder",
            true,
        ),
        (
            layout.args,
            AcpAgentField::Args,
            "acp.args",
            "acp.argsPlaceholder",
            true,
        ),
        (
            layout.env,
            AcpAgentField::Env,
            "settings.agents.envOptional",
            "acp.envPlaceholder",
            true,
        ),
        (
            layout.url,
            AcpAgentField::Url,
            "acp.url",
            "acp.urlPlaceholder",
            true,
        ),
    ];
    for (rect, field, label, placeholder, mono) in fields {
        let Some(rect) = rect else { continue };
        paint_label(cx, theme, t_settings(ui, label), rect);
        let focused = settings.focus == Some(target.focus(field));
        paint_input_box(cx, theme, rect, focused);
        if focused {
            agent_settings_caret::paint_settings_input_view(
                cx,
                theme,
                ui,
                rect,
                VALUE_FONT,
                INPUT_PAD_X,
                chrome::centred_baseline(rect, VALUE_FONT),
                now_ms,
                t_settings(ui, placeholder),
            );
        } else {
            paint_input_value(
                cx,
                theme,
                rect,
                &field_value(agent, field),
                t_settings(ui, placeholder),
                mono,
                0.0,
            );
        }
    }
    if let (Some(hint), Some(preset)) = (layout.hint, target.preset()) {
        cx.backend.fill_round_rect(hint, 8.0, theme.muted);
        draw_icon(
            cx.backend,
            Icon::Terminal,
            Point2D::new(hint.origin.x + 12.0, hint.origin.y + (HINT_H - 15.0) / 2.0),
            15.0,
            theme.muted_foreground,
            1.6,
        );
        let line = format!(
            "{} {}",
            t_settings(ui, "settings.agents.notInstalledHint"),
            preset.install_hint
        );
        let line = fit_text(cx, &line, hint.size.x - 48.0, LABEL_FONT);
        text(
            cx,
            &line,
            LABEL_FONT,
            theme.muted_foreground,
            400,
            MONO,
            hint.origin.x + 36.0,
            chrome::centred_baseline(hint, LABEL_FONT),
        );
    }
    paint_footer(cx, theme, settings, ui, &layout, target);
}

fn paint_footer(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    settings: &AgentSettings,
    ui: &EditorUiState,
    layout: &AcpLayout,
    target: AcpTarget<'_>,
) {
    let connection = target
        .index
        .map(|_| settings.acp_agent_connection_for(&target.agent.id));
    let connected = target
        .index
        .is_some_and(|index| settings.acp_agent_verified_connected_at(index));
    let (icon, color, label) = match &connection {
        _ if connected => (
            Icon::CheckCircle,
            theme.status_success,
            t_settings(ui, "settings.agents.statusConnected").to_string(),
        ),
        Some(connection) if connection.phase == AcpAgentConnectPhase::Probing => (
            Icon::RefreshCw,
            theme.status_warning,
            t_settings(ui, "settings.agents.statusChecking").to_string(),
        ),
        Some(connection) if connection.error.is_some() => (
            Icon::AlertTriangle,
            theme.destructive,
            connection.error.clone().unwrap_or_default(),
        ),
        _ => (
            Icon::Info,
            theme.muted_foreground,
            t_settings(ui, "settings.agents.notTestedYet").to_string(),
        ),
    };
    paint_status(
        cx,
        theme,
        layout.footer.origin.x,
        layout.footer,
        icon,
        color,
        &label,
        (layout.secondary.origin.x - 12.0 - layout.footer.origin.x).max(0.0),
    );
    match target.index {
        Some(_) => paint_icon_button(cx, theme, layout.secondary, Icon::Trash, theme.destructive),
        None => paint_button(
            cx,
            theme,
            layout.secondary,
            t_settings(ui, "common.cancel"),
            None,
            ButtonKind::Outline,
        ),
    }
    paint_button(
        cx,
        theme,
        layout.primary,
        primary_label(ui, settings, target),
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
    fn the_draft_picks_a_preset_and_a_connection() {
        let mut settings = AgentSettings::default();
        settings.begin_acp_agent_draft();
        let ui = EditorUiState::default();
        let target = acp_target(&settings, &AgentsEntry::AcpDraft).unwrap();
        let layout = acp_layout(detail(), &settings, &ui, target);
        let preset = layout.preset.unwrap();
        let first = Point2D::new(preset.origin.x + 10.0, preset.origin.y + 10.0);
        assert_eq!(acp_hit(&layout, first), AcpHit::SelectPreset(Some(0)));
        let last = Point2D::new(
            preset.origin.x + preset.size.x - 10.0,
            preset.origin.y + 10.0,
        );
        assert_eq!(acp_hit(&layout, last), AcpHit::SelectPreset(None));
        let remote = Point2D::new(
            layout.connection.origin.x + layout.connection.size.x - 10.0,
            layout.connection.origin.y + 10.0,
        );
        assert_eq!(
            acp_hit(&layout, remote),
            AcpHit::SetConnection(AcpConnectionType::Remote)
        );
    }

    #[test]
    fn local_and_remote_show_their_own_fields() {
        let mut settings = AgentSettings::default();
        let index = settings.add_acp_agent_preset("gemini-cli").unwrap();
        let ui = EditorUiState::default();
        let id = settings.acp_agents[index].id.clone();
        let target = acp_target(&settings, &AgentsEntry::Acp(id.clone())).unwrap();
        let layout = acp_layout(detail(), &settings, &ui, target);
        assert!(layout.preset.is_none());
        let args = layout.args.unwrap();
        assert_eq!(
            acp_hit(
                &layout,
                Point2D::new(args.origin.x + 10.0, args.origin.y + 10.0)
            ),
            AcpHit::Focus(AcpAgentField::Args)
        );
        assert!(layout.url.is_none());
        settings.set_acp_connection_type(Some(index), AcpConnectionType::Remote);
        let target = acp_target(&settings, &AgentsEntry::Acp(id)).unwrap();
        let layout = acp_layout(detail(), &settings, &ui, target);
        assert!(layout.command.is_none() && layout.url.is_some());
    }

    #[test]
    fn a_missing_preset_binary_shows_its_install_line() {
        let mut settings = AgentSettings::default();
        let index = settings.add_acp_agent_preset("qwen-code").unwrap();
        let ui = EditorUiState::default();
        let id = settings.acp_agents[index].id.clone();
        let target = acp_target(&settings, &AgentsEntry::Acp(id.clone())).unwrap();
        assert!(acp_layout(detail(), &settings, &ui, target).hint.is_none());
        settings
            .acp_preset_installed
            .insert("qwen-code".to_string(), false);
        let target = acp_target(&settings, &AgentsEntry::Acp(id)).unwrap();
        assert!(acp_layout(detail(), &settings, &ui, target).hint.is_some());
    }
}
