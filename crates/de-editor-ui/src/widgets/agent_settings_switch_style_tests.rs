use crate::widgets::agent_settings_panel::AgentSettingsPanel;
use crate::widgets::{PaintCx, Widget};
use crate::{Color, Point2D, Rect, RenderBackend, TextLayout};
use de_editor_core::agent_settings::AgentSettingsTab;
use de_editor_core::editor_ui_state::ThemeMode;
use de_editor_core::{AgentSettingsButton, ButtonPressTarget, EditorState};

mod images;

#[derive(Default)]
struct CaptureBackend {
    round_fills: Vec<(Rect, Color)>,
    ovals: Vec<(Rect, Color)>,
    round_strokes: Vec<(Rect, Color, f32)>,
    clips: Vec<Rect>,
    lines: Vec<(Point2D, Point2D, Color, f32)>,
    text_points: Vec<Point2D>,
}

impl RenderBackend for CaptureBackend {
    fn begin_frame(&mut self) {}
    fn end_frame(&mut self) {}
    fn fill_rect(&mut self, _: Rect, _: Color) {}
    fn stroke_rect(&mut self, _: Rect, _: Color, _: f32) {}
    fn draw_text(&mut self, _: &TextLayout, point: Point2D) {
        self.text_points.push(point);
    }
    fn clip_rect(&mut self, rect: Rect) {
        self.clips.push(rect);
    }
    fn stroke_line(&mut self, from: Point2D, to: Point2D, color: Color, width: f32) {
        self.lines.push((from, to, color, width));
    }
    fn fill_round_rect(&mut self, rect: Rect, _: f32, color: Color) {
        self.round_fills.push((rect, color));
    }
    fn stroke_round_rect(&mut self, rect: Rect, _: f32, color: Color, width: f32) {
        self.round_strokes.push((rect, color, width));
    }
    fn stroke_svg_path(&mut self, _: &str, _: Point2D, _: f32, _: Color, _: f32) {}
    fn fill_oval(&mut self, rect: Rect, color: Color) {
        self.ovals.push((rect, color));
    }
    fn save(&mut self) {}
    fn restore(&mut self) {}
    fn translate(&mut self, _: Point2D) {}
    fn resize(&mut self, _: u32, _: u32) {}
    fn dpi_scale(&self) -> f32 {
        1.0
    }
}

fn color_eq(a: Color, b: Color) -> bool {
    (a.r - b.r).abs() < 1e-6
        && (a.g - b.g).abs() < 1e-6
        && (a.b - b.b).abs() < 1e-6
        && (a.a - b.a).abs() < 1e-6
}

fn rect_eq(a: Rect, b: Rect) -> bool {
    (a.origin.x - b.origin.x).abs() < 0.01
        && (a.origin.y - b.origin.y).abs() < 0.01
        && (a.size.x - b.size.x).abs() < 0.01
        && (a.size.y - b.size.y).abs() < 0.01
}

/// Right-aligned switch track on a settings row.
fn row_switch_track(row: Rect) -> Rect {
    super::agent_settings_rows::row_control_rect(
        row,
        super::agent_settings_switch::SETTINGS_SWITCH_W,
        super::agent_settings_switch::SETTINGS_SWITCH_H,
    )
}

/// System-tab row `index`, below that tab's compact heading. Its rows
/// differ in height (two of them carry a second line), so this walks the
/// mixed ladder rather than assuming a uniform stride.
fn system_row_rect(rect: Rect, index: usize) -> Rect {
    use super::agent_settings_rows::RowLines;
    const SYSTEM_ROWS: [RowLines; 4] = [RowLines::One, RowLines::Two, RowLines::Two, RowLines::One];
    let content = crate::widgets::agent_settings_panel::content_viewport(rect);
    super::agent_settings_rows::row_rect_in(
        super::agent_settings_rows::card_rows_content(content),
        content.origin.y + super::agent_settings_rows::tab_intro_height(true),
        &SYSTEM_ROWS,
        index,
    )
}

fn ts_switch_knob_rect(track: Rect, enabled: bool) -> Rect {
    let inset = 2.0;
    let knob = 16.0;
    let x = if enabled {
        track.origin.x + track.size.x - knob - inset
    } else {
        track.origin.x + inset
    };
    Rect {
        origin: Point2D::new(x, track.origin.y + inset),
        size: Point2D::new(knob, knob),
    }
}

fn mcp_client_config_copy_rect(rect: Rect) -> Rect {
    super::agent_settings_mcp::client_config_copy_button_rect(
        crate::widgets::agent_settings_panel::content_viewport(rect),
        &de_editor_core::agent_settings::AgentSettings::default(),
        EditorState::default().editor_ui.effective_locale(),
    )
}

fn mcp_server_button_rect(rect: Rect) -> Rect {
    super::agent_settings_mcp::server_button_rect(
        crate::widgets::agent_settings_panel::content_viewport(rect),
    )
}

#[test]
fn hovered_mcp_server_button_paints_hover_wash() {
    let mut state = EditorState::default();
    state.editor_ui.theme_mode = ThemeMode::Light;
    state.editor_ui.agent_settings.tab = AgentSettingsTab::Mcp;
    state.editor_ui.agent_settings.mcp_server.running = true;
    state.editor_ui.agent_settings.hover_mcp_server_button = true;
    let panel = AgentSettingsPanel::for_editor(&state);
    let rect = panel.rect(1200.0, 800.0);
    let button = mcp_server_button_rect(rect);
    let mut backend = CaptureBackend::default();
    let mut cx = PaintCx {
        backend: &mut backend,
    };

    panel.paint(&mut cx, rect);

    assert!(
        backend
            .round_fills
            .iter()
            .any(|(r, color)| rect_eq(*r, button) && color_eq(*color, panel.theme.button_hover)),
        "hovering the MCP server start/stop button should paint the same subtle wash as other buttons"
    );
}

#[test]
fn pressed_mcp_server_button_uses_shared_button_feedback() {
    let mut state = EditorState::default();
    state.editor_ui.theme_mode = ThemeMode::Light;
    state.editor_ui.agent_settings.tab = AgentSettingsTab::Mcp;
    state.editor_ui.agent_settings.mcp_server.running = true;
    state.editor_ui.pressed_button = Some(ButtonPressTarget::AgentSettings(
        AgentSettingsButton::McpServer,
    ));
    let panel = AgentSettingsPanel::for_editor(&state);
    let rect = panel.rect(1200.0, 800.0);
    let button = mcp_server_button_rect(rect);
    let expected = panel
        .theme
        .button_hover
        .with_alpha(panel.theme.button_hover.a * 1.8);
    let mut backend = CaptureBackend::default();
    let mut cx = PaintCx {
        backend: &mut backend,
    };

    panel.paint(&mut cx, rect);

    assert!(
        backend
            .round_fills
            .iter()
            .any(|(r, color)| rect_eq(*r, button) && color_eq(*color, expected)),
        "pressed MCP server button should paint the shared pressed feedback token"
    );
}

#[test]
fn system_auto_update_switch_uses_light_thumb() {
    let mut state = EditorState::default();
    state.editor_ui.theme_mode = ThemeMode::Light;
    state.editor_ui.agent_settings.tab = AgentSettingsTab::System;
    state.editor_ui.agent_settings.auto_update_enabled = true;
    let panel = AgentSettingsPanel::for_editor(&state);
    let rect = panel.rect(1200.0, 800.0);
    // Row order is Appearance, Auto-update, Experimental, Pencil cursor.
    let track = row_switch_track(system_row_rect(rect, 1));
    let knob = ts_switch_knob_rect(track, true);
    let mut backend = CaptureBackend::default();
    let mut cx = PaintCx {
        backend: &mut backend,
    };

    panel.paint(&mut cx, rect);

    assert!(
        backend
            .ovals
            .iter()
            .any(|(r, color)| rect_eq(*r, knob) && color_eq(*color, panel.theme.primary_foreground)),
        "auto-update switch should use a light switch thumb"
    );
    assert!(
        !backend
            .ovals
            .iter()
            .any(|(r, color)| rect_eq(*r, knob) && color_eq(*color, panel.theme.foreground)),
        "auto-update switch thumb should not be black in light theme"
    );
}

#[test]
fn system_auto_update_switch_matches_ts_unchecked_geometry_and_track() {
    let mut state = EditorState::default();
    state.editor_ui.theme_mode = ThemeMode::Light;
    state.editor_ui.agent_settings.tab = AgentSettingsTab::System;
    state.editor_ui.agent_settings.auto_update_enabled = false;
    let panel = AgentSettingsPanel::for_editor(&state);
    let rect = panel.rect(1200.0, 800.0);
    // Row order is Appearance, Auto-update, Experimental, Pencil cursor.
    let track = row_switch_track(system_row_rect(rect, 1));
    let knob = ts_switch_knob_rect(track, false);
    let mut backend = CaptureBackend::default();
    let mut cx = PaintCx {
        backend: &mut backend,
    };

    panel.paint(&mut cx, rect);

    assert!(
        backend
            .round_fills
            .iter()
            .any(|(r, color)| rect_eq(*r, track) && color_eq(*color, panel.theme.input)),
        "unchecked settings switch off-track should use the shadcn `input` token \
         (distinct from `border` now that the light theme gives input its own value)"
    );
    assert!(
        backend
            .ovals
            .iter()
            .any(|(r, color)| rect_eq(*r, knob) && color_eq(*color, panel.theme.primary_foreground)),
        "unchecked settings switch thumb should be 16px with 2px inset like the TS Switch"
    );
}

#[test]
fn hovered_mcp_client_config_copy_button_paints_hover_wash() {
    let mut state = EditorState::default();
    state.editor_ui.theme_mode = ThemeMode::Light;
    state.editor_ui.agent_settings.tab = AgentSettingsTab::Mcp;
    state.editor_ui.agent_settings.mcp_server.running = true;
    state.editor_ui.agent_settings.hover_mcp_client_config_copy = true;
    let panel = AgentSettingsPanel::for_editor(&state);
    let rect = panel.rect(1200.0, 800.0);
    let copy = mcp_client_config_copy_rect(rect);
    let mut backend = CaptureBackend::default();
    let mut cx = PaintCx {
        backend: &mut backend,
    };

    panel.paint(&mut cx, rect);

    assert!(
        backend
            .round_fills
            .iter()
            .any(|(r, color)| rect_eq(*r, copy) && color_eq(*color, panel.theme.button_hover)),
        "hovering the MCP client config copy button should paint the same subtle wash as other icon buttons"
    );
}

#[test]
fn pressed_mcp_client_config_copy_button_uses_shared_button_feedback() {
    let mut state = EditorState::default();
    state.editor_ui.theme_mode = ThemeMode::Light;
    state.editor_ui.agent_settings.tab = AgentSettingsTab::Mcp;
    state.editor_ui.agent_settings.mcp_server.running = true;
    state.editor_ui.pressed_button = Some(ButtonPressTarget::AgentSettings(
        AgentSettingsButton::McpClientConfigCopy,
    ));
    let panel = AgentSettingsPanel::for_editor(&state);
    let rect = panel.rect(1200.0, 800.0);
    let copy = mcp_client_config_copy_rect(rect);
    let expected = panel
        .theme
        .button_hover
        .with_alpha(panel.theme.button_hover.a * 1.8);
    let mut backend = CaptureBackend::default();
    let mut cx = PaintCx {
        backend: &mut backend,
    };

    panel.paint(&mut cx, rect);

    assert!(
        backend
            .round_fills
            .iter()
            .any(|(r, color)| rect_eq(*r, copy) && color_eq(*color, expected)),
        "pressed MCP client config copy button should paint the shared pressed feedback token"
    );
}
