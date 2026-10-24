//! #89: the Stroke section's Cap / Join / Dash rows. Each test finds a
//! cell through the shared action walker, proves `hit_test_action`
//! reaches it, then runs the shared dispatch and reads the snapshot back.

use crate::widgets::property_panel::{PropertyPanel, PropertyPanelAction};
use crate::widgets::property_panel_sections as sections;
use crate::widgets::property_panel_snapshot::StrokeStyleSummary;
use crate::widgets::property_panel_test_support::{state_from, visible_for};
use crate::{Point2D, Rect};
use de_editor_core::fills::{node_stroke_style, StrokeDashPreset, StrokeStyleEdit};
use de_editor_core::{EditorState, NodeId};
use jian_ops_schema::style::{StrokeCap, StrokeJoin};

fn panel_rect() -> Rect {
    Rect::xywh(0.0, 0.0, 280.0, 1400.0)
}

fn state(node: &str) -> EditorState {
    let mut state = state_from(&format!(
        r##"{{ "version": "1.0.0", "children": [ {node} ]}}"##
    ));
    state.set_single_selection(NodeId::new("n"));
    state
}

fn stroked_rect(extra: &str) -> EditorState {
    state(&format!(
        r##"{{"type":"rectangle","id":"n","x":0,"y":0,"width":120,"height":80,
              "stroke":{{"thickness":2,"fill":[{{"type":"solid","color":"#111111"}}]{extra}}}}}"##
    ))
}

fn panel(state: &EditorState) -> PropertyPanel {
    PropertyPanel::for_selection(state).expect("panel")
}

fn reachable(state: &EditorState, action: &PropertyPanelAction) {
    let panel = panel(state);
    let rects = sections::action_button_rects_with_fill_picker(
        panel_rect(),
        visible_for(&panel),
        &panel.snapshot.effects,
        &panel.snapshot.fills,
        &panel.snapshot.interactions,
        false,
        0,
        false,
        false,
        false,
        false,
        false,
    );
    let rect = rects
        .iter()
        .find(|(a, _)| a == action)
        .map(|(_, r)| *r)
        .unwrap_or_else(|| panic!("walker emits {action:?}"));
    let centre = Point2D::new(
        rect.origin.x + rect.size.x / 2.0,
        rect.origin.y + rect.size.y / 2.0,
    );
    assert_eq!(
        panel.hit_test_action(panel_rect(), centre).as_ref(),
        Some(action),
        "a press on {action:?} reaches it"
    );
}

fn dispatch(state: &mut EditorState, action: &PropertyPanelAction) {
    use crate::widgets::property_panel_dispatch as d;
    assert!(
        d::updates_document(action),
        "host snapshots history around it"
    );
    let mut image_adjustment_drag = None;
    let mut effect_radius_drag = None;
    let _ = d::apply_property_action(
        state,
        action,
        d::PropertyActionContext {
            now_ms: 0,
            resolved_sizing_fallback: None,
            image_adjustment_drag: &mut image_adjustment_drag,
            effect_radius_drag: &mut effect_radius_drag,
        },
    );
}

fn node(state: &EditorState) -> &jian_ops_schema::node::PenNode {
    de_editor_core::walkers::find_node(state.active_children(), &NodeId::new("n")).expect("node")
}

#[test]
fn every_cell_is_reachable() {
    let state = stroked_rect("");
    let edits = [
        StrokeStyleEdit::Cap(StrokeCap::None),
        StrokeStyleEdit::Cap(StrokeCap::Square),
        StrokeStyleEdit::Join(StrokeJoin::Miter),
        StrokeStyleEdit::Join(StrokeJoin::Bevel),
        StrokeStyleEdit::Dash(StrokeDashPreset::Solid),
        StrokeStyleEdit::Dash(StrokeDashPreset::Dotted),
    ];
    for edit in edits {
        reachable(&state, &PropertyPanelAction::SetStrokeStyle(edit));
    }
}

#[test]
fn cap_join_and_dash_write_and_read_back() {
    let mut state = stroked_rect("");
    assert_eq!(
        panel(&state).snapshot.stroke_style,
        Some(StrokeStyleSummary {
            cap: StrokeCap::None,
            join: StrokeJoin::Miter,
            dash: Some(StrokeDashPreset::Solid),
        })
    );
    for edit in [
        StrokeStyleEdit::Cap(StrokeCap::Square),
        StrokeStyleEdit::Join(StrokeJoin::Bevel),
        StrokeStyleEdit::Dash(StrokeDashPreset::Dashed),
    ] {
        dispatch(&mut state, &PropertyPanelAction::SetStrokeStyle(edit));
    }
    let style = node_stroke_style(node(&state)).expect("stroke");
    assert_eq!(style.cap, Some(StrokeCap::Square));
    assert_eq!(style.join, Some(StrokeJoin::Bevel));
    assert_eq!(style.dash_pattern, Some(vec![8.0, 8.0]));
    assert_eq!(
        panel(&state).snapshot.stroke_style,
        Some(StrokeStyleSummary {
            cap: StrokeCap::Square,
            join: StrokeJoin::Bevel,
            dash: Some(StrokeDashPreset::Dashed),
        })
    );
}

#[test]
fn a_custom_pattern_highlights_no_dash_cell_and_lines_default_round() {
    let custom = stroked_rect(r#","dashPattern":[5,1]"#);
    assert_eq!(panel(&custom).snapshot.stroke_style.unwrap().dash, None);

    let line = state(
        r##"{"type":"line","id":"n","x":0,"y":0,"x2":100,"y2":0,
             "stroke":{"thickness":1,"fill":[{"type":"solid","color":"#111111"}]}}"##,
    );
    let summary = panel(&line).snapshot.stroke_style.expect("line stroke");
    assert_eq!(summary.cap, StrokeCap::Round);
    assert_eq!(summary.join, StrokeJoin::Round);
}

#[test]
fn rows_leave_a_node_without_a_stroke_alone() {
    let mut state = state(r##"{"type":"rectangle","id":"n","x":0,"y":0,"width":10,"height":10}"##);
    dispatch(
        &mut state,
        &PropertyPanelAction::SetStrokeStyle(StrokeStyleEdit::Dash(StrokeDashPreset::Dashed)),
    );
    assert_eq!(node_stroke_style(node(&state)), None);
}
