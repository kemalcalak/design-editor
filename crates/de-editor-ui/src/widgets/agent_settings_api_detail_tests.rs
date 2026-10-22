//! Geometry and hit tests for the API provider detail pane.

use super::*;
use de_editor_core::agent_settings::BuiltinAgentPresetMenuTarget;
use de_editor_core::BUILTIN_AGENT_PRESETS;

fn detail() -> Rect {
    Rect::xywh(300.0, 100.0, 460.0, 600.0)
}

fn settings_with(models: &[&str]) -> AgentSettings {
    let mut settings = AgentSettings::default();
    settings.add_builtin_agent_config(
        "Gateway",
        "sk-test-1234567890",
        models.first().copied().unwrap_or(""),
        BuiltinAgentKind::OpenAiCompat,
        "https://gw.example/v1",
    );
    for model in models.iter().skip(1) {
        settings.builtin_agents[0].toggle_model(*model);
    }
    settings
}

#[test]
fn the_key_is_masked_but_recognisable() {
    assert_eq!(mask_key("sk-abcdefghijk3f9a"), "sk-•••••••••••3f9a");
    assert_eq!(mask_key("short"), "•••••");
}

#[test]
fn saved_models_form_a_checklist_and_rows_toggle() {
    let settings = settings_with(&["gpt-5", "qwen3-coder"]);
    let ui = EditorUiState::default();
    let target = api_target(
        &settings,
        &AgentsEntry::Builtin(settings.builtin_agents[0].id.clone()),
    )
    .unwrap();
    let layout = api_layout(detail(), &settings, &ui, target);
    assert_eq!(layout.models_box, ModelsBox::Checklist { rows: 2 });
    let row = chrome::checklist_row_rect(layout.models, 1, 0.0);
    let point = Point2D::new(row.origin.x + 20.0, row.origin.y + 10.0);
    assert_eq!(
        api_hit(&settings, &layout, target, point),
        ApiHit::ToggleModel(1)
    );
}

#[test]
fn a_provider_without_models_edits_them_as_text() {
    let mut settings = settings_with(&[]);
    settings.builtin_agents[0].models.clear();
    let ui = EditorUiState::default();
    let target = api_target(
        &settings,
        &AgentsEntry::Builtin(settings.builtin_agents[0].id.clone()),
    )
    .unwrap();
    let layout = api_layout(detail(), &settings, &ui, target);
    assert_eq!(layout.models_box, ModelsBox::Text);
    let point = Point2D::new(layout.models.origin.x + 20.0, layout.models.origin.y + 10.0);
    assert_eq!(
        api_hit(&settings, &layout, target, point),
        ApiHit::Focus(BuiltinAgentField::Model)
    );
}

#[test]
fn fields_buttons_and_eye_resolve_and_do_not_overlap() {
    let settings = settings_with(&["gpt-5"]);
    let ui = EditorUiState::default();
    let target = api_target(
        &settings,
        &AgentsEntry::Builtin(settings.builtin_agents[0].id.clone()),
    )
    .unwrap();
    let layout = api_layout(detail(), &settings, &ui, target);
    let centre = |rect: Rect| {
        Point2D::new(
            rect.origin.x + rect.size.x / 2.0,
            rect.origin.y + rect.size.y / 2.0,
        )
    };
    assert_eq!(
        api_hit(&settings, &layout, target, centre(layout.eye)),
        ApiHit::RevealKey
    );
    assert_eq!(
        api_hit(
            &settings,
            &layout,
            target,
            Point2D::new(
                layout.api_key.origin.x + 30.0,
                layout.api_key.origin.y + 10.0
            )
        ),
        ApiHit::Focus(BuiltinAgentField::ApiKey)
    );
    assert_eq!(
        api_hit(&settings, &layout, target, centre(layout.save)),
        ApiHit::Save
    );
    assert_eq!(
        api_hit(&settings, &layout, target, centre(layout.fetch)),
        ApiHit::Fetch
    );
    assert_eq!(
        api_hit(&settings, &layout, target, centre(layout.secondary)),
        ApiHit::Secondary
    );
    assert_eq!(
        api_hit(&settings, &layout, target, centre(layout.switch.unwrap())),
        ApiHit::ToggleEnabled(0)
    );
    assert!(layout.secondary.origin.x + layout.secondary.size.x <= layout.fetch.origin.x);
    assert!(layout.fetch.origin.x + layout.fetch.size.x <= layout.save.origin.x);
}

#[test]
fn the_draft_offers_every_preset_through_its_select() {
    let mut settings = AgentSettings::default();
    settings.begin_builtin_agent_draft();
    settings.builtin_preset_menu_open = Some(BuiltinAgentPresetMenuTarget::Draft);
    let ui = EditorUiState::default();
    let target = api_target(&settings, &AgentsEntry::BuiltinDraft).unwrap();
    let layout = api_layout(detail(), &settings, &ui, target);
    let menu = preset_menu_rect(&layout).unwrap();
    let first = preset_menu::preset_row_rect(menu, 0, 0.0);
    let point = Point2D::new(first.origin.x + 10.0, first.origin.y + 10.0);
    assert_eq!(
        api_hit(&settings, &layout, target, point),
        ApiHit::SelectPreset(BUILTIN_AGENT_PRESETS[0].key)
    );
}
