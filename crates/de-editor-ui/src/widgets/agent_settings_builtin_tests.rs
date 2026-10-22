use crate::widgets::agent_settings_panel::{AgentSettingsHit, AgentSettingsPanel};
use crate::{Point2D, Rect};
use de_editor_core::agent_settings::{BuiltinAgentField, SettingsFocus};
use de_editor_core::EditorState;

fn rect_center(rect: Rect) -> Point2D {
    Point2D::new(
        rect.origin.x + rect.size.x / 2.0,
        rect.origin.y + rect.size.y / 2.0,
    )
}

fn draft_state() -> EditorState {
    let mut state = EditorState::default();
    state.editor_ui.agent_settings.begin_builtin_agent_draft();
    state
        .editor_ui
        .agent_settings
        .set_builtin_agent_draft_preset(de_editor_core::BuiltinAgentPresetKey::Custom);
    state
        .editor_ui
        .agent_settings
        .builtin_agent_draft
        .as_mut()
        .expect("draft should exist")
        .api_key = "sk-test".into();
    state
}

#[test]
fn desktop_builtin_draft_geometry_remains_compact() {
    let state = draft_state();
    let settings = &state.editor_ui.agent_settings;
    let card = Rect::xywh(20.0, 40.0, 480.0, 232.0);

    let form_h =
        crate::widgets::agent_settings_builtin_layout::expanded_card_height_for_ui(settings, None);
    assert_eq!(form_h, 196.0);
    assert_eq!(
        crate::widgets::agent_settings_builtin_layout::draft_card_height_for_ui(settings),
        232.0
    );
    assert_eq!(
        crate::widgets::agent_settings_builtin_parts::provider_select_rect(card)
            .size
            .y,
        24.0
    );
    assert_eq!(
        crate::widgets::agent_settings_builtin_parts::kind_rect(card)
            .size
            .y,
        24.0
    );
    let fields: Vec<_> = (0..4)
        .map(|row| {
            crate::widgets::agent_settings_builtin_layout::field_input_rect_for_ui(
                settings, card, None, row,
            )
        })
        .collect();
    assert!(fields.iter().all(|rect| rect.size.y == 24.0));
    assert!(fields
        .windows(2)
        .all(|pair| pair[1].origin.y - pair[0].origin.y == 28.0));
    assert_eq!(
        crate::widgets::agent_settings_form_actions::save_button_rect_for_ui(card, form_h,).size,
        Point2D::new(68.0, 26.0)
    );
    assert_eq!(
        crate::widgets::agent_settings_form_actions::cancel_button_rect_for_ui(card, form_h,).size,
        Point2D::new(68.0, 26.0)
    );
}

#[test]
fn focused_model_list_expands_without_overlapping_following_fields() {
    let mut state = draft_state();
    let settings = &mut state.editor_ui.agent_settings;
    let collapsed =
        crate::widgets::agent_settings_builtin_layout::expanded_card_height_for_ui(settings, None);
    settings.focus = Some(SettingsFocus::BuiltinAgentDraft(BuiltinAgentField::Model));
    let expanded =
        crate::widgets::agent_settings_builtin_layout::expanded_card_height_for_ui(settings, None);
    let extra = 44.0;
    assert_eq!(expanded, collapsed + extra);

    let card = Rect::xywh(20.0, 40.0, 520.0, expanded + 80.0);
    let model = crate::widgets::agent_settings_builtin_layout::field_input_rect_for_ui(
        settings, card, None, 2,
    );
    let base_url = crate::widgets::agent_settings_builtin_layout::field_input_rect_for_ui(
        settings, card, None, 3,
    );
    assert_eq!(model.size.y, 68.0);
    assert!(
        base_url.origin.y >= model.origin.y + model.size.y + 4.0,
        "the Base URL row must remain below the multiline editor: {model:?} vs {base_url:?}"
    );
}

#[test]
fn pure_builtin_provider_base_url_is_read_only_hit_target() {
    let mut state = EditorState::default();
    state.editor_ui.agent_settings.add_builtin_agent();
    state.editor_ui.agent_settings.focus = Some(SettingsFocus::BuiltinAgent {
        index: 0,
        field: BuiltinAgentField::ApiKey,
    });

    let panel = AgentSettingsPanel::for_editor(&state);
    let rect = panel.rect(1200.0, 800.0);
    let content_x = crate::widgets::agent_settings_panel::content_viewport(rect)
        .origin
        .x;
    let content_y = crate::widgets::agent_settings_panel::content_viewport(rect)
        .origin
        .y;
    let first_card_y =
        content_y + crate::widgets::agent_settings_panel::AGENTS_HERO_HEIGHT + 28.0 + 28.0;
    let point = Point2D::new(content_x + 92.0, first_card_y + 170.0);

    assert_eq!(panel.hit_test(rect, point), AgentSettingsHit::Inside);
}

#[test]
fn credential_sync_error_reserves_a_status_row_in_the_layout() {
    let mut state = EditorState::default();
    state.editor_ui.agent_settings.add_builtin_agent();
    let without =
        crate::widgets::agent_settings_builtin::content_height(&state.editor_ui.agent_settings);

    state.editor_ui.agent_settings.web_credential_sync_error =
        Some("server rejected the credential snapshot (400)".into());
    let with =
        crate::widgets::agent_settings_builtin::content_height(&state.editor_ui.agent_settings);

    assert!(
        with > without,
        "sync error must reserve extra height (with={with}, without={without})"
    );
}

#[test]
fn model_field_chevron_toggles_and_catalog_rows_select() {
    let mut state = EditorState::default();
    let id = state.editor_ui.agent_settings.add_builtin_agent_config(
        "MiniMax",
        "sk-test",
        "MiniMax-M3",
        de_editor_core::BuiltinAgentKind::Anthropic,
        "https://api.minimaxi.com/anthropic",
    );
    let request = state
        .editor_ui
        .agent_settings
        .begin_builtin_model_catalog_refresh(
            de_editor_core::BuiltinModelCatalogTarget::Agent(id),
            0,
        )
        .expect("discovery request");
    let expected = state
        .editor_ui
        .agent_settings
        .builtin_model_catalog_config_for_request(&request)
        .expect("resolvable");
    state
        .editor_ui
        .agent_settings
        .apply_builtin_model_catalog_refresh_outcome_if_current(
            &expected,
            &request,
            de_editor_core::BuiltinModelCatalogRefreshOutcome::Success {
                models: vec![
                    de_editor_core::BuiltinModelOption::new("m1", "Model One"),
                    de_editor_core::BuiltinModelOption::new("m2", "Model Two"),
                ],
            },
        );
    state.editor_ui.agent_settings.focus = Some(SettingsFocus::BuiltinAgent {
        index: 0,
        field: BuiltinAgentField::Model,
    });

    let panel = AgentSettingsPanel::for_editor(&state);
    let rect = panel.rect(1200.0, 800.0);
    let content = panel.resolved_content_viewport(rect);
    let y = crate::widgets::agent_settings_panel_geometry::agents_body_top(content)
        + crate::widgets::agent_settings_builtin_layout::HEADER_HEIGHT
        + crate::widgets::agent_settings_builtin_layout::SUBTITLE_HEIGHT
        + crate::widgets::agent_settings_builtin_layout::sync_error_height(
            &state.editor_ui.agent_settings,
        );
    let card = Rect::xywh(
        content.origin.x,
        y,
        content.size.x,
        crate::widgets::agent_settings_builtin_layout::card_height_for_ui(
            &state.editor_ui.agent_settings,
            0,
        ),
    );
    let settings = &state.editor_ui.agent_settings;
    let model_input = crate::widgets::agent_settings_builtin_layout::field_input_rect_for_ui(
        settings,
        card,
        Some(0),
        2,
    );

    // The right edge of the Model input is the dropdown trigger.
    let chevron = Point2D::new(
        model_input.origin.x + model_input.size.x - 8.0,
        rect_center(model_input).y,
    );
    assert_eq!(
        crate::widgets::agent_settings_builtin::hit_test(content, settings, chevron,),
        crate::widgets::agent_settings_builtin::BuiltinHit::ToggleModelMenu(Some(0))
    );

    // The rest of the field is a plain focus target.
    assert_eq!(
        crate::widgets::agent_settings_builtin::hit_test(
            content,
            settings,
            Point2D::new(model_input.origin.x + 30.0, rect_center(model_input).y),
        ),
        crate::widgets::agent_settings_builtin::BuiltinHit::Focus {
            index: 0,
            field: BuiltinAgentField::Model,
        }
    );

    // With the menu open, a click on the second catalog row selects it.
    state.editor_ui.agent_settings.builtin_model_menu_open =
        Some(de_editor_core::agent_settings::BuiltinModelMenuTarget::Agent(0));
    let settings = &state.editor_ui.agent_settings;
    let menu =
        crate::widgets::agent_settings_builtin_model_menu::model_menu_rect(settings, card, Some(0));
    let row_1 = Point2D::new(menu.origin.x + 40.0, menu.origin.y + 4.0 + 24.0 + 12.0);
    assert_eq!(
        crate::widgets::agent_settings_builtin::hit_test(content, settings, row_1,),
        crate::widgets::agent_settings_builtin::BuiltinHit::SelectModel {
            index: Some(0),
            row: 1,
        }
    );
}
