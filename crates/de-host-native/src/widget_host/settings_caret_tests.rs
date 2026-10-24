use super::WidgetHostNative;
use de_editor_core::agent_settings::{BuiltinAgentField, SettingsFocus};
use de_editor_core::editor_ui_state::VariableRowFocus;

#[test]
fn settings_input_uses_text_input_state_for_editing() {
    let mut host = WidgetHostNative::new();
    {
        let ui = &mut host.editor_state_mut().editor_ui;
        ui.agent_settings.focus =
            Some(SettingsFocus::BuiltinAgentDraft(BuiltinAgentField::BaseUrl));
        ui.settings_input.set_text("abcd");
    }

    assert!(host.apply_settings_caret(false));
    assert!(host.apply_settings_caret(false));
    assert_eq!(host.editor_state().editor_ui.settings_input.caret(), 2);

    assert!(host.apply_text('X'));
    assert_eq!(host.editor_state().editor_ui.settings_input.text(), "abXcd");
    assert_eq!(host.editor_state().editor_ui.settings_input.caret(), 3);

    assert!(host.apply_backspace());
    assert_eq!(host.editor_state().editor_ui.settings_input.text(), "abcd");
    assert_eq!(host.editor_state().editor_ui.settings_input.caret(), 2);

    assert!(host.apply_select_all());
    assert!(host.apply_text('Z'));
    assert_eq!(host.editor_state().editor_ui.settings_input.text(), "Z");
    assert_eq!(host.editor_state().editor_ui.settings_input.caret(), 1);
}

#[test]
fn enter_adds_a_model_line_instead_of_committing_settings() {
    let mut host = WidgetHostNative::new();
    {
        let ui = &mut host.editor_state_mut().editor_ui;
        ui.agent_settings.focus = Some(SettingsFocus::BuiltinAgentDraft(BuiltinAgentField::Model));
        ui.settings_input.set_text("model-a");
    }

    assert!(host.apply_send());
    assert_eq!(
        host.editor_state().editor_ui.settings_input.text(),
        "model-a\n"
    );
    assert_eq!(
        host.editor_state().editor_ui.agent_settings.focus,
        Some(SettingsFocus::BuiltinAgentDraft(BuiltinAgentField::Model))
    );
}

#[test]
fn model_clipboard_paste_preserves_lines_and_normalizes_newlines() {
    let mut host = WidgetHostNative::new();
    {
        let ui = &mut host.editor_state_mut().editor_ui;
        ui.agent_settings.focus = Some(SettingsFocus::BuiltinAgentDraft(BuiltinAgentField::Model));
        ui.settings_input.set_text("old-model");
        ui.settings_input.select_all();
    }

    assert!(host.apply_input_paste("model-a\r\nmodel-b\rmodel-c\nmodel-d",));
    assert_eq!(
        host.editor_state().editor_ui.settings_input.text(),
        "model-a\nmodel-b\nmodel-c\nmodel-d"
    );
}

#[test]
fn model_ime_commit_preserves_lines_for_mobile_editor_shells() {
    let mut host = WidgetHostNative::new();
    {
        let ui = &mut host.editor_state_mut().editor_ui;
        ui.agent_settings.focus = Some(SettingsFocus::BuiltinAgentDraft(BuiltinAgentField::Model));
        ui.settings_input.set_text("");
    }

    assert!(host.apply_ime_commit("模型甲\r\n模型乙\r模型丙"));
    assert_eq!(
        host.editor_state().editor_ui.settings_input.text(),
        "模型甲\n模型乙\n模型丙"
    );
}

#[test]
fn next_animation_deadline_uses_focused_variable_row_input_anchor() {
    let _guard = crate::agent_indicator_test_support::write();
    de_editor_core::agent_indicators::clear();
    let mut host = WidgetHostNative::new();
    host.set_now_ms(1_300);
    {
        let state = host.editor_state_mut();
        state.editor_ui.variable_row_focus = Some(VariableRowFocus::String(0));
        state.editor_ui.variable_row_input.touch(1_250);
        state.ui.property_caret_anchor_ms = 0;
    }

    let expected = host
        .editor_state()
        .editor_ui
        .variable_row_input
        .next_blink_flip_ms(1_300);

    assert_eq!(host.next_animation_deadline_ms(), Some(expected));
}

#[test]
fn next_animation_deadline_ticks_slowly_for_unseen_agent_reveals() {
    let _guard = crate::agent_indicator_test_support::write();
    let epoch = de_editor_core::agent_indicators::begin();
    de_editor_core::agent_indicators::add_reveal(epoch, "n1", 1_400);
    let mut host = WidgetHostNative::new();
    host.set_now_ms(1_000);

    // Nothing painted yet, so the reveal is not known to be on screen: the
    // host waits for the slow overlay tick instead of the reveal's start
    // (#17; the on-screen cadence is covered in canvas_generation_visibility).
    assert_eq!(
        host.next_animation_deadline_ms(),
        Some(1_000 + de_editor_ui::widgets::canvas_generation_visibility::OFFSCREEN_TICK_MS)
    );
    de_editor_core::agent_indicators::end_if_epoch(epoch);
}

#[test]
fn next_animation_deadline_ticks_slowly_for_an_unseen_generating_frame() {
    let _guard = crate::agent_indicator_test_support::write();
    let epoch = de_editor_core::agent_indicators::begin();
    de_editor_core::agent_indicators::add_frame(epoch, "frame", "#4ECDC4", "Mochi");
    let mut host = WidgetHostNative::new();
    host.set_now_ms(1_000);

    // Off screen (nothing painted yet) the scan ticks slowly (#17).
    assert_eq!(
        host.next_animation_deadline_ms(),
        Some(1_000 + de_editor_ui::widgets::canvas_generation_visibility::OFFSCREEN_TICK_MS)
    );
    de_editor_core::agent_indicators::end_if_epoch(epoch);
}
