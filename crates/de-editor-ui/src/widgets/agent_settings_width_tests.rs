//! The settings modal's labelled controls are hit-tested at the size they
//! are painted at, in every locale (#440): the API / ACP / CLI detail
//! panes' footer buttons and segmented controls.

use crate::theme::Theme;
use crate::widgets::agent_settings_acp_detail::{acp_layout, acp_target, paint_acp_detail};
use crate::widgets::agent_settings_api_detail::{api_layout, api_target, paint_api_detail};
use crate::widgets::agent_settings_cli_detail::{cli_layout, paint_cli_detail};
use crate::widgets::agent_settings_i18n::t as t_settings;
use crate::widgets::agent_settings_panel::{AgentSettingsHit, AgentSettingsPanel};
use crate::widgets::agent_settings_system::IN_APP_UPDATE_AVAILABLE;
use crate::widgets::label_fit_test_support::{assert_inside, fresh_backend, run_in};
use crate::widgets::test_family_gap_backend::FamilyGapBackend;
use crate::widgets::PaintCx;
use crate::Rect;
use de_editor_core::agent_settings::{AgentProvider, AgentSettings, AgentsEntry, BuiltinAgentKind};
use de_editor_core::editor_ui_state::EditorUiState;
use de_editor_core::BuiltinAgentPresetKey;
use de_i18n::Locale;

/// A detail column wide enough that no footer row has to squeeze.
fn detail() -> Rect {
    Rect::xywh(300.0, 100.0, 640.0, 700.0)
}

fn ui(locale: Locale) -> EditorUiState {
    EditorUiState {
        locale,
        ..EditorUiState::default()
    }
}

/// A footer button paints its label inside its box, clear of the padding
/// the box was sized with.
const BUTTON_INSET: f32 = 15.9;

fn cx(backend: &mut FamilyGapBackend) -> PaintCx<'_> {
    PaintCx { backend }
}

/// Each segment label sits inside its own equal cell.
fn assert_segments(backend: &FamilyGapBackend, rect: Rect, labels: &[&str], what: &str) {
    let cell = rect.size.x / labels.len() as f32;
    for (i, label) in labels.iter().enumerate() {
        let cell_rect = Rect::xywh(
            rect.origin.x + cell * i as f32,
            rect.origin.y,
            cell,
            rect.size.y,
        );
        assert_inside(run_in(backend, label, rect, what), cell_rect, 0.0, what);
    }
}

#[test]
fn the_api_pane_agrees_in_every_locale() {
    for locale in Locale::ALL {
        let ui = ui(locale);
        let mut settings = AgentSettings::default();
        settings.add_builtin_agent_config(
            "Gateway",
            "sk-test-1234567890",
            "gpt-5",
            BuiltinAgentKind::OpenAiCompat,
            "https://gw.example/v1",
        );
        // A preset with an alternative API format shows the format control.
        settings.builtin_agents[0].preset = BuiltinAgentPresetKey::OpenRouter;
        let entry = AgentsEntry::Builtin(settings.builtin_agents[0].id.clone());
        let target = api_target(&settings, &entry).unwrap();
        let mut backend = fresh_backend();
        let theme = Theme::dark();
        paint_api_detail(
            &mut cx(&mut backend),
            &theme,
            &settings,
            &ui,
            detail(),
            target,
            0,
        );
        let layout = api_layout(detail(), &settings, &ui, target);
        let what = format!("{locale:?} api");
        for (rect, key) in [
            (layout.save, "common.save"),
            (layout.fetch, "settings.agents.fetchModels"),
        ] {
            let label = run_in(&backend, t_settings(&ui, key), rect, &what);
            assert_inside(label, rect, BUTTON_INSET, &what);
        }
        let format = layout.format.expect("an alternative format");
        assert_segments(&backend, format, &["OpenAI-compatible", "Anthropic"], &what);
    }
}

#[test]
fn the_acp_pane_agrees_in_every_locale() {
    for locale in Locale::ALL {
        let ui = ui(locale);
        let mut settings = AgentSettings::default();
        settings.begin_acp_agent_draft();
        let target = acp_target(&settings, &AgentsEntry::AcpDraft).unwrap();
        let mut backend = fresh_backend();
        let theme = Theme::dark();
        paint_acp_detail(
            &mut cx(&mut backend),
            &theme,
            &settings,
            &ui,
            detail(),
            target,
            0,
        );
        let layout = acp_layout(detail(), &settings, &ui, target);
        let what = format!("{locale:?} acp");
        for rect in [layout.primary, layout.secondary] {
            let label = backend
                .runs
                .iter()
                .find(|run| {
                    run.origin.y > rect.origin.y
                        && run.origin.y <= rect.origin.y + rect.size.y
                        && run.origin.x >= rect.origin.x
                        && run.origin.x < rect.origin.x + rect.size.x
                })
                .unwrap_or_else(|| panic!("{what}: a button label"));
            assert!(
                !label.text.ends_with('…'),
                "{what}: {:?} was fitted",
                label.text
            );
            assert_inside(label, rect, BUTTON_INSET, &what);
        }
        let connection = [
            t_settings(&ui, "settings.agents.localCommand"),
            t_settings(&ui, "settings.agents.remoteUrl"),
        ];
        assert_segments(&backend, layout.connection, &connection, &what);
    }
}

#[test]
fn the_cli_pane_agrees_in_every_locale() {
    for locale in Locale::ALL {
        let ui = ui(locale);
        let settings = AgentSettings::default();
        let provider = AgentProvider::ClaudeCode;
        let mut backend = fresh_backend();
        let theme = Theme::dark();
        paint_cli_detail(
            &mut cx(&mut backend),
            &theme,
            &settings,
            &ui,
            detail(),
            &[],
            provider,
            0,
        );
        let layout = cli_layout(detail(), &settings, &ui, &[], provider);
        let what = format!("{locale:?} cli");
        let connect = run_in(
            &backend,
            t_settings(&ui, "common.connect"),
            layout.connect,
            &what,
        );
        assert_inside(connect, layout.connect, BUTTON_INSET, &what);
        let recheck = run_in(
            &backend,
            t_settings(&ui, "settings.agents.checkAgain"),
            layout.recheck,
            &what,
        );
        // The label sits right of the icon slot.
        assert_inside(recheck, layout.recheck, BUTTON_INSET, &what);
    }
}

/// A footer button is exactly as wide as its painted label plus padding.
#[test]
fn a_button_follows_its_painted_label() {
    let ui = ui(Locale::De);
    let settings = AgentSettings::default();
    let provider = AgentProvider::ClaudeCode;
    let mut backend = fresh_backend();
    paint_cli_detail(
        &mut cx(&mut backend),
        &Theme::dark(),
        &settings,
        &ui,
        detail(),
        &[],
        provider,
        0,
    );
    let wide = cli_layout(detail(), &settings, &ui, &[], provider)
        .connect
        .size
        .x;
    let label = crate::widgets::text_metrics::measure_in_family_weighted(
        &mut backend,
        t_settings(&ui, "common.connect"),
        13.0,
        crate::widgets::home_surface::SANS,
        500,
    );
    assert_eq!(wide, label + 32.0);
    crate::widgets::label_widths::reset();
    let mut uniform = FamilyGapBackend::uniform();
    paint_cli_detail(
        &mut cx(&mut uniform),
        &Theme::dark(),
        &settings,
        &ui,
        detail(),
        &[],
        provider,
        0,
    );
    assert!(
        cli_layout(detail(), &settings, &ui, &[], provider)
            .connect
            .size
            .x
            < wide
    );
}

/// The panel painted into the family-aware backend, and its rect.
fn painted_panel(state: &de_editor_core::EditorState) -> (Rect, FamilyGapBackend) {
    use crate::widgets::Widget;
    let panel = AgentSettingsPanel::for_editor(state);
    let rect = panel.rect(1200.0, 800.0);
    let mut backend = fresh_backend();
    panel.paint(&mut cx(&mut backend), rect);
    (rect, backend)
}

/// The System row's Update / Restart / Check now button: in every locale
/// its label is painted whole, and both ends of the painted run are inside
/// the box the backend-less hit-test resolves for the button.
#[test]
fn the_system_row_button_agrees_in_every_locale() {
    use de_editor_core::editor_ui_state::{UpdateModalState, UpdatePhase};
    use de_editor_core::{AgentSettingsTab, EditorState, UpdateStatus};
    for locale in Locale::ALL {
        let cases = [
            (
                "settings.system.checkButton",
                AgentSettingsHit::CheckUpdatesButton,
            ),
            (
                "settings.system.updateButton",
                AgentSettingsHit::UpdateButton,
            ),
            (
                "settings.system.restartButton",
                AgentSettingsHit::UpdateButton,
            ),
        ];
        for (key, expected) in cases {
            let mut state = EditorState::default();
            state.editor_ui.locale = locale;
            state.editor_ui.agent_settings.tab = AgentSettingsTab::System;
            match key {
                "settings.system.updateButton" => {
                    state.editor_ui.update_status = UpdateStatus::Available {
                        version: "9.8.7".to_string(),
                    };
                }
                "settings.system.restartButton" => {
                    let mut modal = UpdateModalState::new("9.8.7", UpdatePhase::ReadyToRestart);
                    modal.visible = false;
                    state.editor_ui.update_modal = Some(modal);
                }
                _ => {}
            }
            let (rect, backend) = painted_panel(&state);
            let label = t_settings(&state.editor_ui, key);
            let what = format!("{locale:?} {key}");
            // Update / Restart exist only where the app can update in place
            // (macOS arm64, Windows x64); elsewhere the row has no button.
            if expected == AgentSettingsHit::UpdateButton && !IN_APP_UPDATE_AVAILABLE {
                assert!(
                    backend.runs.iter().all(|run| run.text != label),
                    "{what}: no Update / Restart button on this platform"
                );
                continue;
            }
            let run = backend
                .runs
                .iter()
                .find(|run| run.text == label)
                .unwrap_or_else(|| panic!("{what}: {label:?} was not painted whole"));
            let panel = AgentSettingsPanel::for_editor(&state);
            let y = run.origin.y - 4.0;
            for x in [run.origin.x + 0.5, run.right_edge() - 0.5] {
                let hit = panel.hit_test(rect, crate::Point2D::new(x, y));
                assert_eq!(hit, expected, "{what} at x={x}");
            }
        }
    }
}

/// The MCP tab's Copy chip: in every locale its label is painted whole
/// inside the chip rect the backend-less hit-test resolves.
#[test]
fn the_mcp_copy_chip_agrees_in_every_locale() {
    use crate::widgets::agent_settings_mcp::client_config_copy_button_rect;
    use crate::widgets::agent_settings_panel::content_viewport;
    use de_editor_core::{AgentSettingsTab, EditorState};
    for locale in Locale::ALL {
        let mut state = EditorState::default();
        state.editor_ui.locale = locale;
        state.editor_ui.agent_settings.tab = AgentSettingsTab::Mcp;
        state.editor_ui.agent_settings.mcp_server.running = true;
        let (rect, backend) = painted_panel(&state);
        let chip = client_config_copy_button_rect(
            content_viewport(rect),
            &state.editor_ui.agent_settings,
            locale,
        );
        let what = format!("{locale:?} mcp copy");
        let label = run_in(
            &backend,
            de_i18n::translate(locale, "home.mcp.copy"),
            chip,
            &what,
        );
        // Pad, icon and gap on the left; the pad on the right.
        assert_inside(label, chip, 8.0, &what);
        let panel = AgentSettingsPanel::for_editor(&state);
        let edge = crate::Point2D::new(chip.origin.x + chip.size.x - 0.5, chip.origin.y + 1.0);
        assert_eq!(
            panel.hit_test(rect, edge),
            AgentSettingsHit::CopyMcpClientConfig,
            "{what}"
        );
    }
}
