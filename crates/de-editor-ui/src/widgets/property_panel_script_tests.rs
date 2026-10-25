use super::*;
use crate::widgets::property_panel::PropertyPanel;
use crate::widgets::property_panel_test_support::{state_from, visible_for};
use de_editor_core::shader_source_editor::SourceTarget;
use de_editor_core::NodeId;

const DOC: &str = r##"{"version":"1.0.0","children":[
  {"type":"script","id":"s","width":200,"height":100,
   "script":"/** @input bars: number = 5\n * @input stacked: boolean = true\n * @input mode: enum(a, b)\n * @input accent: color */\nreturn [];",
   "inputs":{"accent":"$brand"}}]}"##;

fn selected() -> EditorState {
    let mut state = state_from(DOC);
    state.set_single_selection(NodeId::new("s"));
    state
}

#[test]
fn a_script_lists_one_row_per_declared_input() {
    let state = selected();
    let panel = PropertyPanel::for_selection(&state).expect("script panel");
    let summary = panel.snapshot.script.as_ref().expect("script section");
    let rows: Vec<(&str, ScriptControl, &str)> = summary
        .rows
        .iter()
        .map(|r| (r.name.as_str(), r.control, r.value.as_str()))
        .collect();
    assert_eq!(
        rows,
        [
            ("bars", ScriptControl::Text, "5"),
            ("stacked", ScriptControl::Toggle(true), "true"),
            ("mode", ScriptControl::Cycle, "a"),
            ("accent", ScriptControl::Text, "$brand"),
        ]
    );
    assert_eq!(summary.status, ScriptStatusLine::None);
    assert_eq!(panel.snapshot.kind, "Script");
    let visible = visible_for(&panel);
    assert_eq!(visible.script.map(|rows| rows.count), Some(4));
}

#[test]
fn text_rows_take_focus_and_the_others_are_actions() {
    let state = selected();
    let panel = PropertyPanel::for_selection(&state).unwrap();
    let rows = ScriptRows::of(panel.snapshot.script.as_ref().unwrap());
    let mut inputs = Vec::new();
    push_script_input_rects(&mut inputs, rows, 0.0, 0.0, 240.0);
    let focuses: Vec<_> = inputs.iter().map(|(f, _)| *f).collect();
    assert_eq!(
        focuses,
        [PropertyFocus::ScriptInput(0), PropertyFocus::ScriptInput(3)]
    );
    let mut actions = Vec::new();
    push_script_action_rects(&mut actions, rows, 0.0, 0.0, 240.0);
    let kinds: Vec<_> = actions.iter().map(|(a, _)| a.clone()).collect();
    assert_eq!(
        kinds,
        [
            PropertyPanelAction::ToggleScriptInput(1),
            PropertyPanelAction::CycleScriptInput(2),
            PropertyPanelAction::OpenScriptSource,
        ]
    );
    // The button sits under the last input row.
    assert!(actions[2].1.origin.y > inputs[1].1.origin.y);
}

#[test]
fn a_status_line_adds_a_row() {
    let summary = ScriptSummary {
        rows: Vec::new(),
        status: ScriptStatusLine::None,
    };
    let quiet = script_section_height(ScriptRows::of(&summary));
    let running = ScriptSummary {
        status: ScriptStatusLine::Running,
        ..summary
    };
    assert_eq!(
        script_section_height(ScriptRows::of(&running)) - quiet,
        ROW_ADVANCE
    );
}

#[test]
fn actions_write_one_undo_step_and_open_the_source_editor() {
    let mut state = selected();
    let depth = state.history.past.len();
    assert!(apply_script_action(
        &mut state,
        &PropertyPanelAction::ToggleScriptInput(1)
    ));
    assert_eq!(state.history.past.len(), depth + 1);
    let summary = script_summary(state.selected_node().unwrap(), &state).unwrap();
    assert_eq!(summary.rows[1].control, ScriptControl::Toggle(false));

    assert!(apply_script_action(
        &mut state,
        &PropertyPanelAction::OpenScriptSource
    ));
    let editor = state
        .editor_ui
        .shader_editing
        .editor
        .as_ref()
        .expect("editor open");
    assert_eq!(editor.target, SourceTarget::Script);
    assert!(editor.input.text().contains("@input bars"));
    assert!(!state.shader_source_editor_dirty());
}

#[test]
fn the_run_status_reaches_the_summary() {
    let mut state = selected();
    let job = state.take_script_jobs().remove(0);
    let summary = script_summary(state.selected_node().unwrap(), &state).unwrap();
    assert_eq!(summary.status, ScriptStatusLine::Running);
    let failed = de_editor_core::script_regen::ScriptOutcome::Failed("boom".into());
    state.land_script_output(&job.node_id, &job.hash, failed, 1);
    let summary = script_summary(state.selected_node().unwrap(), &state).unwrap();
    assert_eq!(summary.status, ScriptStatusLine::Failed("boom".into()));
}
