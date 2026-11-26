use de_editor_core::rename_layers::{rename_layers_text, RenameLayersHover};
use de_editor_core::{EditorState, NodeId, PenNodeExt};
use serde_json::json;

use super::rename_layers_flow as flow;
use super::rename_layers_panel::{RenameLayersHit, RenameLayersPanel};
use crate::{Point2D, Rect};

const VW: f32 = 1200.0;
const VH: f32 = 800.0;

fn state() -> EditorState {
    let doc = json!({"version": "1.0.0", "children": [
        {"type": "frame", "id": "screen", "name": "Frame 1", "width": 1440, "height": 900,
         "children": [
            {"type": "frame", "id": "cta", "name": "Rectangle 3", "width": 160, "height": 48,
             "fill": [{"type": "solid", "color": "#2563EB"}],
             "children": [{"type": "text", "id": "label", "name": "Text", "content": "Start", "fontSize": 16}]}
         ]}
    ]});
    let mut state = EditorState::from_document(
        jian_ops_schema::load_str(&doc.to_string())
            .expect("fixture")
            .value,
    );
    state.set_single_selection(NodeId::new("cta"));
    assert!(state.open_rename_layers());
    state
}

fn centre(rect: Rect) -> Point2D {
    Point2D::new(
        rect.origin.x + rect.size.x / 2.0,
        rect.origin.y + rect.size.y / 2.0,
    )
}

/// Centre of row `index`'s toggle / name field.
fn row_points(state: &EditorState, index: usize) -> (Point2D, Point2D) {
    let panel = RenameLayersPanel::for_editor(state).expect("open");
    let rect = panel.rect(VW, VH);
    let rows = panel.rows_rect(rect);
    let y = rows.origin.y + index as f32 * 34.0 + 17.0;
    (
        Point2D::new(rect.origin.x + 24.0, y),
        Point2D::new(rect.origin.x + rect.size.x - 60.0, y),
    )
}

fn name_of(state: &EditorState, id: &str) -> Option<String> {
    de_editor_core::walkers::find_node(state.active_children(), &NodeId::new(id))
        .and_then(|node| node.base().name.clone())
}

#[test]
fn rows_hit_toggle_name_and_buttons() {
    let state = state();
    let panel = RenameLayersPanel::for_editor(&state).expect("open");
    let rect = panel.rect(VW, VH);
    let (toggle, name) = row_points(&state, 0);
    assert_eq!(panel.hit_test(rect, toggle), RenameLayersHit::Toggle(0));
    assert_eq!(panel.hit_test(rect, name), RenameLayersHit::Name(0));
    assert_eq!(
        panel.hit_test(rect, centre(panel.apply_rect(rect))),
        RenameLayersHit::Apply
    );
    assert_eq!(
        panel.hit_test(rect, centre(panel.cancel_rect(rect))),
        RenameLayersHit::Cancel
    );
    assert_eq!(
        panel.hit_test(rect, Point2D::new(2.0, 2.0)),
        RenameLayersHit::Outside
    );
    assert!(panel.summary_text().contains("2 layers to rename"));
}

#[test]
fn edit_then_apply_renames_in_one_step() {
    let mut state = state();
    let (_, name) = row_points(&state, 0);
    assert!(flow::press(&mut state, VW, VH, name, 0));
    state
        .editor_ui
        .rename_layers
        .as_mut()
        .unwrap()
        .input
        .set_text("");
    for c in "Start button".chars() {
        rename_layers_text(&mut state, c, 0);
    }
    let apply = {
        let panel = RenameLayersPanel::for_editor(&state).unwrap();
        centre(panel.apply_rect(panel.rect(VW, VH)))
    };
    assert!(flow::press(&mut state, VW, VH, apply, 0));
    assert!(state.editor_ui.rename_layers.is_none());
    assert_eq!(name_of(&state, "cta").as_deref(), Some("Start button"));
    assert_eq!(name_of(&state, "label").as_deref(), Some("Start"));
    assert!(state.undo());
    assert_eq!(name_of(&state, "cta").as_deref(), Some("Rectangle 3"));
    assert_eq!(name_of(&state, "label").as_deref(), Some("Text"));
}

#[test]
fn the_scrim_is_modal_and_cancel_changes_nothing() {
    let mut state = state();
    assert!(flow::press(&mut state, VW, VH, Point2D::new(2.0, 2.0), 0));
    assert!(
        state.editor_ui.rename_layers.is_some(),
        "a scrim press keeps it open"
    );
    let cancel = {
        let panel = RenameLayersPanel::for_editor(&state).unwrap();
        centre(panel.cancel_rect(panel.rect(VW, VH)))
    };
    assert!(flow::hover(&mut state, VW, VH, cancel));
    assert_eq!(
        state.editor_ui.rename_layers.as_ref().unwrap().hover,
        Some(RenameLayersHover::Cancel)
    );
    assert!(flow::press(&mut state, VW, VH, cancel, 0));
    assert!(state.editor_ui.rename_layers.is_none());
    assert_eq!(name_of(&state, "cta").as_deref(), Some("Rectangle 3"));
    assert!(
        !flow::press(&mut state, VW, VH, cancel, 0),
        "closed: not consumed"
    );
}

#[test]
fn unticking_a_row_keeps_its_name() {
    let mut state = state();
    let (toggle, _) = row_points(&state, 0);
    assert!(flow::press(&mut state, VW, VH, toggle, 0));
    assert!(state.apply_rename_layers());
    assert_eq!(name_of(&state, "cta").as_deref(), Some("Rectangle 3"));
    assert_eq!(name_of(&state, "label").as_deref(), Some("Start"));
}
