//! CLI agent connect-lifecycle rendering on the Agents tab (#26): the
//! list row's status line and the selected agent's status card follow
//! the probe, not the persisted `connected` flag.

use crate::widgets::agent_settings_panel::AgentSettingsPanel;
use crate::widgets::{PaintCx, Widget};
use crate::{Color, Point2D, Rect, RenderBackend, TextLayout};
use de_editor_core::agent_settings::{AgentsEntry, ProviderConnectPhase};
use de_editor_core::{AgentProvider, EditorState};

/// Capture backend recording every text run's content + color so
/// status-line strings and their TS palette colors are assertable.
#[derive(Default)]
struct TextCapture {
    texts: Vec<(String, jian_core::scene::Color)>,
}

impl RenderBackend for TextCapture {
    fn begin_frame(&mut self) {}
    fn end_frame(&mut self) {}
    fn fill_rect(&mut self, _: Rect, _: Color) {}
    fn stroke_rect(&mut self, _: Rect, _: Color, _: f32) {}
    fn draw_text(&mut self, layout: &TextLayout, _: Point2D) {
        for run in layout.runs() {
            self.texts.push((run.content.clone(), run.color));
        }
    }
    fn clip_rect(&mut self, _: Rect) {}
    fn stroke_line(&mut self, _: Point2D, _: Point2D, _: Color, _: f32) {}
    fn fill_round_rect(&mut self, _: Rect, _: f32, _: Color) {}
    fn stroke_round_rect(&mut self, _: Rect, _: f32, _: Color, _: f32) {}
    fn stroke_svg_path(&mut self, _: &str, _: Point2D, _: f32, _: Color, _: f32) {}
    fn save(&mut self) {}
    fn restore(&mut self) {}
    fn translate(&mut self, _: Point2D) {}
    fn resize(&mut self, _: u32, _: u32) {}
    fn dpi_scale(&self) -> f32 {
        1.0
    }
}

fn paint_panel(state: &mut EditorState) -> TextCapture {
    // The default locale is ZhCn — pin EN so the status-line
    // assertions match the English table.
    state.editor_ui.locale = de_i18n::Locale::EnUs;
    let panel = AgentSettingsPanel::for_editor(state);
    let rect = panel.rect(1200.0, 800.0);
    let mut backend = TextCapture::default();
    let mut cx = PaintCx {
        backend: &mut backend,
    };
    panel.paint(&mut cx, rect);
    backend
}

fn has_text(capture: &TextCapture, needle: &str) -> bool {
    capture.texts.iter().any(|(text, _)| text.contains(needle))
}

/// Codex selected, with its probe in `phase`.
fn codex_state(phase: ProviderConnectPhase) -> EditorState {
    let mut state = EditorState::default();
    let settings = &mut state.editor_ui.agent_settings;
    settings.select_agents_entry(AgentsEntry::Cli(AgentProvider::CodexCli));
    settings.provider_connection[AgentProvider::CodexCli.index()].phase = phase;
    state
}

#[test]
fn a_probing_agent_reads_as_checking() {
    let mut state = codex_state(ProviderConnectPhase::Probing);
    let capture = paint_panel(&mut state);
    assert!(has_text(&capture, "CLI · Checking status…"), "list row");
    assert!(has_text(&capture, "Checking status…"), "status card");
}

#[test]
fn a_connected_agent_shows_its_probe_info_and_version() {
    let mut state = codex_state(ProviderConnectPhase::Connected);
    let index = AgentProvider::CodexCli.index();
    state.editor_ui.agent_settings.connected[index] = true;
    let connection = &mut state.editor_ui.agent_settings.provider_connection[index];
    connection.info = Some("Signed in as a@b.c".into());
    connection.version = Some("v2.1.4".into());
    let capture = paint_panel(&mut state);
    assert!(has_text(&capture, "Connected · v2.1.4"));
    assert!(has_text(&capture, "Signed in as a@b.c"));
    assert!(has_text(&capture, "Disconnect"));
}

#[test]
fn a_persisted_flag_without_a_probe_is_not_connected() {
    let mut state = codex_state(ProviderConnectPhase::Idle);
    state.editor_ui.agent_settings.connected[AgentProvider::CodexCli.index()] = true;
    let capture = paint_panel(&mut state);
    assert!(has_text(&capture, "CLI · Not connected"));
    assert!(!has_text(&capture, "Disconnect"));
}

#[test]
fn a_failed_probe_paints_its_error_and_a_missing_cli_its_install_line() {
    let mut state = codex_state(ProviderConnectPhase::Error);
    state.editor_ui.agent_settings.provider_connection[AgentProvider::CodexCli.index()].error =
        Some("auth expired".into());
    assert!(has_text(&paint_panel(&mut state), "auth expired"));

    let mut state = codex_state(ProviderConnectPhase::Error);
    let connection =
        &mut state.editor_ui.agent_settings.provider_connection[AgentProvider::CodexCli.index()];
    connection.not_installed = true;
    connection.install_command = Some("npm i -g @openai/codex".into());
    let capture = paint_panel(&mut state);
    assert!(has_text(&capture, "Not installed"));
    assert!(has_text(&capture, "npm i -g @openai/codex"));
}
