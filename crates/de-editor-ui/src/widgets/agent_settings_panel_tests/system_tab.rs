//! System tab: the auto-update row and its click target.
//!
//! Split out of `agent_settings_panel_tests.rs` to keep that file under the
//! 800-line cap.

use super::*;
use crate::widgets::agent_settings_metrics::CONTENT_TAIL_PAD;
use crate::widgets::agent_settings_rows::{row_height, tab_intro_height, RowLines, FOOTNOTE_H};

/// Row order and shape on the System tab: Appearance (one line),
/// Auto-update (two), Experimental (two), Pencil cursor (one), Pencil
/// smoothing (one).
const SYSTEM_ROWS: [RowLines; 7] = [
    RowLines::One,
    RowLines::Two,
    RowLines::Two,
    RowLines::One,
    RowLines::One,
    RowLines::Two,
    // Ask before destructive agent actions (#398).
    RowLines::Two,
];

/// Centre-y of System row `index`, below the tab's compact heading.
fn system_row_mid_y(rect: Rect, index: usize) -> f32 {
    let content = crate::widgets::agent_settings_panel::content_viewport(rect);
    let top: f32 = content.origin.y
        + tab_intro_height(true)
        + SYSTEM_ROWS[..index]
            .iter()
            .copied()
            .map(row_height)
            .sum::<f32>();
    top + row_height(SYSTEM_ROWS[index]) / 2.0
}

#[test]
fn system_auto_update_switch_has_click_target() {
    let mut state = EditorState::default();
    state.editor_ui.agent_settings.tab = AgentSettingsTab::System;
    let panel = AgentSettingsPanel::for_editor(&state);
    let rect = panel.rect(1200.0, 800.0);
    let content = crate::widgets::agent_settings_panel::content_viewport(rect);
    // Row order is Appearance, Auto-update, Experimental, Pencil cursor,
    // Pencil smoothing.
    let point = crate::Point2D::new(
        content.origin.x + content.size.x - 18.0,
        system_row_mid_y(rect, 1),
    );

    assert_eq!(
        panel.hit_test(rect, point),
        AgentSettingsHit::ToggleAutoUpdate
    );
}

#[test]
fn confirm_agent_actions_switch_has_click_target() {
    let mut state = EditorState::default();
    state.editor_ui.agent_settings.tab = AgentSettingsTab::System;
    let panel = AgentSettingsPanel::for_editor(&state);
    let rect = panel.rect(1200.0, 800.0);
    let content = crate::widgets::agent_settings_panel::content_viewport(rect);
    let point = crate::Point2D::new(
        content.origin.x + content.size.x - 18.0,
        system_row_mid_y(rect, 6),
    );
    assert_eq!(
        panel.hit_test(rect, point),
        AgentSettingsHit::ToggleConfirmAgentActions
    );
    // The press flips the setting.
    let before = state
        .editor_ui
        .agent_settings
        .confirm_destructive_agent_actions;
    assert!(!before, "off by default");
}

#[test]
fn system_tab_reserves_space_for_update_status() {
    let mut state = EditorState::default();
    state.editor_ui.agent_settings.tab = AgentSettingsTab::System;
    let panel = AgentSettingsPanel::for_editor(&state);

    assert_eq!(
        panel.content_total_height(),
        tab_intro_height(true)
            + SYSTEM_ROWS.iter().copied().map(row_height).sum::<f32>()
            // The gap between the rows' card and the version footnote.
            + 8.0
            + FOOTNOTE_H
            + CONTENT_TAIL_PAD,
        "System tab = heading + Appearance/Auto-update/Experimental/Pencil-cursor/\
         Pencil-smoothing/Live-pages/Confirm-agent-actions rows + version footnote + bottom pad"
    );
}

#[test]
fn system_pencil_smoothing_segments_pick_their_level() {
    use de_editor_core::PencilSmoothing;
    let mut state = EditorState::default();
    state.editor_ui.agent_settings.tab = AgentSettingsTab::System;
    let panel = AgentSettingsPanel::for_editor(&state);
    let rect = panel.rect(1200.0, 800.0);
    let content = crate::widgets::agent_settings_panel::content_viewport(rect);
    let y = system_row_mid_y(rect, 4);
    let seg = crate::widgets::agent_settings_system::SMOOTHING_SEGMENT_W;
    // The control is right-aligned: Smooth is the last segment.
    let right = content.origin.x + content.size.x;
    assert_eq!(
        panel.hit_test(rect, crate::Point2D::new(right - 18.0, y)),
        AgentSettingsHit::SelectPencilSmoothing(PencilSmoothing::Smooth)
    );
    assert_eq!(
        panel.hit_test(rect, crate::Point2D::new(right - 18.0 - seg, y)),
        AgentSettingsHit::SelectPencilSmoothing(PencilSmoothing::Balanced)
    );
    assert_eq!(
        panel.hit_test(rect, crate::Point2D::new(right - 18.0 - 2.0 * seg, y)),
        AgentSettingsHit::SelectPencilSmoothing(PencilSmoothing::Precise)
    );
}

#[test]
fn system_live_pages_switch_toggles_streaming() {
    let mut state = EditorState::default();
    state.editor_ui.agent_settings.tab = AgentSettingsTab::System;
    let panel = AgentSettingsPanel::for_editor(&state);
    let rect = panel.rect(1200.0, 800.0);
    let content = crate::widgets::agent_settings_panel::content_viewport(rect);
    let y = system_row_mid_y(rect, 5);
    let right = content.origin.x + content.size.x;
    let hit = panel.hit_test(rect, crate::Point2D::new(right - 18.0, y));
    assert_eq!(hit, AgentSettingsHit::ToggleLivePages);
    assert!(state.editor_ui.browser_live.live_pages(), "on by default");
    crate::widgets::agent_settings_press_flow::apply_agent_settings_hit(
        &mut state,
        hit,
        de_editor_core::host_settings_commit::SettingsCommitScope::Operator,
        0,
    );
    assert!(!state.editor_ui.browser_live.live_pages());
}

#[test]
fn system_tab_paints_each_update_probe_result() {
    let cases = [
        (de_editor_core::UpdateStatus::Idle, "Not checked yet"),
        (de_editor_core::UpdateStatus::Checking, "Checking…"),
        (de_editor_core::UpdateStatus::UpToDate, "Up to date"),
        (
            de_editor_core::UpdateStatus::Available {
                version: "9.8.7".to_string(),
            },
            "Update available v9.8.7",
        ),
        (de_editor_core::UpdateStatus::Error, "Check failed"),
    ];

    for (status, expected) in cases {
        let mut state = EditorState::default();
        state.editor_ui.locale = de_editor_core::Locale::EnUs;
        state.editor_ui.agent_settings.tab = AgentSettingsTab::System;
        state.editor_ui.update_status = status;
        let panel = AgentSettingsPanel::for_editor(&state);
        let rect = panel.rect(1200.0, 800.0);
        let mut backend = CaptureBackend::default();
        let mut cx = PaintCx {
            backend: &mut backend,
        };

        panel.paint(&mut cx, rect);

        assert!(
            backend
                .text_effective_points
                .iter()
                .any(|(text, _)| text == expected),
            "System tab should paint update status {expected:?}"
        );
    }
}

/// Where the System tab painted `label`, if it did.
fn painted_at(state: &EditorState, label: &str) -> Option<(Rect, crate::Point2D)> {
    let panel = AgentSettingsPanel::for_editor(state);
    let rect = panel.rect(1200.0, 800.0);
    let mut backend = CaptureBackend::default();
    let mut cx = PaintCx {
        backend: &mut backend,
    };
    panel.paint(&mut cx, rect);
    backend
        .text_effective_points
        .iter()
        .find(|(text, _)| text == label)
        .map(|(_, point)| (rect, *point))
}

#[test]
fn system_check_now_runs_the_release_probe_while_no_update_is_known() {
    for status in [
        de_editor_core::UpdateStatus::Idle,
        de_editor_core::UpdateStatus::UpToDate,
        de_editor_core::UpdateStatus::Error,
    ] {
        let mut state = EditorState::default();
        state.editor_ui.locale = de_editor_core::Locale::EnUs;
        state.editor_ui.agent_settings.tab = AgentSettingsTab::System;
        state.editor_ui.update_status = status.clone();
        let (rect, text) = painted_at(&state, "Check now")
            .unwrap_or_else(|| panic!("Check now is offered for {status:?}"));
        // The label's baseline sits inside the button; aim just above it.
        let point = crate::Point2D::new(text.x + 4.0, text.y - 4.0);
        let hit = AgentSettingsPanel::for_editor(&state).hit_test(rect, point);
        assert_eq!(hit, AgentSettingsHit::CheckUpdatesButton);

        crate::widgets::agent_settings_press_flow::apply_agent_settings_hit(
            &mut state,
            hit,
            de_editor_core::host_settings_commit::SettingsCommitScope::Operator,
            0,
        );
        assert_eq!(
            state.editor_ui.update_status,
            de_editor_core::UpdateStatus::Checking
        );
        assert_eq!(
            state.editor_ui.pending_update_action,
            Some(de_editor_core::UpdateAction::Check)
        );
        assert!(
            painted_at(&state, "Check now").is_none(),
            "hidden while the check runs"
        );
    }
}

#[test]
fn system_check_now_is_hidden_once_an_update_is_known() {
    let mut state = EditorState::default();
    state.editor_ui.locale = de_editor_core::Locale::EnUs;
    state.editor_ui.agent_settings.tab = AgentSettingsTab::System;
    state.editor_ui.update_status = de_editor_core::UpdateStatus::Available {
        version: "9.8.7".to_string(),
    };
    assert!(painted_at(&state, "Check now").is_none());
}
