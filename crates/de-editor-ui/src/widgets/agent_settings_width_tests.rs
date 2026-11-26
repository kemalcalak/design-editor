//! The settings modal's labelled controls are hit-tested at the size they
//! are painted at, in every locale (#440): the API / ACP / CLI detail
//! panes' footer buttons and segmented controls.

use crate::theme::Theme;
use crate::widgets::agent_settings_acp_detail::{acp_layout, acp_target, paint_acp_detail};
use crate::widgets::agent_settings_api_detail::{api_layout, api_target, paint_api_detail};
use crate::widgets::agent_settings_cli_detail::{cli_layout, paint_cli_detail};
use crate::widgets::agent_settings_i18n::t as t_settings;
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
