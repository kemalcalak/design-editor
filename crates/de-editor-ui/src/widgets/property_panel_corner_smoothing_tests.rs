//! #389: the Position section's corner smoothing row — which nodes show
//! it, that the slider and field are reachable, that a drag is one undo
//! step, that the typed percentage commits, and that every translation of
//! its label fits.

use super::*;
use crate::widgets::property_panel::{PropertyPanel, PropertyPanelAction};
use crate::widgets::property_panel_sections as sections;
use crate::widgets::property_panel_test_support::{state_from, visible_for};
use crate::Point2D;
use de_editor_core::command_node_attrs::corners::node_corner_smoothing;
use de_editor_core::NodeId;

fn panel_rect() -> Rect {
    Rect::xywh(0.0, 0.0, 272.0, 1400.0)
}

fn state(node: &str) -> EditorState {
    let mut state = state_from(&format!(
        r##"{{ "version": "1.0.0", "children": [ {node} ]}}"##
    ));
    state.set_single_selection(NodeId::new("n"));
    state
}

fn rect_node() -> EditorState {
    state(r#"{"type":"rectangle","id":"n","x":0,"y":0,"width":100,"height":60,"cornerRadius":12}"#)
}

fn panel(state: &EditorState) -> PropertyPanel {
    PropertyPanel::for_selection(state).expect("panel")
}

fn smoothing(state: &EditorState) -> f32 {
    let node =
        de_editor_core::walkers::find_node(state.active_children(), &NodeId::new("n")).unwrap();
    node_corner_smoothing(node).unwrap()
}

fn slider_rect(panel: &PropertyPanel) -> Option<Rect> {
    sections::action_button_rects_with_fill_picker(
        panel_rect(),
        visible_for(panel),
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
    )
    .into_iter()
    .find(|(action, _)| matches!(action, PropertyPanelAction::SetCornerSmoothing(_)))
    .map(|(_, rect)| rect)
}

fn value_rect(panel: &PropertyPanel) -> Option<Rect> {
    sections::editable_input_rects(
        panel_rect(),
        visible_for(panel),
        &panel.snapshot.fills,
        &panel.snapshot.effects,
    )
    .into_iter()
    .find(|(focus, _)| *focus == PropertyFocus::CornerSmoothing)
    .map(|(_, rect)| rect)
}

fn dispatch(state: &mut EditorState, action: &PropertyPanelAction, dragging: &mut bool) {
    use crate::widgets::property_panel_dispatch as d;
    let _ = d::apply_property_action(
        state,
        action,
        d::PropertyActionContext {
            now_ms: 0,
            resolved_sizing_fallback: None,
            image_adjustment_drag: &mut None,
            effect_radius_drag: &mut None,
            corner_smoothing_drag: dragging,
        },
    );
}

#[test]
fn the_row_shows_for_kinds_that_store_smoothing() {
    for node in [
        r#"{"type":"rectangle","id":"n","width":10,"height":10,"cornerRadius":2}"#,
        r#"{"type":"frame","id":"n","width":10,"height":10,"cornerRadius":[0,0,3,0]}"#,
        r#"{"type":"polygon","id":"n","width":10,"height":10,"polygonCount":5,"cornerRadius":2}"#,
        r#"{"type":"path","id":"n","width":10,"height":10,"d":"M0 0 L10 0 L10 10","vertexRadius":2}"#,
    ] {
        let p = panel(&state(node));
        assert!(visible_for(&p).corner_smoothing, "{node}");
        assert!(
            slider_rect(&p).is_some() && value_rect(&p).is_some(),
            "{node}"
        );
    }
    for node in [
        // Nothing rounded yet: nothing to smooth.
        r#"{"type":"rectangle","id":"n","width":10,"height":10}"#,
        r#"{"type":"polygon","id":"n","width":10,"height":10,"polygonCount":5}"#,
        r#"{"type":"ellipse","id":"n","width":10,"height":10,"cornerRadius":2}"#,
        r##"{"type":"image","id":"n","width":10,"height":10,"src":"x.png"}"##,
        r#"{"type":"text","id":"n","content":"hi"}"#,
    ] {
        let p = panel(&state(node));
        assert!(!visible_for(&p).corner_smoothing, "{node}");
        assert!(slider_rect(&p).is_none(), "{node}");
    }
    // A path now edits its radius too.
    let path = state(r#"{"type":"path","id":"n","width":10,"height":10,"d":"M0 0 L10 0 L10 10"}"#);
    assert!(panel(&path).snapshot.has_corner_radius);
}

#[test]
fn the_row_sits_under_the_radius_and_grows_the_section() {
    let state = rect_node();
    let p = panel(&state);
    let visible = visible_for(&p);
    let slider = slider_rect(&p).unwrap();
    let value = value_rect(&p).unwrap();
    let radius = sections::editable_input_rects(
        panel_rect(),
        visible,
        &p.snapshot.fills,
        &p.snapshot.effects,
    )
    .into_iter()
    .find(|(focus, _)| *focus == PropertyFocus::PositionR)
    .map(|(_, rect)| rect)
    .unwrap();
    assert!(value.origin.y >= radius.origin.y + radius.size.y);
    assert!(slider.origin.x + slider.size.x <= value.origin.x);
    assert_eq!(
        value.origin.x + value.size.x,
        radius.origin.x + radius.size.x + 28.0,
        "the field lines up with the radius column's right edge"
    );
    let mut without = visible;
    without.corner_smoothing = false;
    assert_eq!(
        visible.position_section_h() - without.position_section_h(),
        SMOOTHING_ROW_HEIGHT
    );
    // Opening the per-corner grid pushes the row down.
    let mut grid = visible;
    grid.corner_expand = true;
    let shifted =
        sections::editable_input_rects(panel_rect(), grid, &p.snapshot.fills, &p.snapshot.effects)
            .into_iter()
            .find(|(focus, _)| *focus == PropertyFocus::CornerSmoothing)
            .map(|(_, rect)| rect)
            .unwrap();
    assert_eq!(
        shifted.origin.y - value.origin.y,
        crate::widgets::property_panel_corner::CORNER_GRID_EXTRA_HEIGHT
    );
}

#[test]
fn a_slider_drag_is_one_undo_step() {
    let mut state = rect_node();
    let p = panel(&state);
    let slider = slider_rect(&p).unwrap();
    let at = |t: f32| Point2D::new(slider.origin.x + slider.size.x * t, slider.origin.y + 8.0);
    // A press lands where the pointer is.
    let press = p.hit_test_action(panel_rect(), at(0.25)).expect("press");
    assert_eq!(press, PropertyPanelAction::SetCornerSmoothing(25.0));
    let mut dragging = false;
    dispatch(&mut state, &press, &mut dragging);
    assert!(dragging);
    for t in [0.5, 0.75, 1.0] {
        let step = panel(&state)
            .corner_smoothing_drag_action(panel_rect(), at(t).x)
            .expect("drag step");
        dispatch(&mut state, &step, &mut dragging);
    }
    assert_eq!(smoothing(&state), 1.0);
    assert_eq!(panel(&state).snapshot.corner_smoothing, Some(1.0));
    assert!(state.undo());
    assert_eq!(smoothing(&state), 0.0, "the whole drag undoes at once");
}

#[test]
fn the_typed_percentage_commits_and_clamps() {
    let mut state = rect_node();
    let commit = |state: &mut EditorState, draft: &str| {
        state.ui.property_focus = Some(PropertyFocus::CornerSmoothing);
        state.ui.property_input.set_text(draft);
        assert!(crate::widgets::property_panel_commit::commit_property_focus(state));
    };
    commit(&mut state, "60");
    assert!((smoothing(&state) - 0.6).abs() < 1e-6);
    assert_eq!(
        crate::widgets::property_panel_commit::property_focus_initial(
            PropertyFocus::CornerSmoothing,
            &panel(&state)
        ),
        "60"
    );
    commit(&mut state, "250");
    assert_eq!(smoothing(&state), 1.0);
    assert!(state.undo());
    assert!((smoothing(&state) - 0.6).abs() < 1e-6);
}

/// The label fits its column in every locale at the panel's width, by the
/// same 12 px estimate the stroke-end rows are swept with.
#[test]
fn the_label_fits_in_every_locale() {
    let rects = row_rects(
        0.0,
        0.0,
        crate::widgets::property_panel::PROPERTY_PANEL_WIDTH,
    );
    let room = rects.label.size.x - 6.0;
    for locale in de_editor_core::Locale::ALL {
        let ui = de_editor_core::editor_ui_state::EditorUiState {
            locale,
            ..Default::default()
        };
        let label = sections::PropertyLabels::for_editor_ui(&ui).corner_smoothing;
        let width = crate::widgets::property_panel_test_support::chrome_width(label);
        assert!(
            width <= room,
            "{locale:?}: {label:?} is {width:.0} px, room {room:.0} px"
        );
    }
    assert!(rects.slider.size.x >= 40.0, "{:?}", rects.slider);
}
