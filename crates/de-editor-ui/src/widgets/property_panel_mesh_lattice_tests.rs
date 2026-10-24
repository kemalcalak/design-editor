//! #142: the mesh body's Rows / Columns steppers. Each test finds a
//! stepper through the shared action walker, proves `hit_test_action`
//! reaches it, then runs the shared dispatch the way both hosts do
//! (history snapshot around `updates_document` actions).

use crate::widgets::property_panel::{PropertyPanel, PropertyPanelAction};
use crate::widgets::property_panel_sections as sections;
use crate::widgets::property_panel_test_support::{state_from, visible_for};
use crate::{Point2D, Rect};
use de_editor_core::fills::node_fills;
use de_editor_core::{ColorTarget, EditorState, FillType, MeshAxis, MeshLatticeEdit, NodeId};
use jian_ops_schema::style::{MeshGradientBody, PenFill};

fn panel_rect() -> Rect {
    Rect::xywh(0.0, 0.0, 280.0, 1400.0)
}

fn mesh_state() -> EditorState {
    let mut state = state_from(
        r##"{ "version": "1.0.0", "children": [
              {"type":"rectangle","id":"r","name":"R","x":0,"y":0,"width":120,"height":80,
               "fill":[{"type":"solid","color":"#3366ff"}]}
        ]}"##,
    );
    state.set_single_selection(NodeId::new("r"));
    assert!(state.set_selected_fill_type_at(0, FillType::MeshGradient));
    state
}

fn actions(state: &EditorState) -> (PropertyPanel, Vec<(PropertyPanelAction, Rect)>) {
    let panel = PropertyPanel::for_selection(state).expect("panel");
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
    (panel, rects)
}

/// The walker emits `action` and a press at its centre resolves to it.
fn reachable(state: &EditorState, action: &PropertyPanelAction) -> Rect {
    let (panel, rects) = actions(state);
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
    rect
}

/// The hosts' choke point: snapshot, dispatch, push only on a change.
fn press(state: &mut EditorState, edit: MeshLatticeEdit) {
    use crate::widgets::property_panel_dispatch as d;
    let action = PropertyPanelAction::EditMeshLattice(edit);
    reachable(state, &action);
    assert!(d::updates_document(&action), "host snapshots history");
    let before = state.snapshot_for_history();
    let mut image_adjustment_drag = None;
    let mut effect_radius_drag = None;
    let _ = d::apply_property_action(
        state,
        &action,
        d::PropertyActionContext {
            now_ms: 0,
            resolved_sizing_fallback: None,
            image_adjustment_drag: &mut image_adjustment_drag,
            effect_radius_drag: &mut effect_radius_drag,
        },
    );
    if state.snapshot_for_history() != before {
        state.history_push_past(before);
    }
}

fn mesh(state: &EditorState) -> MeshGradientBody {
    let node = de_editor_core::walkers::find_node(state.active_children(), &NodeId::new("r"))
        .expect("node");
    match node_fills(node).and_then(|f| f.first()) {
        Some(PenFill::MeshGradient(body)) => body.clone(),
        other => panic!("expected a mesh fill, got {other:?}"),
    }
}

#[test]
fn steppers_resize_the_lattice_and_reflow_the_vertex_grid() {
    let mut state = mesh_state();
    let (panel, _) = actions(&state);
    assert_eq!(panel.snapshot.mesh_lattice, Some((2, 2)));

    press(&mut state, MeshLatticeEdit::Add(MeshAxis::Columns));
    press(&mut state, MeshLatticeEdit::Add(MeshAxis::Rows));
    let body = mesh(&state);
    assert_eq!((body.rows, body.cols), (3, 3));
    assert_eq!(body.stops.len(), 9);
    let (panel, rects) = actions(&state);
    assert_eq!(panel.snapshot.mesh_lattice, Some((3, 3)));
    assert_eq!(panel.snapshot.gradient_stops.len(), 9);

    // Every vertex is a target, and the grid still ends above Stroke.
    let last = reachable(
        &state,
        &PropertyPanelAction::OpenColorPicker(ColorTarget::GradientStop(8)),
    );
    let stroke_top = rects
        .iter()
        .find(|(a, _)| matches!(a, PropertyPanelAction::ToggleStrokeModePopover))
        .map(|(_, r)| r.origin.y)
        .expect("stroke gear");
    assert!(last.origin.y + last.size.y < stroke_top);

    press(&mut state, MeshLatticeEdit::Remove(MeshAxis::Rows));
    assert_eq!((mesh(&state).rows, mesh(&state).cols), (2, 3));
}

#[test]
fn each_step_is_one_undo_and_a_limit_press_adds_none() {
    let mut state = mesh_state();
    let original = mesh(&state);
    let depth = state.history.past.len();

    press(&mut state, MeshLatticeEdit::Remove(MeshAxis::Rows));
    assert_eq!(state.history.past.len(), depth, "2 rows is the minimum");
    assert_eq!(mesh(&state), original);

    press(&mut state, MeshLatticeEdit::Add(MeshAxis::Rows));
    press(&mut state, MeshLatticeEdit::Add(MeshAxis::Columns));
    assert_eq!(state.history.past.len(), depth + 2);
    assert!(state.undo());
    assert_eq!((mesh(&state).rows, mesh(&state).cols), (3, 2));
    assert!(state.undo());
    assert_eq!(mesh(&state), original);
}
