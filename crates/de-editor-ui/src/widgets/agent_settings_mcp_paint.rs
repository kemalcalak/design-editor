//! Paint pass for the MCP settings tab; geometry lives in the parent
//! `agent_settings_mcp.rs`.

use super::*;
use crate::theme::Theme;
use crate::widgets::agent_settings_caret::paint_settings_input_view;
use crate::widgets::agent_settings_i18n::t as t_settings;
use crate::widgets::agent_settings_rows::{fit_text, paint_tab_intro, SETTINGS_FONT_FAMILY};
use crate::widgets::home_surface::MONO;
use crate::widgets::icons::{draw_icon, Icon};
use crate::widgets::PaintCx;
use crate::{Color, TextLayout};
use de_editor_core::agent_settings::SettingsFocus;
use de_editor_core::editor_ui_state::EditorUiState;
use de_editor_core::home_mcp_setup::{home_mcp_agent_name, home_mcp_config_path};
use de_editor_core::{AgentSettingsButton, ButtonPressTarget};
use jian_widgets::centered_text_baseline_y;

fn text(
    cx: &mut PaintCx<'_>,
    content: &str,
    family: &str,
    size: f32,
    color: Color,
    weight: u16,
    origin: Point2D,
) {
    if content.is_empty() {
        return;
    }
    let layout = TextLayout::single_run(content, family, size, color.to_jian(), Point2D::ZERO)
        .with_font_weight(weight);
    cx.backend.draw_text(&layout, origin);
}

fn caption(cx: &mut PaintCx<'_>, theme: &Theme, label: &str, x: f32, y: f32) {
    text(
        cx,
        &label.to_uppercase(),
        SETTINGS_FONT_FAMILY,
        10.5,
        theme.muted_foreground,
        600,
        Point2D::new(x, y),
    );
}

fn outlined(cx: &mut PaintCx<'_>, theme: &Theme, rect: Rect, radius: f32, fill: Color) {
    cx.backend.fill_round_rect(rect, radius, fill);
    cx.backend
        .stroke_round_rect(rect, radius, theme.border, 1.0);
}

fn copy_chip(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    ui: &EditorUiState,
    rect: Rect,
    copied: bool,
    hovered: bool,
    pressed: bool,
) {
    outlined(cx, theme, rect, 6.0, theme.card);
    crate::widgets::button::paint_ghost_button_feedback(cx.backend, theme, rect, hovered, pressed);
    let icon = if copied { Icon::Check } else { Icon::Copy };
    let icon_size = 12.0;
    draw_icon(
        cx.backend,
        icon,
        Point2D::new(
            rect.origin.x + 8.0,
            rect.origin.y + (rect.size.y - icon_size) / 2.0,
        ),
        icon_size,
        if copied {
            theme.status_success
        } else {
            theme.muted_foreground
        },
        1.7,
    );
    let label = t_settings(
        ui,
        if copied {
            "home.mcp.copied"
        } else {
            "home.mcp.copy"
        },
    );
    text(
        cx,
        label,
        SETTINGS_FONT_FAMILY,
        11.0,
        theme.muted_foreground,
        500,
        Point2D::new(
            rect.origin.x + 8.0 + icon_size + 5.0,
            centered_text_baseline_y(rect, 11.0),
        ),
    );
}

pub(in crate::widgets) fn paint_mcp_tab(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    settings: &AgentSettings,
    ui: &EditorUiState,
    content: Rect,
    now_ms: u64,
    external_cli_available: bool,
) {
    paint_tab_intro(
        cx,
        theme,
        content,
        t_settings(ui, "settings.tab.mcp"),
        Some(t_settings(ui, "settings.mcp.heroSubtitle")),
    );
    paint_server_card(cx, theme, settings, ui, content, now_ms);
    if cli_integrations_visible(external_cli_available) {
        paint_agents(cx, theme, settings, ui, content, now_ms);
    }
    paint_connected(cx, theme, settings, ui, content, external_cli_available);
}

fn paint_server_card(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    settings: &AgentSettings,
    ui: &EditorUiState,
    content: Rect,
    now_ms: u64,
) {
    let card = server_card_rect(content);
    outlined(cx, theme, card, 10.0, theme.background);
    // Host-managed (embed): the extension's proxy is alive by the time any
    // editor mounts — the card reads always-running at the host's port.
    let host_managed = settings.mcp_host_managed();
    let running = host_managed || settings.mcp_server.running;
    let x = card.origin.x + CARD_PAD;
    cx.backend.fill_oval(
        Rect::xywh(x, card.origin.y + 21.0, 8.0, 8.0),
        if running {
            theme.status_success
        } else {
            theme.muted_foreground
        },
    );
    let title = format!(
        "{} · {}",
        t_settings(ui, "settings.mcp.server"),
        t_settings(
            ui,
            if running {
                "settings.mcp.running"
            } else {
                "settings.mcp.stopped"
            }
        )
    );
    text(
        cx,
        &title,
        SETTINGS_FONT_FAMILY,
        13.0,
        theme.foreground,
        600,
        Point2D::new(x + 16.0, card.origin.y + 29.0),
    );
    let url = endpoint_url(settings);
    let line = endpoint_rect(content);
    text(
        cx,
        &url,
        MONO,
        ENDPOINT_FONT,
        theme.muted_foreground,
        400,
        Point2D::new(line.origin.x, centered_text_baseline_y(line, ENDPOINT_FONT)),
    );
    copy_chip(
        cx,
        theme,
        ui,
        endpoint_copy_rect(content, &url),
        copy_feedback_active(settings.mcp_client_config_copied_at_ms, now_ms),
        settings.hover_mcp_client_config_copy,
        ui.button_pressed(ButtonPressTarget::AgentSettings(
            AgentSettingsButton::McpClientConfigCopy,
        )),
    );

    // Port: a field while the server is stopped, read-only while it runs.
    let field = port_field_rect(content);
    let port_editable = !running && !host_managed;
    let focused = port_editable && matches!(settings.focus, Some(SettingsFocus::McpPort));
    let label = t_settings(ui, "settings.mcp.port");
    let label_w = cx
        .backend
        .measure_text_family(label, 12.0, SETTINGS_FONT_FAMILY);
    text(
        cx,
        label,
        SETTINGS_FONT_FAMILY,
        12.0,
        theme.muted_foreground,
        400,
        Point2D::new(
            field.origin.x - 8.0 - label_w,
            centered_text_baseline_y(field, 12.0),
        ),
    );
    cx.backend.fill_round_rect(field, 7.0, theme.card);
    cx.backend.stroke_round_rect(
        field,
        7.0,
        if focused { theme.ring } else { theme.border },
        if focused { 1.5 } else { 1.0 },
    );
    let port = if focused {
        ui.settings_input.text().to_owned()
    } else if host_managed {
        settings
            .embed_mcp_port_text()
            .unwrap_or_default()
            .to_owned()
    } else {
        settings.mcp_server.port.to_string()
    };
    let baseline = centered_text_baseline_y(field, 12.0);
    if focused {
        paint_settings_input_view(cx, theme, ui, field, 12.0, 10.0, baseline, now_ms, "");
    } else {
        text(
            cx,
            &port,
            MONO,
            12.0,
            if port_editable {
                theme.foreground
            } else {
                theme.muted_foreground
            },
            400,
            Point2D::new(field.origin.x + 10.0, baseline),
        );
    }

    // Host-managed: no start/stop affordance — the extension owns the
    // lifecycle (hit_test skips ToggleServer under the same gate).
    if host_managed {
        return;
    }
    let btn = server_button_rect(content);
    let pressed = ui.button_pressed(ButtonPressTarget::AgentSettings(
        AgentSettingsButton::McpServer,
    ));
    let hovered = settings.hover_mcp_server_button;
    if running {
        outlined(cx, theme, btn, 7.0, theme.card);
    } else {
        cx.backend.fill_round_rect(btn, 7.0, theme.primary);
    }
    crate::widgets::button::paint_ghost_button_feedback(cx.backend, theme, btn, hovered, pressed);
    let label = t_settings(
        ui,
        if running {
            "settings.mcp.stop"
        } else {
            "settings.mcp.start"
        },
    );
    let label = fit_text(cx, label, BTN_W - 12.0, 12.0);
    let w = cx
        .backend
        .measure_text_family(&label, 12.0, SETTINGS_FONT_FAMILY);
    text(
        cx,
        &label,
        SETTINGS_FONT_FAMILY,
        12.0,
        if running {
            theme.foreground
        } else {
            theme.primary_foreground
        },
        600,
        Point2D::new(
            btn.origin.x + (btn.size.x - w) / 2.0,
            centered_text_baseline_y(btn, 12.0),
        ),
    );
}

fn paint_agents(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    settings: &AgentSettings,
    ui: &EditorUiState,
    content: Rect,
    now_ms: u64,
) {
    let head_y = agents_header_top(content);
    caption(
        cx,
        theme,
        t_settings(ui, "settings.mcp.agents"),
        content.origin.x,
        head_y + 18.0,
    );
    let hint = t_settings(ui, "settings.mcp.agentsHint");
    let hint = fit_text(cx, hint, content.size.x - 120.0, 11.0);
    let hint_w = cx
        .backend
        .measure_text_family(&hint, 11.0, SETTINGS_FONT_FAMILY);
    text(
        cx,
        &hint,
        SETTINGS_FONT_FAMILY,
        11.0,
        theme.muted_foreground,
        400,
        Point2D::new(content.origin.x + content.size.x - hint_w, head_y + 18.0),
    );

    let frame = list_frame(content, settings);
    outlined(cx, theme, frame, 10.0, theme.background);
    let rows = agent_rows(content, settings);
    for (index, row) in rows.iter().enumerate() {
        if index > 0 {
            cx.backend.fill_rect(
                Rect::xywh(
                    row.row.origin.x + 1.0,
                    row.row.origin.y,
                    row.row.size.x - 2.0,
                    1.0,
                ),
                theme.border,
            );
        }
        paint_agent_row(cx, theme, settings, ui, content, row, now_ms);
    }

    let more = show_more_rect(content, settings);
    cx.backend.fill_rect(
        Rect::xywh(more.origin.x + 1.0, more.origin.y, more.size.x - 2.0, 1.0),
        theme.border,
    );
    let hidden = McpCli::DISPLAY.len() - COLLAPSED_ROWS;
    let label = if settings.mcp_show_all_agents {
        t_settings(ui, "settings.mcp.showFewer").to_string()
    } else {
        t_settings(ui, "settings.mcp.showMore").replace("{{count}}", &hidden.to_string())
    };
    text(
        cx,
        &label,
        SETTINGS_FONT_FAMILY,
        12.0,
        theme.primary,
        500,
        Point2D::new(more.origin.x + 14.0, centered_text_baseline_y(more, 12.0)),
    );
}

#[allow(clippy::too_many_arguments)]
fn paint_agent_row(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    settings: &AgentSettings,
    ui: &EditorUiState,
    content: Rect,
    row: &AgentRow,
    now_ms: u64,
) {
    let expanded = row.snippet.size.y > 0.0;
    if expanded {
        let block = Rect::xywh(
            row.row.origin.x + 1.0,
            row.row.origin.y + 1.0,
            row.row.size.x - 2.0,
            row.snippet.origin.y + row.snippet.size.y + 13.0 - row.row.origin.y,
        );
        cx.backend.fill_rect(block, theme.card);
    }
    let r = row.row;
    draw_icon(
        cx.backend,
        if expanded {
            Icon::ChevronDown
        } else {
            Icon::ChevronRight
        },
        Point2D::new(r.origin.x + 14.0, r.origin.y + (r.size.y - 14.0) / 2.0),
        14.0,
        theme.muted_foreground,
        1.6,
    );
    let name_x = r.origin.x + 40.0;
    text(
        cx,
        home_mcp_agent_name(row.cli),
        SETTINGS_FONT_FAMILY,
        13.0,
        theme.foreground,
        500,
        Point2D::new(name_x, r.origin.y + 20.0),
    );
    text(
        cx,
        home_mcp_config_path(row.cli),
        MONO,
        10.5,
        theme.muted_foreground,
        400,
        Point2D::new(name_x, r.origin.y + 36.0),
    );

    let enabled = settings.mcp_cli_enabled[row.cli.index()];
    let action = row.action;
    if enabled {
        let added = t_settings(ui, "home.mcp.added");
        let added_w = cx
            .backend
            .measure_text_family(added, 12.0, SETTINGS_FONT_FAMILY);
        let added_x = action.origin.x - 10.0 - added_w;
        draw_icon(
            cx.backend,
            Icon::Check,
            Point2D::new(added_x - 18.0, action.origin.y + 7.5),
            13.0,
            theme.status_success,
            2.0,
        );
        text(
            cx,
            added,
            SETTINGS_FONT_FAMILY,
            12.0,
            theme.status_success,
            600,
            Point2D::new(added_x, centered_text_baseline_y(action, 12.0)),
        );
        outlined(cx, theme, action, 7.0, theme.card);
        let label = t_settings(ui, "home.mcp.remove");
        let w = cx
            .backend
            .measure_text_family(label, 12.0, SETTINGS_FONT_FAMILY);
        text(
            cx,
            label,
            SETTINGS_FONT_FAMILY,
            12.0,
            theme.muted_foreground,
            500,
            Point2D::new(
                action.origin.x + (action.size.x - w) / 2.0,
                centered_text_baseline_y(action, 12.0),
            ),
        );
    } else {
        cx.backend.fill_round_rect(action, 7.0, theme.primary);
        let label = t_settings(ui, "home.mcp.setUp");
        let label = fit_text(cx, label, action.size.x - 34.0, 12.0);
        draw_icon(
            cx.backend,
            Icon::Link,
            Point2D::new(action.origin.x + 12.0, action.origin.y + 8.0),
            12.0,
            theme.primary_foreground,
            1.8,
        );
        text(
            cx,
            &label,
            SETTINGS_FONT_FAMILY,
            12.0,
            theme.primary_foreground,
            600,
            Point2D::new(
                action.origin.x + 30.0,
                centered_text_baseline_y(action, 12.0),
            ),
        );
    }

    if !expanded {
        return;
    }
    let snippet = row.snippet;
    outlined(cx, theme, snippet, 8.0, theme.background);
    for (i, line) in snippet_lines(content, settings, row.cli).iter().enumerate() {
        text(
            cx,
            line,
            MONO,
            SNIPPET_FONT,
            theme.foreground,
            400,
            Point2D::new(
                snippet.origin.x + 12.0,
                snippet.origin.y + 8.0 + i as f32 * SNIPPET_LINE + 12.5,
            ),
        );
    }
    copy_chip(
        cx,
        theme,
        ui,
        row.snippet_copy,
        copy_feedback_active(settings.mcp_snippet_copied_at_ms, now_ms),
        false,
        false,
    );
}

fn paint_connected(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    settings: &AgentSettings,
    ui: &EditorUiState,
    content: Rect,
    external_cli_available: bool,
) {
    let rect = connected_rect(content, settings, external_cli_available);
    let baseline = centered_text_baseline_y(rect, 12.0);
    caption(
        cx,
        theme,
        t_settings(ui, "home.mcp.connectedHeader"),
        rect.origin.x,
        baseline,
    );
    let label_w = cx.backend.measure_text_family(
        &t_settings(ui, "home.mcp.connectedHeader").to_uppercase(),
        10.5,
        SETTINGS_FONT_FAMILY,
    );
    let x = rect.origin.x + label_w + 12.0;
    match &ui.home.mcp_client {
        Some(name) => {
            cx.backend.fill_oval(
                Rect::xywh(x, rect.origin.y + 6.5, 7.0, 7.0),
                theme.status_success,
            );
            text(
                cx,
                name,
                SETTINGS_FONT_FAMILY,
                12.0,
                theme.foreground,
                500,
                Point2D::new(x + 14.0, baseline),
            );
        }
        None => text(
            cx,
            t_settings(ui, "home.mcp.noAgent"),
            SETTINGS_FONT_FAMILY,
            12.0,
            theme.muted_foreground,
            400,
            Point2D::new(x, baseline),
        ),
    }
}
