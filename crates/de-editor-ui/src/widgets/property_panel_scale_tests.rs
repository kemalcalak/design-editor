use super::*;
use crate::layout_scene::{NodeKind, SceneNode, ScenePage};
use crate::widgets::PropertyPanel;
use de_editor_core::{NodeId, PenNodeExt};
use serde_json::json;

fn fixture() -> (EditorState, LayoutScene) {
    let doc: jian_ops_schema::PenDocument = serde_json::from_value(json!({
        "version": "1.0",
        "children": [
            {"type": "rectangle", "id": "r", "x": 100, "y": 100, "width": 100, "height": 50,
             "cornerRadius": 4}
        ]
    }))
    .expect("doc");
    let mut state = EditorState::from_document(doc);
    state.set_single_selection(NodeId::new("r"));
    let mut rect = SceneNode::leaf("r", NodeKind::Rect);
    rect.bounds = Rect::xywh(100.0, 100.0, 100.0, 50.0);
    let scene = LayoutScene {
        pages: vec![ScenePage {
            id: "p1".into(),
            name: "Page 1".into(),
            children: vec![rect],
        }],
        active_page_index: 0,
    };
    (state, scene)
}

fn commit(
    state: &mut EditorState,
    scene: &LayoutScene,
    focus: PropertyFocus,
    draft: &str,
) -> Option<bool> {
    state.ui.property_focus = Some(focus);
    state.ui.property_input.set_text(draft);
    commit_scale_focus(state, scene, 0)
}

fn rect_json(state: &EditorState) -> serde_json::Value {
    let node =
        de_editor_core::walkers::find_node(state.active_children(), &NodeId::new("r")).expect("r");
    serde_json::to_value(node).expect("json")
}

#[test]
fn the_section_shows_only_with_the_scale_tool() {
    let (mut state, scene) = fixture();
    assert!(ScaleSummary::for_state(&state).is_none());
    state.tool = Tool::Scale;
    let mut summary = ScaleSummary::for_state(&state).expect("summary");
    summary.refine(&state, &scene);
    assert_eq!((summary.width, summary.height), (100.0, 50.0));
    let panel = PropertyPanel::for_selection_with_scene(&state, &scene).expect("panel");
    assert!(panel.snapshot.scale.is_some());
    assert_eq!(
        crate::widgets::property_panel_commit::property_focus_initial(
            PropertyFocus::ScaleWidth,
            &panel
        ),
        "100"
    );
}

#[test]
fn the_factor_field_scales_about_the_anchor_as_one_undo_step() {
    let (mut state, scene) = fixture();
    state.tool = Tool::Scale;
    state.editor_ui.scale_anchor = ScaleAnchor::Center;
    assert_eq!(
        commit(&mut state, &scene, PropertyFocus::ScaleFactor, "2"),
        Some(true)
    );
    let r = rect_json(&state);
    assert_eq!(r["x"], json!(50.0));
    assert_eq!(r["y"], json!(75.0));
    assert_eq!(r["width"], json!(200.0));
    assert_eq!(r["cornerRadius"], json!(8.0));
    assert!(state.ui.property_focus.is_none());
    assert!(state.undo());
    assert_eq!(rect_json(&state)["width"], json!(100.0));
}

#[test]
fn width_and_height_fields_set_the_factor() {
    let (mut state, scene) = fixture();
    state.tool = Tool::Scale;
    assert_eq!(
        commit(&mut state, &scene, PropertyFocus::ScaleHeight, "25"),
        Some(true)
    );
    assert_eq!(rect_json(&state)["width"], json!(50.0));
    assert_eq!(rect_json(&state)["x"], json!(100.0), "top-left stays");
}

#[test]
fn bad_drafts_and_other_focuses() {
    let (mut state, scene) = fixture();
    state.tool = Tool::Scale;
    assert_eq!(
        commit(&mut state, &scene, PropertyFocus::ScaleFactor, "abc"),
        Some(false)
    );
    assert_eq!(
        commit(&mut state, &scene, PropertyFocus::ScaleFactor, "1"),
        Some(false)
    );
    assert_eq!(
        commit(&mut state, &scene, PropertyFocus::ScaleWidth, "0"),
        Some(false)
    );
    assert_eq!(
        commit(&mut state, &scene, PropertyFocus::PositionX, "4"),
        None
    );
    assert_eq!(rect_json(&state)["width"], json!(100.0));
}

#[test]
fn anchor_cells_are_actions_and_set_the_anchor() {
    let (mut state, _) = fixture();
    let mut out = Vec::new();
    push_scale_action_rects(&mut out, 0.0, 0.0, 272.0);
    assert_eq!(out.len(), 9);
    assert!(out
        .iter()
        .all(|(_, r)| r.origin.y + r.size.y <= scale_section_height()));
    let (action, _) = out[8].clone();
    assert_eq!(
        action,
        PropertyPanelAction::SetScaleAnchor(ScaleAnchor::BottomRight)
    );
    crate::widgets::property_panel_dispatch_paint::apply_paint_option_action(&mut state, &action);
    assert_eq!(state.editor_ui.scale_anchor, ScaleAnchor::BottomRight);
}

#[test]
fn an_untouched_size_field_does_not_scale_by_its_rounding() {
    let (mut state, mut scene) = fixture();
    state.tool = Tool::Scale;
    // A 100.4 px wide box opens its W field on "100".
    scene.pages[0].children[0].bounds = Rect::xywh(100.0, 100.0, 100.4, 50.0);
    let depth = state.history.past.len();
    assert_eq!(
        commit(&mut state, &scene, PropertyFocus::ScaleWidth, "100"),
        Some(false)
    );
    assert_eq!(state.history.past.len(), depth, "no undo step");
    assert_eq!(rect_json(&state)["width"], json!(100.0));
}

#[test]
fn a_kept_binding_raises_a_toast() {
    let (mut state, scene) = fixture();
    state.tool = Tool::Scale;
    let id = NodeId::new("r");
    let node = de_editor_core::walkers::find_node_mut(state.active_children_mut(), &id).unwrap();
    let mut bindings = std::collections::BTreeMap::new();
    bindings.insert(
        "cornerRadius".to_string(),
        jian_ops_schema::node::base::VariableBinding {
            variable: "radius".into(),
            value: json!(4),
        },
    );
    node.base_mut().variable_bindings = Some(bindings);
    assert_eq!(
        commit(&mut state, &scene, PropertyFocus::ScaleFactor, "2"),
        Some(true)
    );
    assert_eq!(rect_json(&state)["cornerRadius"], json!(4.0));
    let toast = state.editor_ui.editor_toast.as_ref().expect("toast");
    assert_eq!(toast.i18n_key, "scale.keptBound");
}
